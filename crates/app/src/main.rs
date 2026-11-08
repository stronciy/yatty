mod convert;
mod input;

use std::time::{Duration, Instant};

use render::Renderer;
use term::{Session, SpawnOptions};
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::ModifiersState;
use winit::window::{Window, WindowId};

const FONT_SIZE: f32 = 13.0;
const FRAME_INTERVAL: Duration = Duration::from_millis(16);

struct State {
    window: std::sync::Arc<Window>,
    renderer: Renderer,
    session: Session,
}

#[derive(Default)]
struct App {
    state: Option<State>,
    modifiers: ModifiersState,
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

        let renderer = Renderer::new(window.clone(), size.width, size.height, FONT_SIZE * scale)
            .expect("renderer");
        let (cols, rows) = Self::grid_size(&renderer, size.width, size.height);
        let session = Session::spawn(&SpawnOptions {
            shell: None,
            cwd: None,
            cols,
            rows,
        })
        .expect("spawn shell");

        window.request_redraw();
        self.state = Some(State {
            window,
            renderer,
            session,
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
                let _ = state.session.resize(cols, rows);
                state.window.request_redraw();
            }
            WindowEvent::ModifiersChanged(modifiers) => {
                self.modifiers = modifiers.state();
            }
            WindowEvent::KeyboardInput { event, .. } if event.state == ElementState::Pressed => {
                if let Some(bytes) = input::key_bytes(&event, self.modifiers) {
                    let _ = state.session.write(&bytes);
                }
            }
            WindowEvent::RedrawRequested => {
                let grid = convert::to_render_grid(&state.session.snapshot());
                state.renderer.draw(&grid);
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        let Some(state) = self.state.as_ref() else {
            return;
        };
        let mut changed = false;
        while state.session.events().try_recv().is_ok() {
            changed = true;
        }
        if changed {
            state.window.request_redraw();
        }
        event_loop.set_control_flow(ControlFlow::WaitUntil(Instant::now() + FRAME_INTERVAL));
    }
}

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    let event_loop = EventLoop::new().expect("event loop");
    let mut app = App::default();
    event_loop.run_app(&mut app).expect("run app");
}
