mod camera;
mod render;
mod viewport;

use crate::render::State;
use std::mem::size_of;
use std::sync::Arc;
use winit::application::ApplicationHandler;
use winit::dpi::PhysicalPosition;
use winit::event::{ElementState, KeyEvent, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{PhysicalKey};
use winit::window::Window;
use cgmath::InnerSpace;
use winit::event_loop;

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
            array_stride: size_of::<Vertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: size_of::<[f32; 3]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: (size_of::<[f32; 3]>() * 2) as wgpu::BufferAddress,
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
    env_logger::init();
    let event_loop = EventLoop::<State>::with_user_event().build()?;
    let mut app = App::new();
    event_loop.run_app(&mut app)?;
    Ok(())
}

pub struct App {
    state: Option<State>,
    cursor_pos: PhysicalPosition<f64>,
    holding_left: bool,
    holding_right: bool,
    popup_window: Option<Arc<Window>>,
    popup_state: Option<State>,
    popup_content: Option<PopupContent>,
}

#[derive(PartialEq)]
pub enum PopupContent {
    TextInput(String),
    ConfirmationMessage(String),
    TextMessage(String),
}

impl App {
    pub fn popup_window(
        &mut self,
        event_loop: &ActiveEventLoop,
        title: &str,
        case: i32,
        ctx: &egui::Context,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if self.popup_window.is_some() {
            return Ok(());
        }
        self.popup_window = Some(Arc::new(event_loop.create_window(Window::default_attributes())?));
        let Some(state) = self.state.as_mut() else {
            return Ok(());
        };
        let viewport_local = &mut state.viewport;
        if case == 1 {
            egui::Window::new(title)
                .resizable(false)
                .collapsible(false)
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Height:");
                        let mut height = viewport_local
                            .selected_entity
                            .and_then(|id| viewport_local.entities.iter().find(|e| e.id == id))
                            .map(|e| e.height)
                            .unwrap_or(0.0);
                        if ui.add(egui::DragValue::new(&mut height).speed(0.1)).changed() {
                            viewport_local.extrude_selected(height);
                        }
                    });
                });
        }
        Ok(())
    }
    pub fn close_popup(&mut self) {
        self.popup_window = None;
        self.popup_state = None;
    }
    pub fn new() -> Self {
        Self {
            state: None,
            cursor_pos: PhysicalPosition::new(0.0, 0.0),
            holding_left: false,
            holding_right: false,
            popup_window: None,
            popup_state: None,
            popup_content: None,
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

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: winit::window::WindowId,
        event: WindowEvent,
    ) {
        if self.popup_window.as_ref().is_some_and(|window| window.id() == window_id) {
            if matches!(&event, WindowEvent::CloseRequested) {
                self.close_popup();
            }
            return;
        }
        if let WindowEvent::KeyboardInput {
            event: KeyEvent {
                physical_key: PhysicalKey::Code(code),
                state: key_state,
                ..
            },
            ..
        } = &event {
            if *code == winit::keyboard::KeyCode::Escape && key_state.is_pressed() {
                event_loop.exit();
                return;
            }
            if *code == winit::keyboard::KeyCode::KeyP && key_state.is_pressed() {
                if let Err(e) = self.popup_window(event_loop, "Popup Window", 1, &egui::Context::default()) {
                    log::error!("Failed to create popup window: {:?}", e);
                }
                return;
            }
        }
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
                if state.viewport.graph_handle_key(code, key_state.is_pressed()) {
                    if let Some(window) = &state.window {
                        window.request_redraw();
                    }
                }
                let handled = state.camera_controller.handle_key(
                    &mut state.viewport.camera,
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
                device_id: _device_id,
                delta,
                phase: _phase,
            } => {
                let _ = &mut state.viewport.camera_controller
                    .handle_scroll(&mut state.camera, &delta);
                if let Some(window) = &state.window {
                    window.request_redraw();
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.cursor_pos = position;
                if self.holding_left {
                    state.viewport.drawing(
                        self.cursor_pos,
                        MouseButton::Left,
                        true,
                    );
                }
            }
            WindowEvent::MouseInput { state: mouse_state, button, .. } => {
                if button == MouseButton::Left {
                    self.holding_left = mouse_state == ElementState::Pressed;
                }
                if let Some(state) = self.state.as_mut() {
                    if mouse_state == ElementState::Pressed {
                        match button {
                            MouseButton::Middle => {
                                let mouse_px = cgmath::Vector2::new(self.cursor_pos.x as f32, self.cursor_pos.y as f32);
                                state.viewport.select_shape(mouse_px, 10.0);
                            }
                            MouseButton::Left => {
                                let cursor = cgmath::vec2(self.cursor_pos.x as f32, self.cursor_pos.y as f32);
                                for (idx, entity) in state.entities.iter().enumerate() {
                                    for vertex in &entity.vertices {
                                        if let Some(screen_pos) = state.viewport.world_to_screen(*vertex) {
                                            let dist = (screen_pos - cursor).magnitude();
                                            if dist < 50.0 {
                                                println!("Debug, Clicked near: {}, Dist: {dist:.1}px", idx);
                                            }
                                        }
                                    }
                                }
                                state.viewport.drawing(
                                    self.cursor_pos,
                                    button,
                                    mouse_state == ElementState::Pressed,
                                );
                                if let Some(window) = &state.window {
                                    window.request_redraw();
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
            _ => {}
        }
    }
}