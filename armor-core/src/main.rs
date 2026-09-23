mod camera;
mod render;

use crate::render::State;
use std::sync::Arc;
use winit::application::ApplicationHandler;
use winit::dpi::PhysicalPosition;
use winit::event::{ElementState, KeyEvent, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::Window;

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    position: [f32; 3],
    coords: [f32; 3],
    color: [f32; 4],
}

impl Vertex {
    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 3]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: (std::mem::size_of::<[f32; 3]>() * 2) as wgpu::BufferAddress, // Offset 24
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32x4,
                },
            ],
        }
    }
}

pub const COLOR: [f32; 4] = [200.0 / 255.0, 200.0 / 255.0, 200.0 / 255.0, 1.0];

pub const GRAPH_VERTICES: &[Vertex] = &[
    Vertex {
        position: [-1000.0, -1000.0, 0.0],
        coords: [0.0, 1.0, 0.0],
        color: COLOR,
    },
    Vertex {
        position: [1000.0, -1000.0, 0.0],
        coords: [1.0, 1.0, 0.0],
        color: COLOR,
    },
    Vertex {
        position: [-1000.0, 1000.0, 0.0],
        coords: [0.0, 0.0, 0.0],
        color: COLOR,
    },
    Vertex {
        position: [1000.0, 1000.0, 0.0],
        coords: [1.0, 0.0, 0.0],
        color: COLOR,
    },
    Vertex {
        position: [-1000.0, 0.0, -1000.0],
        coords: [0.0, 1.0, 0.0],
        color: COLOR,
    },
    Vertex {
        position: [1000.0, 0.0, -1000.0],
        coords: [1.0, 1.0, 0.0],
        color: COLOR,
    },
    Vertex {
        position: [-1000.0, 0.0, 1000.0],
        coords: [0.0, 0.0, 0.0],
        color: COLOR,
    },
    Vertex {
        position: [1000.0, 0.0, 1000.0],
        coords: [1.0, 0.0, 0.0],
        color: COLOR,
    },
];

pub const GRAPH_INDICES: &[u16] = &[0, 1, 2, 2, 1, 3, 4, 6, 5, 6, 7, 5];

pub fn main() -> anyhow::Result<()> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        env_logger::init();
    }
    #[cfg(target_arch = "wasm32")]
    {
        console_log::init_with_level(log::Level::Info).unwrap_throw();
    }

    let event_loop = EventLoop::with_user_event().build()?;
    #[cfg(not(target_arch = "wasm32"))]
    {
        let mut app = App::new();
        event_loop.run_app(&mut app)?;
    }
    #[cfg(target_arch = "wasm32")]
    {
        let app = App::new(&event_loop);
        event_loop.spawn_app(app);
    }

    Ok(())
}

pub struct App {
    #[cfg(target_arch = "wasm32")]
    proxy: Option<winit::event_loop::EventLoopProxy<State>>,
    state: Option<State>,
    cursor_pos: PhysicalPosition<f64>,
    holding_left: bool,
}

impl App {
    pub fn new(#[cfg(target_arch = "wasm32")] event_loop: &EventLoop<State>) -> Self {
        #[cfg(target_arch = "wasm32")]
        let proxy = Some(event_loop.create_proxy());
        Self {
            state: None,
            #[cfg(target_arch = "wasm32")]
            proxy,
            cursor_pos: PhysicalPosition::new(0.0, 0.0),
            holding_left: false,
        }
    }
}

impl ApplicationHandler<State> for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        #[allow(unused_mut)]
        let mut window_attributes = Window::default_attributes();

        #[cfg(target_arch = "wasm32")]
        {
            use wasm_bindgen::JsCast;
            use winit::platform::web::WindowAttributesExtWebSys;

            const CANVAS_ID: &str = "canvas";

            let window = wgpu::web_sys::window().unwrap_throw();
            let document = window.document().unwrap_throw();
            let canvas = document.get_element_by_id(CANVAS_ID).unwrap_throw();
            let html_canvas_element = canvas.unchecked_into();
            window_attributes = window_attributes.with_canvas(Some(html_canvas_element));
        }

        let window = Arc::new(event_loop.create_window(window_attributes).unwrap());
        let state = pollster::block_on(State::new(window.clone())).expect("State::new");
        self.state = Some(state);
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.state = Some(pollster::block_on(State::new(window)).unwrap());
        }

        #[cfg(target_arch = "wasm32")]
        {
            if let Some(proxy) = self.proxy.take() {
                wasm_bindgen_futures::spawn_local(async move {
                    assert!(
                        proxy
                            .send_event(
                                State::new(window)
                                    .await
                                    .expect("Unable to create canvas!!!")
                            )
                            .is_ok()
                    )
                });
            }
        }
    }

    #[allow(unused_mut)]
    fn user_event(&mut self, _event_loop: &ActiveEventLoop, mut event: State) {
        #[cfg(target_arch = "wasm32")]
        {
            if let Some(window) = &event.window {
                window.request_redraw();
            }
            event.resize(
                event.window.inner_size().width,
                event.window.inner_size().height,
            );
        }
        self.state = Some(event);
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: winit::window::WindowId,
        event: WindowEvent,
    ) {
        let state = match self.state.as_mut() {
            Some(state) => state,
            None => return,
        };
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => state.resize(size.width, size.height),
            WindowEvent::RedrawRequested => match state.render() {
                Ok(_) => {}
                Err(e) => {
                    log::error!("{:?}", e);
                    eprintln!("{:?}", e);
                    event_loop.exit();
                }
            },
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        physical_key: PhysicalKey::Code(code),
                        state: key_state,
                        ..
                    },
                ..
            } => {
                let handled = state.camera_controller.handle_key(
                    &mut state.camera,
                    code,
                    key_state.is_pressed(),
                );
                if handled {
                    if let Some(window) = &state.window {
                        window.request_redraw();
                    }
                }
            },
            WindowEvent::MouseWheel {
                device_id,
                delta,
                phase,
            } => {
                state
                    .camera_controller
                    .handle_scroll(&mut state.camera, &delta);
                if let Some(window) = &state.window {
                    window.request_redraw();
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.cursor_pos = position;
                if self.holding_left {
                    state.drawing(
                        self.cursor_pos,
                        MouseButton::Left,
                        true,
                    );
                }
            }
            WindowEvent::MouseInput { state: mouse_state, button, .. } => {
                if button == MouseButton::Left {
                    self.holding_left = mouse_state == ElementState::Pressed;
                    if self.holding_left {
                        state.drawing(
                            self.cursor_pos,
                            button,
                            mouse_state == ElementState::Pressed,
                        );
                    }
                }
            }
            _ => {}
        }
    }
}