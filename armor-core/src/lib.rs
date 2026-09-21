mod camera;
mod render;

use pyo3::prelude::*;
use render::State;

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
                    offset: (std::mem::size_of::<[f32; 3]>() * 2) as wgpu::BufferAddress,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32x4,
                },
            ],
        }
    }
}

pub const COLOR: [f32; 4] = [200.0 / 255.0, 200.0 / 255.0, 200.0 / 255.0, 1.0];
pub const GRAPH_VERTICES: &[Vertex] = &[
    Vertex { position: [-1000.0, -1000.0, 0.0], coords: [0.0, 1.0, 0.0], color: COLOR },
    Vertex { position: [1000.0, -1000.0, 0.0], coords: [1.0, 1.0, 0.0], color: COLOR },
    Vertex { position: [-1000.0, 1000.0, 0.0], coords: [0.0, 0.0, 0.0], color: COLOR },
    Vertex { position: [1000.0, 1000.0, 0.0], coords: [1.0, 0.0, 0.0], color: COLOR },
    Vertex { position: [-1000.0, 0.0, -1000.0], coords: [0.0, 1.0, 0.0], color: COLOR },
    Vertex { position: [1000.0, 0.0, -1000.0], coords: [1.0, 1.0, 0.0], color: COLOR },
    Vertex { position: [-1000.0, 0.0, 1000.0], coords: [0.0, 0.0, 0.0], color: COLOR },
    Vertex { position: [1000.0, 0.0, 1000.0], coords: [1.0, 0.0, 0.0], color: COLOR },
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

    fn resize(&mut self, width: u32, height: u32) {
        self.state.resize(width, height);
    }

    fn render(&mut self) -> PyResult<()> {
        self.state
            .render()
            .map_err(|error| pyo3::exceptions::PyRuntimeError::new_err(error.to_string()))
    }

    fn add_point(&mut self, x: f32, y: f32, z: f32) {
        self.state.add_point(cgmath::Vector3::new(x, y, z));
    }

    fn mouse_move(&mut self, x: f64, y: f64) {
        self.state.mouse_move(x, y);
    }

    fn mouse_button(&mut self, pressed: bool) {
        self.state.mouse_button(pressed);
    }
}

#[pymodule]
fn armor_core(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<ViewportRenderer>()?;
    Ok(())
}
