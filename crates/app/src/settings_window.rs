use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use config::{Colors, Config, Rgb, Settings, Theme};
use egui::{ComboBox, DragValue, ScrollArea};
use winit::event::WindowEvent;
use winit::event_loop::ActiveEventLoop;
use winit::window::{Window, WindowAttributes, WindowId};

const WEIGHTS: [(u16, &str); 4] = [
    (300, "Light"),
    (400, "Regular"),
    (500, "Medium"),
    (700, "Bold"),
];

#[derive(Clone, Copy, PartialEq)]
enum Tab {
    Font,
    Colors,
    Footer,
}

#[derive(Clone, Copy, PartialEq)]
enum Action {
    None,
    Save,
    Revert,
}

pub enum Outcome {
    Nothing,
    Close,
    Saved(Config),
}

struct Form {
    config_dir: PathBuf,
    saved: Config,
    draft: Config,
    theme: Theme,
    themes: Vec<String>,
    families: Vec<String>,
    tab: Tab,
    status: String,
}

pub struct SettingsWindow {
    pub window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface_config: wgpu::SurfaceConfiguration,
    ctx: egui::Context,
    input: egui_winit::State,
    renderer: egui_wgpu::Renderer,
    form: Form,
    repaint_at: Option<Instant>,
}

impl SettingsWindow {
    pub fn open(
        event_loop: &ActiveEventLoop,
        config_dir: PathBuf,
        saved: &Config,
    ) -> anyhow::Result<Self> {
        let attributes = WindowAttributes::default()
            .with_title("Settings")
            .with_inner_size(winit::dpi::LogicalSize::new(520.0, 480.0));
        let window = Arc::new(event_loop.create_window(attributes)?);
        let size = window.inner_size();

        let instance = wgpu::Instance::default();
        let surface = instance.create_surface(window.clone())?;
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::LowPower,
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
        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            color_space: wgpu::SurfaceColorSpace::Srgb,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: capabilities.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &surface_config);

        let ctx = egui::Context::default();
        let input = egui_winit::State::new(
            ctx.clone(),
            egui::ViewportId::ROOT,
            &*window,
            Some(window.scale_factor() as f32),
            None,
            None,
        );
        let renderer =
            egui_wgpu::Renderer::new(&device, format, egui_wgpu::RendererOptions::default());

        let theme = Settings::from_config(saved, &config_dir).theme;
        let themes = config::available_themes(&config_dir);
        let families = render::monospace_families();
        Ok(Self {
            window,
            surface,
            device,
            queue,
            surface_config,
            ctx,
            input,
            renderer,
            repaint_at: Some(Instant::now()),
            form: Form {
                config_dir,
                saved: saved.clone(),
                draft: saved.clone(),
                theme,
                themes,
                families,
                tab: Tab::Font,
                status: String::new(),
            },
        })
    }

    pub fn repaint_due(&self) -> bool {
        self.repaint_at.is_some_and(|at| at <= Instant::now())
    }

    pub fn id(&self) -> WindowId {
        self.window.id()
    }

    pub fn handle_event(&mut self, event: &WindowEvent) -> Outcome {
        let _ = self.input.on_window_event(&self.window, event);
        match event {
            WindowEvent::CloseRequested => Outcome::Close,
            WindowEvent::Resized(size) => {
                self.surface_config.width = size.width.max(1);
                self.surface_config.height = size.height.max(1);
                self.surface.configure(&self.device, &self.surface_config);
                self.window.request_redraw();
                Outcome::Nothing
            }
            WindowEvent::RedrawRequested => self.draw(),
            _ => {
                self.window.request_redraw();
                Outcome::Nothing
            }
        }
    }

    fn draw(&mut self) -> Outcome {
        let Self {
            window,
            surface,
            device,
            queue,
            surface_config,
            ctx,
            input,
            renderer,
            form,
            repaint_at,
        } = self;

        let raw_input = input.take_egui_input(window);
        let mut action = Action::None;
        let mut output = ctx.run_ui(raw_input, |ui| action = form.ui(ui));
        *repaint_at = output
            .viewport_output
            .values()
            .map(|viewport| viewport.repaint_delay)
            .min()
            .filter(|delay| *delay < Duration::MAX)
            .map(|delay| Instant::now() + delay);
        input.handle_platform_output(window, output.platform_output);

        let outcome = match action {
            Action::Save => form.save(),
            Action::Revert => {
                form.draft = form.saved.clone();
                form.theme = Settings::from_config(&form.saved, &form.config_dir).theme;
                form.status = "Changes discarded".into();
                Outcome::Nothing
            }
            Action::None => Outcome::Nothing,
        };

        let primitives = ctx.tessellate(output.shapes, output.pixels_per_point);
        for (id, deltas) in &output.textures_delta.set {
            for delta in deltas {
                renderer.update_texture(device, queue, *id, delta);
            }
        }
        let screen = egui_wgpu::ScreenDescriptor {
            size_in_pixels: [surface_config.width, surface_config.height],
            pixels_per_point: output.pixels_per_point,
        };

        match surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame)
            | wgpu::CurrentSurfaceTexture::Suboptimal(frame) => {
                let view = frame
                    .texture
                    .create_view(&wgpu::TextureViewDescriptor::default());
                let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("settings"),
                });
                renderer.update_buffers(device, queue, &mut encoder, &primitives, &screen);
                {
                    let mut pass = encoder
                        .begin_render_pass(&wgpu::RenderPassDescriptor {
                            label: Some("settings pass"),
                            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                                view: &view,
                                resolve_target: None,
                                depth_slice: None,
                                ops: wgpu::Operations {
                                    load: wgpu::LoadOp::Clear(wgpu::Color {
                                        r: 0.10,
                                        g: 0.105,
                                        b: 0.12,
                                        a: 1.0,
                                    }),
                                    store: wgpu::StoreOp::Store,
                                },
                            })],
                            depth_stencil_attachment: None,
                            timestamp_writes: None,
                            occlusion_query_set: None,
                            multiview_mask: None,
                        })
                        .forget_lifetime();
                    renderer.render(&mut pass, &primitives, &screen);
                }
                queue.submit(Some(encoder.finish()));
                queue.present(frame);
            }
            _ => surface.configure(device, surface_config),
        }
        for id in &output.textures_delta.free {
            renderer.free_texture(id);
        }
        output.textures_delta.clear();

        outcome
    }
}

impl Form {
    fn ui(&mut self, ui: &mut egui::Ui) -> Action {
        let mut action = Action::None;
        let mut changed = false;

        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.tab, Tab::Font, "Font");
            ui.selectable_value(&mut self.tab, Tab::Colors, "Colors");
            ui.selectable_value(&mut self.tab, Tab::Footer, "Footer");
        });
        ui.separator();
        ui.add_space(4.0);

        match self.tab {
            Tab::Font => {
                ui.label("Family");
                ComboBox::from_id_salt("font-family")
                    .selected_text(self.draft.font.family.clone())
                    .width(360.0)
                    .show_ui(ui, |ui| {
                        ScrollArea::vertical().max_height(260.0).show(ui, |ui| {
                            for name in &self.families {
                                let selected = self.draft.font.family == *name;
                                if ui.selectable_label(selected, name).clicked() {
                                    self.draft.font.family = name.clone();
                                    changed = true;
                                }
                            }
                        });
                    });
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.label("Size");
                    changed |= ui
                        .add(
                            DragValue::new(&mut self.draft.font.size)
                                .range(8.0..=40.0)
                                .speed(0.5)
                                .suffix(" pt"),
                        )
                        .changed();
                });
                ui.horizontal(|ui| {
                    ui.label("Weight");
                    let current = WEIGHTS
                        .iter()
                        .find(|(weight, _)| *weight == self.draft.font.weight)
                        .map_or("Custom", |(_, label)| label);
                    ComboBox::from_id_salt("font-weight")
                        .selected_text(current)
                        .show_ui(ui, |ui| {
                            for (weight, label) in WEIGHTS {
                                if ui
                                    .selectable_value(&mut self.draft.font.weight, weight, label)
                                    .changed()
                                {
                                    changed = true;
                                }
                            }
                        });
                });
                changed |= ui
                    .checkbox(&mut self.draft.font.ligatures, "Ligatures")
                    .changed();
            }
            Tab::Colors => {
                ui.horizontal(|ui| {
                    ui.label("Theme");
                    let current = self
                        .draft
                        .colors
                        .theme
                        .clone()
                        .unwrap_or_else(|| "default".into());
                    ComboBox::from_id_salt("theme")
                        .selected_text(current.clone())
                        .show_ui(ui, |ui| {
                            for name in &self.themes {
                                if ui.selectable_label(current == *name, name).clicked() {
                                    self.draft.colors.theme = Some(name.clone());
                                    changed = true;
                                }
                            }
                        });
                });
                ui.add_space(8.0);
                let fallback = self.theme.clone();
                changed |= color_row(
                    ui,
                    "Foreground",
                    &mut self.draft.colors.foreground,
                    fallback.foreground,
                );
                changed |= color_row(
                    ui,
                    "Background",
                    &mut self.draft.colors.background,
                    fallback.background,
                );
                changed |= color_row(ui, "Cursor", &mut self.draft.colors.cursor, fallback.cursor);
                ui.add_space(8.0);
                ui.label("Palette");
                let mut palette = self
                    .draft
                    .colors
                    .palette
                    .clone()
                    .unwrap_or_else(|| fallback.palette.to_vec());
                let mut palette_changed = false;
                egui::Grid::new("palette")
                    .spacing([6.0, 6.0])
                    .show(ui, |ui| {
                        for row in 0..2 {
                            for col in 0..8 {
                                let index = row * 8 + col;
                                let mut arr =
                                    [palette[index].0, palette[index].1, palette[index].2];
                                if ui.color_edit_button_srgb(&mut arr).changed() {
                                    palette[index] = Rgb(arr[0], arr[1], arr[2]);
                                    palette_changed = true;
                                }
                            }
                            ui.end_row();
                        }
                    });
                if palette_changed {
                    self.draft.colors.palette = Some(palette);
                    changed = true;
                }
                ui.add_space(8.0);
                if ui.button("Reset colors to theme").clicked() {
                    self.draft.colors = Colors {
                        theme: self.draft.colors.theme.clone(),
                        ..Colors::default()
                    };
                    changed = true;
                }
            }
            Tab::Footer => {
                changed |= ui
                    .checkbox(&mut self.draft.footer.enabled, "Show footer")
                    .changed();
                changed |= ui
                    .checkbox(
                        &mut self.draft.prompt.enabled,
                        "oh-my-zsh style prompt (new tabs)",
                    )
                    .changed();
                ui.add_space(4.0);
                ui.label(
                    "The footer shows encoding, shell, folder, git branch, time and terminal size.",
                );
            }
        }

        if changed {
            self.theme = Settings::from_config(&self.draft, &self.config_dir).theme;
        }

        ui.add_space(12.0);
        ui.separator();
        ui.horizontal(|ui| {
            let dirty = self.draft != self.saved;
            if ui.add_enabled(dirty, egui::Button::new("Save")).clicked() {
                action = Action::Save;
            }
            if ui.add_enabled(dirty, egui::Button::new("Revert")).clicked() {
                action = Action::Revert;
            }
            if dirty {
                ui.label("Unsaved changes");
            } else if !self.status.is_empty() {
                ui.label(self.status.clone());
            }
        });
        ui.label(format!("{}", self.config_dir.join("config.toml").display()));
        action
    }

    fn save(&mut self) -> Outcome {
        match config::save_config(&self.config_dir, &self.draft) {
            Ok(()) => {
                self.saved = self.draft.clone();
                self.status = "Saved".into();
                Outcome::Saved(self.saved.clone())
            }
            Err(err) => {
                self.status = format!("Save failed: {err}");
                Outcome::Nothing
            }
        }
    }
}

fn color_row(ui: &mut egui::Ui, label: &str, value: &mut Option<Rgb>, fallback: Rgb) -> bool {
    let current = value.unwrap_or(fallback);
    let mut arr = [current.0, current.1, current.2];
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.add_sized([110.0, 20.0], egui::Label::new(label));
        if ui.color_edit_button_srgb(&mut arr).changed() {
            *value = Some(Rgb(arr[0], arr[1], arr[2]));
            changed = true;
        }
    });
    changed
}
