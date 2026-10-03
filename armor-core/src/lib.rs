mod camera;
mod render;
mod viewport;

use pyo3::prelude::*;
use render::State;
use winit::keyboard::KeyCode;

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    pub position: [f32; 3],
    pub coords: [f32; 3],
    pub color: [f32; 4],
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

#[pyclass(unsendable)]
pub struct ViewportRenderer {
    state: State,
}

#[pymethods]
impl ViewportRenderer {
    #[new]
    fn new(hwnd: usize, width: u32, height: u32) -> PyResult<Self> {
        let state = pollster::block_on(State::new_embedded(hwnd as isize, width, height))
            .map_err(|error| pyo3::exceptions::PyRuntimeError::new_err(error.to_string()))?;
        Ok(Self { state })
    }
    fn clear(&mut self) {
        self.state.viewport.clear();
    }

    fn resize(&mut self, width: u32, height: u32) {
        self.state.resize(width, height);
    }

    fn render(&mut self) -> PyResult<()> {
        self.state
            .render()
            .map_err(|error| pyo3::exceptions::PyRuntimeError::new_err(error.to_string()))
    }

    fn add_point(&mut self, x: f32, y: f32, z: f32) {
        self.state.viewport.add_point(cgmath::Vector3::new(x, y, z));
    }
    fn extrude(&mut self, height: f32) {
        self.state.viewport.extrude_selected(height);
    }

    fn mouse_move(&mut self, x: f64, y: f64) -> Option<String> {
        self.state.viewport.mouse_move(x, y)
    }

    fn mouse_button(&mut self, pressed: bool) -> bool {
        self.state.viewport.mouse_button(pressed)
    }

    fn select_at(&mut self, x: f32, y: f32) -> Option<usize> {
        self.state
            .viewport
            .select_shape(cgmath::Vector2::new(x, y), 8.0)
    }

    fn select_box(&mut self, start_x: f32, start_y: f32, end_x: f32, end_y: f32) -> usize {
        self.state
            .viewport
            .select_box(start_x, start_y, end_x, end_y)
    }

    fn delete_selected(&mut self) -> usize {
        self.state.viewport.delete_selected()
    }

    fn begin_move_selected(&mut self, x: f32, y: f32) -> bool {
        self.state.viewport.begin_move_selected(x, y)
    }

    fn move_selected(&mut self, x: f32, y: f32) -> bool {
        self.state.viewport.move_selected(x, y)
    }

    fn end_move_selected(&mut self) {
        self.state.viewport.end_move_selected();
    }

    fn start_polyline(&mut self) {
        self.state.viewport.start_polyline();
    }

    fn finish_polyline(&mut self) {
        self.state.viewport.finish_polyline();
    }

    fn cancel_polyline(&mut self) {
        self.state.viewport.cancel_polyline();
    }

    fn set_osnap_modes(&mut self, end_enabled: bool, near_enabled: bool) {
        self.state
            .viewport
            .set_osnap_modes(end_enabled, near_enabled);
    }

    fn set_grid_snap(&mut self, enabled: bool) {
        self.state.viewport.set_grid_snap(enabled);
    }

    fn set_grid_spacing(&mut self, spacing: f32) {
        self.state.viewport.set_grid_spacing(spacing);
    }

    fn snap_cursor_position(&mut self) -> Option<(f32, f32)> {
        self.state.viewport.snap_cursor_position()
    }
    fn cursor_world_position(&mut self) -> Option<(f32, f32, f32) > {
        self.state.viewport.cursor_world_position()
    }
    fn scene_data(
        &self,
    ) -> Vec<(
        usize,
        Vec<(f32, f32, f32)>,
        (f32, f32, f32, f32),
        f32,
        bool,
    )> {
        self.state
            .viewport
            .entities
            .iter()
            .map(|entity| {
                (
                    entity.id,
                    entity
                        .vertices
                        .iter()
                        .map(|point| (point.x, point.y, point.z))
                        .collect(),
                    (
                        entity.color[0],
                        entity.color[1],
                        entity.color[2],
                        entity.color[3],
                    ),
                    entity.height,
                    entity.selected,
                )
            })
            .collect()
    }

    fn set_polyline_color(&mut self, red: f32, green: f32, blue: f32, alpha: f32) {
        self.state
            .viewport
            .set_polyline_color(red, green, blue, alpha);
    }

    fn pan(&mut self, dx: f32, dy: f32) {
        self.state.viewport.camera.pan(dx, dy);
    }

    fn orbit(&mut self, dx: f32, dy: f32) {
        self.state.viewport.camera.orbit(dx, dy);
    }

    fn zoom(&mut self, steps: f32) {
        self.state.viewport.camera.zoom(steps);
        self.state.viewport.rebuild_vertices();
    }

    fn key_event(&mut self, key: &str, pressed: bool) -> bool {
        let code = match key {
            "w" | "W" => KeyCode::KeyW,
            "a" | "A" => KeyCode::KeyA,
            "s" | "S" => KeyCode::KeyS,
            "d" | "D" => KeyCode::KeyD,
            "c" | "C" => KeyCode::KeyC,
            "e" | "E" => KeyCode::KeyE,
            "g" | "G" => KeyCode::KeyG,
            "Tab" | "tab" => KeyCode::Tab,
            "Up" => KeyCode::ArrowUp,
            "Down" => KeyCode::ArrowDown,
            "Left" => KeyCode::ArrowLeft,
            "Right" => KeyCode::ArrowRight,
            "2" | "KP_2" => KeyCode::Digit2,
            _ => return false,
        };
        if self.state.viewport.graph_handle_key(code, pressed) {
            return true;
        }
        let handled = self.state
            .camera_controller
            .handle_key(&mut self.state.viewport.camera, code, pressed);
        if handled {
            self.state.viewport.rebuild_vertices();
        }
        handled
    }
}

#[pymodule]
fn armor_core(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<ViewportRenderer>()?;
    Ok(())
}
