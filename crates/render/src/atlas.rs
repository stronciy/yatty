use std::collections::HashMap;

use cosmic_text::CacheKey;

pub const ATLAS_SIZE: u32 = 2048;

#[derive(Clone, Copy, Debug)]
pub struct GlyphEntry {
    pub uv: [f32; 4],
    pub left: i32,
    pub top: i32,
    pub width: u32,
    pub height: u32,
}

pub struct Glyph {
    pub left: i32,
    pub top: i32,
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>,
}

pub struct Atlas {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    cursor: (u32, u32),
    row_height: u32,
    entries: HashMap<CacheKey, Option<GlyphEntry>>,
    white: GlyphEntry,
}

impl Atlas {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("glyph atlas"),
            size: wgpu::Extent3d {
                width: ATLAS_SIZE,
                height: ATLAS_SIZE,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        let center = 0.5 / ATLAS_SIZE as f32;
        let white = GlyphEntry {
            uv: [center; 4],
            left: 0,
            top: 0,
            width: 1,
            height: 1,
        };
        let mut atlas = Self {
            texture,
            view,
            cursor: (0, 0),
            row_height: 0,
            entries: HashMap::new(),
            white,
        };
        atlas.upload(queue, 0, 0, 1, 1, &[255]);
        atlas.cursor = (2, 0);
        atlas.row_height = 1;
        atlas
    }

    pub fn view(&self) -> &wgpu::TextureView {
        &self.view
    }

    pub fn white(&self) -> GlyphEntry {
        self.white
    }

    pub fn get_or_insert(
        &mut self,
        queue: &wgpu::Queue,
        key: CacheKey,
        rasterize: impl FnOnce() -> Option<Glyph>,
    ) -> Option<GlyphEntry> {
        if let Some(entry) = self.entries.get(&key) {
            return *entry;
        }
        let entry = rasterize().and_then(|glyph| self.insert(queue, glyph));
        self.entries.insert(key, entry);
        entry
    }

    fn insert(&mut self, queue: &wgpu::Queue, glyph: Glyph) -> Option<GlyphEntry> {
        if glyph.width == 0 || glyph.height == 0 {
            return None;
        }
        if glyph.data.len() != (glyph.width * glyph.height) as usize {
            return None;
        }
        let (mut x, mut y) = self.cursor;
        if x + glyph.width > ATLAS_SIZE {
            x = 0;
            y += self.row_height;
            self.row_height = 0;
        }
        if y + glyph.height > ATLAS_SIZE {
            tracing::warn!("glyph atlas is full, glyph skipped");
            return None;
        }
        self.upload(queue, x, y, glyph.width, glyph.height, &glyph.data);
        self.cursor = (x + glyph.width, y);
        self.row_height = self.row_height.max(glyph.height);

        let size = ATLAS_SIZE as f32;
        Some(GlyphEntry {
            uv: [
                x as f32 / size,
                y as f32 / size,
                (x + glyph.width) as f32 / size,
                (y + glyph.height) as f32 / size,
            ],
            left: glyph.left,
            top: glyph.top,
            width: glyph.width,
            height: glyph.height,
        })
    }

    fn upload(&self, queue: &wgpu::Queue, x: u32, y: u32, width: u32, height: u32, data: &[u8]) {
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d { x, y, z: 0 },
                aspect: wgpu::TextureAspect::All,
            },
            data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(width),
                rows_per_image: Some(height),
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
    }
}
