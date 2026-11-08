mod convert;
mod footer;
mod input;

use std::time::{Duration, Instant};

use config::{Settings, Theme};
use render::{FontSpec, Renderer};
use term::{Session, SpawnOptions};
use winit::application::ApplicationHandler;
use winit::dpi::{LogicalSize, PhysicalPosition, PhysicalSize};
use winit::event::{ElementState, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, ModifiersState, PhysicalKey};
use winit::window::{Window, WindowId};

const FRAME_INTERVAL: Duration = Duration::from_millis(16);

struct State {
    window: std::sync::Arc<Window>,
    renderer: Renderer,
    session: Session,
    theme: Theme,
    footer: Option<footer::Footer>,
    footer_refreshed: Instant,
}

struct App {
    settings: Settings,
    state: Option<State>,
    modifiers: ModifiersState,
    pointer: PhysicalPosition<f64>,
    dragging: bool,
    clipboard: Option<arboard::Clipboard>,
}

impl App {
    fn grid_size(renderer: &Renderer, width: u32, height: u32) -> (u16, u16) {
        let (cell_w, cell_h) = renderer.cell_size();
        let cols = (width as f32 / cell_w).floor().max(1.0) as u16;
        let rows = (height as f32 / cell_h).floor().max(1.0) as u16;
        (cols, rows)
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_some() {
            return;
        }
        let attributes = Window::default_attributes()
            .with_title("yatty")
            .with_inner_size(LogicalSize::new(1100.0, 700.0));
        let window =
            std::sync::Arc::new(event_loop.create_window(attributes).expect("create window"));
        let scale = window.scale_factor() as f32;
        let size = window.inner_size();

        let font = FontSpec {
            family: self.settings.font.family.clone(),
            weight: self.settings.font.weight,
            size_px: self.settings.font.size * scale,
            ligatures: self.settings.font.ligatures,
        };
        let renderer =
            Renderer::new(window.clone(), size.width, size.height, font).expect("renderer");
        let (cols, rows) = Self::grid_size(&renderer, size.width, size.height);
        let reserved = u16::from(self.settings.footer.enabled);
        let session = Session::spawn(&SpawnOptions {
            shell: None,
            cwd: None,
            cols,
            rows: rows.saturating_sub(reserved).max(1),
        })
        .expect("spawn shell");
        let footer = self
            .settings
            .footer
            .enabled
            .then(|| footer::Footer::start(session.pid()));

        window.request_redraw();
        self.state = Some(State {
            window,
            renderer,
            session,
            theme: self.settings.theme.clone(),
            footer,
            footer_refreshed: Instant::now(),
        });
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let Some(state) = self.state.as_mut() else {
            return;
        };
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                state.renderer.resize(size.width, size.height);
                let (cols, rows) = Self::grid_size(&state.renderer, size.width, size.height);
                let reserved = u16::from(state.footer.is_some());
                let _ = state
                    .session
                    .resize(cols, rows.saturating_sub(reserved).max(1));
                state.window.request_redraw();
            }
            WindowEvent::ModifiersChanged(modifiers) => {
                self.modifiers = modifiers.state();
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.pointer = position;
                if self.dragging {
                    let (row, col, right) = cell_at(state, self.pointer);
                    state.session.update_selection(row, col, right);
                    state.window.request_redraw();
                }
            }
            WindowEvent::MouseInput {
                state: button_state,
                button: MouseButton::Left,
                ..
            } => {
                self.dragging = button_state == ElementState::Pressed;
                if self.dragging {
                    let (row, col, right) = cell_at(state, self.pointer);
                    state.session.start_selection(row, col, right);
                } else if state
                    .session
                    .selection_text()
                    .is_none_or(|text| text.is_empty())
                {
                    state.session.clear_selection();
                }
                state.window.request_redraw();
            }
            WindowEvent::KeyboardInput { event, .. } if event.state == ElementState::Pressed => {
                if self.modifiers.super_key() {
                    if let Some(clipboard) = self.clipboard.as_mut() {
                        match event.physical_key {
                            PhysicalKey::Code(KeyCode::KeyC) => {
                                if let Some(text) = state.session.selection_text() {
                                    let _ = clipboard.set_text(text);
                                }
                            }
                            PhysicalKey::Code(KeyCode::KeyV) => {
                                if let Ok(text) = clipboard.get_text() {
                                    let _ = state.session.write(text.as_bytes());
                                }
                            }
                            _ => {}
                        }
                    }
                } else if let Some(bytes) = input::key_bytes(&event, self.modifiers) {
                    state.session.clear_selection();
                    let _ = state.session.write(&bytes);
                }
            }
            WindowEvent::RedrawRequested => {
                let mut grid = convert::to_render_grid(&state.session.snapshot(), &state.theme);
                if let Some(footer) = &state.footer {
                    let line = footer.line(grid.cols, (grid.cols as u16, grid.rows as u16));
                    convert::append_footer(&mut grid, &line, &state.theme);
                }
                state.renderer.draw(&grid);
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        let Some(state) = self.state.as_mut() else {
            return;
        };
        let mut changed = false;
        while state.session.events().try_recv().is_ok() {
            changed = true;
        }
        if state.footer.is_some() && state.footer_refreshed.elapsed() >= Duration::from_secs(1) {
            state.footer_refreshed = Instant::now();
            changed = true;
        }
        if changed {
            state.window.request_redraw();
        }
        event_loop.set_control_flow(ControlFlow::WaitUntil(Instant::now() + FRAME_INTERVAL));
    }
}

fn cell_at(state: &State, pointer: PhysicalPosition<f64>) -> (usize, usize, bool) {
    let PhysicalSize { width, height } = state.window.inner_size();
    let (cols, rows) = App::grid_size(&state.renderer, width, height);
    let rows = rows
        .saturating_sub(u16::from(state.footer.is_some()))
        .max(1);
    let (cell_w, cell_h) = state.renderer.cell_size();
    let x = pointer.x.max(0.0) as f32;
    let y = pointer.y.max(0.0) as f32;
    let col = ((x / cell_w) as usize).min(cols as usize - 1);
    let row = ((y / cell_h) as usize).min(rows as usize - 1);
    let right_half = x - col as f32 * cell_w > cell_w / 2.0;
    (row, col, right_half)
}

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    let settings = Settings::load(&config::default_config_dir());
    for warning in &settings.warnings {
        tracing::warn!("config: {warning}");
    }

    let event_loop = EventLoop::new().expect("event loop");
    let mut app = App {
        settings,
        state: None,
        modifiers: ModifiersState::default(),
        pointer: PhysicalPosition::new(0.0, 0.0),
        dragging: false,
        clipboard: arboard::Clipboard::new().ok(),
    };
    event_loop.run_app(&mut app).expect("run app");
}
