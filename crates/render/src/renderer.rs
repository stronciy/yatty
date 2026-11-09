use std::borrow::Cow;
use std::path::PathBuf;

use cosmic_text::{
    Attrs, Buffer, Family, FeatureTag, FontFeatures, FontSystem, Metrics, Shaping, Style,
    SwashCache, Weight,
};
use wgpu::util::DeviceExt;

use crate::atlas::{Atlas, Glyph, GlyphEntry};
use crate::{CursorShape, FontSpec, Insets, RenderGrid};

const BOLD: u8 = 1;
const ITALIC: u8 = 2;

const SHADER: &str = include_str!("shader.wgsl");

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex {
    position: [f32; 2],
    uv: [f32; 2],
    color: [f32; 4],
}

pub struct Renderer {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    pipeline: wgpu::RenderPipeline,
    bind_group: wgpu::BindGroup,
    atlas: Atlas,
    font_system: FontSystem,
    swash: SwashCache,
    metrics: Metrics,
    cell_w: f32,
    cell_h: f32,
    font: FontSpec,
    row_text: Vec<Vec<(char, u8)>>,
    row_buffers: Vec<Buffer>,
    capture: Option<PathBuf>,
    insets: Insets,
}

impl Renderer {
    pub fn new(
        target: impl Into<wgpu::SurfaceTarget<'static>>,
        width: u32,
        height: u32,
        font: FontSpec,
    ) -> anyhow::Result<Self> {
        let instance = wgpu::Instance::default();
        let surface = instance.create_surface(target)?;
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
            apply_limit_buckets: false,
        }))?;
        let (device, queue) =
            pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))?;

        let capabilities = surface.get_capabilities(&adapter);
        let format = capabilities
            .formats
            .iter()
            .copied()
            .find(|format| !format.is_srgb())
            .unwrap_or(capabilities.formats[0]);
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            format,
            color_space: wgpu::SurfaceColorSpace::Srgb,
            width: width.max(1),
            height: height.max(1),
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: capabilities.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        let atlas = Atlas::new(&device, &queue);
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("text bind group layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("text bind group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(atlas.view()),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("text shader"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(SHADER)),
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("text pipeline layout"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });
        let vertex_attributes =
            wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x2, 2 => Float32x4];
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("text pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &vertex_attributes,
                })],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });

        let mut font_system = FontSystem::new();
        let metrics = Metrics::new(font.size_px, (font.size_px * 1.25).ceil());
        let cell_w = measure_advance(&mut font_system, metrics, &text_attrs(&font), font.size_px);
        let cell_h = metrics.line_height;

        Ok(Self {
            surface,
            device,
            queue,
            config,
            pipeline,
            bind_group,
            atlas,
            font_system,
            swash: SwashCache::new(),
            metrics,
            cell_w,
            cell_h,
            font,
            row_text: Vec::new(),
            row_buffers: Vec::new(),
            capture: None,
            insets: Insets::default(),
        })
    }

    /// Sets the padding around the content. The space left over after fitting whole rows
    /// is added to the top, so the last row (the footer bar) keeps exactly one row height.
    pub fn fit_insets(&mut self, base: Insets) {
        let height = self.config.height as f32;
        let rows = ((height - base.top - base.bottom) / self.cell_h)
            .floor()
            .max(1.0);
        let leftover = (height - base.top - base.bottom - rows * self.cell_h).max(0.0);
        self.insets = Insets {
            top: base.top + leftover,
            ..base
        };
    }

    pub fn insets(&self) -> Insets {
        self.insets
    }

    pub fn cell_size(&self) -> (f32, f32) {
        (self.cell_w, self.cell_h)
    }

    pub fn capture_next_frame(&mut self, path: PathBuf) {
        self.capture = Some(path);
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.config.width = width.max(1);
        self.config.height = height.max(1);
        self.surface.configure(&self.device, &self.config);
    }

    pub fn draw(&mut self, grid: &RenderGrid) {
        self.sync_rows(grid.rows);

        let screen = (self.config.width as f32, self.config.height as f32);
        let cell_w = self.cell_w;
        let cell_h = self.cell_h;
        let mut vertices: Vec<Vertex> = Vec::with_capacity(grid.cols * grid.rows * 6);

        let (ox, oy) = (self.insets.left, self.insets.top);
        let white = self.atlas.white();
        let thickness = (self.font.size_px / 14.0).round().max(1.0);
        for row in 0..grid.rows {
            for col in 0..grid.cols {
                let cell = grid.cell(row, col);
                let x = ox + col as f32 * cell_w;
                let y = oy + row as f32 * cell_h;
                push_quad(
                    &mut vertices,
                    screen,
                    [x, y, x + cell_w, y + cell_h],
                    white,
                    cell.bg,
                );
                if cell.underline {
                    let line_y = y + cell_h - 2.0 * thickness;
                    push_quad(
                        &mut vertices,
                        screen,
                        [x, line_y, x + cell_w, line_y + thickness],
                        white,
                        cell.fg,
                    );
                }
            }
        }
        if let Some(bar) = grid.bottom_bar {
            let y = oy + (grid.rows - 1) as f32 * cell_h;
            push_quad(
                &mut vertices,
                screen,
                [0.0, y, screen.0, screen.1],
                white,
                bar,
            );
        }
        let cursor_color = [
            grid.cursor_color[0],
            grid.cursor_color[1],
            grid.cursor_color[2],
            grid.cursor_color[3] * grid.cursor_alpha,
        ];
        if let Some((row, col)) = grid.cursor.filter(|_| grid.cursor_alpha > 0.0) {
            let x = ox + col as f32 * cell_w;
            let y = oy + row as f32 * cell_h;
            let rect = match grid.cursor_shape {
                CursorShape::Block => [x, y, x + cell_w, y + cell_h],
                CursorShape::Bar => {
                    let width = (self.font.size_px / 8.0).round().max(2.0);
                    [x, y, x + width, y + cell_h]
                }
                CursorShape::Underline => [
                    x,
                    y + cell_h - thickness * 2.0,
                    x + cell_w,
                    y + cell_h - thickness,
                ],
            };
            push_quad(&mut vertices, screen, rect, white, cursor_color);
        }

        let Self {
            atlas,
            font_system,
            swash,
            queue,
            row_buffers,
            row_text,
            font,
            ..
        } = self;
        let attrs = text_attrs(font);
        for row in 0..grid.rows {
            let key: Vec<(char, u8)> = (0..grid.cols)
                .map(|col| {
                    let cell = grid.cell(row, col);
                    let ch = if cell.wide_spacer { ' ' } else { cell.ch };
                    (ch, style_bits(cell.bold, cell.italic))
                })
                .collect();
            if row_text[row] != key {
                let mut spans: Vec<(String, u8)> = Vec::new();
                for (ch, style) in &key {
                    match spans.last_mut() {
                        Some((text, last)) if last == style => text.push(*ch),
                        _ => spans.push((ch.to_string(), *style)),
                    }
                }
                row_buffers[row].set_rich_text(
                    spans
                        .iter()
                        .map(|(text, style)| (text.as_str(), styled_attrs(font, *style))),
                    &attrs,
                    Shaping::Advanced,
                    None,
                );
                row_buffers[row].shape_until_scroll(font_system, false);
                row_text[row] = key;
            }

            for run in row_buffers[row].layout_runs() {
                for glyph in run.glyphs {
                    let col = (glyph.x / cell_w).round() as usize;
                    if col >= grid.cols {
                        continue;
                    }
                    let cell = grid.cell(row, col);
                    let block_under_cursor = grid.cursor == Some((row, col))
                        && grid.cursor_shape == CursorShape::Block
                        && grid.cursor_alpha >= 0.5;
                    let fg = if block_under_cursor {
                        grid.background
                    } else {
                        cell.fg
                    };
                    let physical = glyph.physical((0.0, run.line_y), 1.0);
                    let key = physical.cache_key;
                    let entry: Option<GlyphEntry> = atlas.get_or_insert(queue, key, || {
                        swash
                            .get_image(font_system, key)
                            .as_ref()
                            .map(|image| Glyph {
                                left: image.placement.left,
                                top: image.placement.top,
                                width: image.placement.width,
                                height: image.placement.height,
                                data: image.data.clone(),
                            })
                    });
                    if let Some(entry) = entry {
                        let x0 = ox + physical.x as f32 + entry.left as f32;
                        let y0 = oy + row as f32 * cell_h + physical.y as f32 - entry.top as f32;
                        let rect = [x0, y0, x0 + entry.width as f32, y0 + entry.height as f32];
                        push_quad(&mut vertices, screen, rect, entry, fg);
                    }
                }
            }
        }

        let frame = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame)
            | wgpu::CurrentSurfaceTexture::Suboptimal(frame) => frame,
            _ => {
                self.surface.configure(&self.device, &self.config);
                return;
            }
        };
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("text vertices"),
                contents: bytemuck::cast_slice(&vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("frame"),
            });
        let readback = self.capture.take().map(|path| {
            let (width, height) = (self.config.width, self.config.height);
            let padded_row = (width * 4).div_ceil(256) * 256;
            let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("capture"),
                size: (padded_row * height) as u64,
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            });
            (path, buffer, padded_row)
        });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("text pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: grid.background[0] as f64,
                            g: grid.background[1] as f64,
                            b: grid.background[2] as f64,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.bind_group, &[]);
            pass.set_vertex_buffer(0, buffer.slice(..));
            pass.draw(0..vertices.len() as u32, 0..1);
        }
        if let Some((_, buffer, padded_row)) = &readback {
            encoder.copy_texture_to_buffer(
                wgpu::TexelCopyTextureInfo {
                    texture: &frame.texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::TexelCopyBufferInfo {
                    buffer,
                    layout: wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(*padded_row),
                        rows_per_image: Some(self.config.height),
                    },
                },
                wgpu::Extent3d {
                    width: self.config.width,
                    height: self.config.height,
                    depth_or_array_layers: 1,
                },
            );
        }
        self.queue.submit(Some(encoder.finish()));
        if let Some((path, buffer, padded_row)) = readback
            && let Err(err) = self.save_capture(&buffer, padded_row, &path)
        {
            tracing::error!("capture failed: {err}");
        }
        self.queue.present(frame);
    }

    fn save_capture(
        &self,
        buffer: &wgpu::Buffer,
        padded_row: u32,
        path: &PathBuf,
    ) -> anyhow::Result<()> {
        let (width, height) = (self.config.width, self.config.height);
        let slice = buffer.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = tx.send(result);
        });
        self.device.poll(wgpu::PollType::wait_indefinitely())?;
        rx.recv()??;

        let data = slice.get_mapped_range()?;
        let swap_rb = matches!(
            self.config.format,
            wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Bgra8UnormSrgb
        );
        let mut rgba = Vec::with_capacity((width * height * 4) as usize);
        for row in 0..height {
            let start = (row * padded_row) as usize;
            for px in data[start..start + (width * 4) as usize].as_chunks::<4>().0 {
                if swap_rb {
                    rgba.extend_from_slice(&[px[2], px[1], px[0], px[3]]);
                } else {
                    rgba.extend_from_slice(px);
                }
            }
        }
        drop(data);
        buffer.unmap();

        let file = std::io::BufWriter::new(std::fs::File::create(path)?);
        let mut encoder = png::Encoder::new(file, width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.write_header()?.write_image_data(&rgba)?;
        tracing::info!("frame saved to {}", path.display());
        Ok(())
    }

    fn sync_rows(&mut self, rows: usize) {
        while self.row_buffers.len() < rows {
            let mut buffer = Buffer::new(&mut self.font_system, self.metrics);
            buffer.set_size(None, None);
            buffer.set_monospace_width(Some(self.cell_w));
            self.row_buffers.push(buffer);
            self.row_text.push(Vec::new());
        }
        self.row_buffers.truncate(rows);
        self.row_text.truncate(rows);
    }
}

fn style_bits(bold: bool, italic: bool) -> u8 {
    (u8::from(bold) * BOLD) | (u8::from(italic) * ITALIC)
}

fn styled_attrs(font: &FontSpec, style: u8) -> Attrs<'_> {
    let mut attrs = text_attrs(font);
    if style & BOLD != 0 {
        attrs = attrs.weight(Weight(700));
    }
    if style & ITALIC != 0 {
        attrs = attrs.style(Style::Italic);
    }
    attrs
}

fn text_attrs(font: &FontSpec) -> Attrs<'_> {
    let mut features = FontFeatures::new();
    if !font.ligatures {
        features
            .disable(FeatureTag::STANDARD_LIGATURES)
            .disable(FeatureTag::CONTEXTUAL_LIGATURES)
            .disable(FeatureTag::CONTEXTUAL_ALTERNATES);
    }
    Attrs::new()
        .family(Family::Name(&font.family))
        .weight(Weight(font.weight))
        .font_features(features)
}

fn measure_advance(
    font_system: &mut FontSystem,
    metrics: Metrics,
    attrs: &Attrs,
    fallback: f32,
) -> f32 {
    let mut buffer = Buffer::new(font_system, metrics);
    buffer.set_size(None, None);
    buffer.set_text("M", attrs, Shaping::Advanced, None);
    buffer.shape_until_scroll(font_system, false);
    buffer
        .layout_runs()
        .next()
        .and_then(|run| run.glyphs.first().map(|glyph| glyph.w))
        .filter(|width| *width > 0.0)
        .unwrap_or(fallback * 0.6)
}

fn push_quad(
    vertices: &mut Vec<Vertex>,
    screen: (f32, f32),
    rect: [f32; 4],
    entry: GlyphEntry,
    color: [f32; 4],
) {
    let [x0, y0, x1, y1] = rect;
    let [u0, v0, u1, v1] = entry.uv;
    let to_ndc = |x: f32, y: f32| [x / screen.0 * 2.0 - 1.0, 1.0 - y / screen.1 * 2.0];
    let tl = Vertex {
        position: to_ndc(x0, y0),
        uv: [u0, v0],
        color,
    };
    let tr = Vertex {
        position: to_ndc(x1, y0),
        uv: [u1, v0],
        color,
    };
    let bl = Vertex {
        position: to_ndc(x0, y1),
        uv: [u0, v1],
        color,
    };
    let br = Vertex {
        position: to_ndc(x1, y1),
        uv: [u1, v1],
        color,
    };
    vertices.extend_from_slice(&[tl, bl, tr, tr, bl, br]);
}
