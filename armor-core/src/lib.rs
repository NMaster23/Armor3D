mod camera;
mod reference_image;
mod render;
mod viewport;
pub mod nurbs;
pub mod gumball;
pub mod exporter;
pub mod objects;
pub mod csg;
pub mod bvh;

use cgmath::{InnerSpace, Quaternion, Vector3};
use glyphon::Family;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use render::State;
use winit::dpi::PhysicalPosition;
use winit::event::MouseButton;
use winit::keyboard::KeyCode;

use crate::exporter::write_file;
use crate::viewport::TextEntity;

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

#[pyfunction]
#[pyo3(name = "handle_input")]
pub fn handle_input(pos: (f64, f64), button: &str, vec: (f32, f32, f32)) -> PyResult<()> {
    let _pos = PhysicalPosition::new(pos.0, pos.1);
    let _mouse_button = match button {
        "left" => MouseButton::Left,
        "middle" => MouseButton::Middle,
        "right" => MouseButton::Right,
        _ => return Err(PyValueError::new_err(format!("Unknown button {}", button))),
    };
    let vector = Vector3::new(vec.0 as f32, vec.1 as f32, vec.2 as f32);
    Ok(())
}

#[pymethods]
impl ViewportRenderer {
    pub fn offset_selected_curve(&mut self, dist: f32) -> Option<usize> {
        self.state.viewport.offset_selected_curve(dist)
    }
    pub fn sweep1_selected_curves(&mut self, closed_caps: Option<bool>) -> Option<usize> {
        self.state.viewport.sweep1_selected_curves(closed_caps.unwrap_or(true))
    }
    pub fn array_linear(&mut self, count: usize, delta: (f32, f32, f32)) -> Vec<usize> {
        self.state.viewport.array_linear_selected(count, Vector3::new(delta.0, delta.1, delta.2))
    }
    pub fn array_polar(
        &mut self,
        count: usize,
        center: (f32, f32, f32),
        axis: (f32, f32, f32),
        angle_deg: Option<f32>,
    ) -> Vec<usize> {
        let angle_rad = angle_deg.unwrap_or(360.0).to_radians();
        self.state.viewport.array_polar_selected(
            count,
            Vector3::new(center.0, center.1, center.2),
            Vector3::new(axis.0, axis.1, axis.2),
            angle_rad,
        )
    }
    pub fn array_grid(
        &mut self,
        count: (usize, usize, usize),
        spacing: (f32, f32, f32),
    ) -> Vec<usize> {
        self.state.viewport.array_grid_selected(
            count.0,
            count.1,
            count.2,
            Vector3::new(spacing.0, spacing.1, spacing.2),
        )
    }
    pub fn section_selected_solid(&mut self, origin: (f32, f32, f32), norm: (f32, f32, f32)) -> Vec<usize> {
        let p_origin = Vector3::new(origin.0, origin.1, origin.2);
        let p_norm = Vector3::new(norm.0, norm.1, norm.2);
        self.state.viewport.section_selected_solid(p_origin, p_norm)
    }
    pub fn contour_selected_solid(
        &mut self,
        start: (f32, f32, f32),
        dir: (f32, f32, f32),
        interval: f32,
        count: usize,
    ) -> Vec<usize> {
        let p_start = Vector3::new(start.0, start.1, start.2);
        let p_dir = Vector3::new(dir.0, dir.1, dir.2);
        self.state.viewport.contour_selected_solid(p_start, p_dir, interval, count)
    }
    pub fn loft_selected_curves(&mut self, closed_caps: Option<bool>) -> Option<usize> {
        self.state.viewport.loft_selected_curves(closed_caps.unwrap_or(true))
    }
    pub fn hollow_selected_solid(&mut self, wall_thickness: f32) -> Option<usize> {
        self.state.viewport.hollow_selected_solid(wall_thickness)
    }
    pub fn scale_selected_solid(&mut self, fx: f32, fy: f32, fz: f32) -> bool {
        self.state.viewport.scale_selected_solid(fx, fy, fz)
    }
    pub fn mirror_selected_solid(&mut self, axis: &str) -> bool {
        self.state.viewport.mirror_selected_solid(axis)
    }
    pub fn import_model(&mut self) -> PyResult<Option<usize>> {
        if let Some((tris, _name)) = pollster::block_on(crate::exporter::read_mesh_file()) {
            let id = self.state.viewport.import_solid_mesh(tris, [0.7, 0.7, 0.75, 1.0]);
            Ok(Some(id))
        } else {
            Ok(None)
        }
    }
    pub fn import_model_from_path(&mut self, path: &str) -> PyResult<Option<usize>> {
        let path_buf = std::path::PathBuf::from(path);
        match crate::exporter::read_mesh_from_path(&path_buf) {
            Ok(tris) => {
                let id = self.state.viewport.import_solid_mesh(tris, [0.7, 0.7, 0.75, 1.0]);
                Ok(Some(id))
            }
            Err(_) => Ok(None),
        }
    }
    pub fn revolve_selected_curve_to_solid(&mut self, degrees: f32, axis: &str) -> Option<usize> {
        self.state.viewport.revolve_selected_curve_to_solid(degrees, axis)
    }
    pub fn start_arc(&mut self) {
        self.state.viewport.start_arc();
    }
    pub fn being_arc_start(&mut self, x: f32, y: f32) -> bool {
        self.state.viewport.begin_arc_start(x, y)
    }
    pub fn set_arc_end(&mut self, x: f32, y: f32) -> bool {
        self.state.viewport.set_arc_end(x, y)
    }
    pub fn update_arc_preview(&mut self, x: f32, y: f32) -> bool {
        self.state.viewport.set_arc_end(x, y)
    }
    pub fn set_arc_degrees(&mut self, deg: f32) -> bool {
        self.state.viewport.set_arc_degrees(deg)
    }
    pub fn commit_arc(&mut self) -> bool {
        self.state.viewport.commit_arc()
    }
    pub fn cancel_arc(&mut self) {
        self.state.viewport.cancel_arc();
    }
    pub fn trim_at(&mut self, mouse_x: f32, mouse_y: f32) -> bool {
        self.state.viewport.trim_at(mouse_x, mouse_y)
    }
    pub fn bool_union(&mut self, id_a: usize, id_b: usize) -> Option<usize> {
        self.state.viewport.bool_union(id_a, id_b)
    }
    pub fn bool_difference(&mut self, id_a: usize, id_b: usize) -> Option<usize> {
        self.state.viewport.bool_difference(id_a, id_b)
    }
    pub fn bool_intersection(&mut self, id_a: usize, id_b: usize) -> Option<usize> {
        self.state.viewport.bool_intersection(id_a, id_b)
    }
    pub fn bool_union_selected(&mut self) -> Option<usize> {
        self.state.viewport.bool_union_selected()
    }
    fn set_white_viewport(&mut self, enabled: bool) {
        self.state.white_viewport = enabled;
        self.state.viewport.set_material_render_mode(enabled);
    }
    pub fn set_object_material(&mut self, object_id: usize, material: Option<String>) -> bool {
        self.state.viewport.set_object_material(object_id, material)
    }
    pub fn bool_difference_selected(&mut self) -> Option<usize> {
        self.state.viewport.bool_difference_selected()
    }
    pub fn bool_intersection_selected(&mut self) -> Option<usize> {
        self.state.viewport.bool_intersection_selected()
    }
    pub fn extrude_selected_curve_to_solid(&mut self, height: f32) -> Option<usize> {
        self.state.viewport.extrude_selected_curve_to_solid(height)
    }
    pub fn export_model(&mut self) -> PyResult<Option<String>> {
        let raw_triangles = self.state.viewport.extract_triangles();
        if raw_triangles.is_empty() {
            return Ok(None);
        }
        let triangles: Vec<exporter::Triangle> = raw_triangles
            .into_iter()
            .map(|triangle| {
                let to_slicer = |p: [f32; 3]| Vector3::new(p[0], -p[2], p[1]);
                let v0 = to_slicer(triangle.v0);
                let v1 = to_slicer(triangle.v1);
                let v2 = to_slicer(triangle.v2);
                let n = (v1 - v0).cross(v2 - v0);
                let normal = if n.magnitude2() > f32::EPSILON {
                    let norm = n.normalize();
                    [norm.x, norm.y, norm.z]
                } else {
                    [0.0, 0.0, 1.0]
                };
                crate::exporter::Triangle {
                    v0: [v0.x, v0.y, v0.z],
                    v1: [v1.x, v1.y, v1.z],
                    v2: [v2.x, v2.y, v2.z],
                    normal,
                }
            }).collect();
        let file_obj = pollster::block_on(write_file(&triangles, "3mf"));
        Ok(file_obj.map(|f| f.full_path.to_string_lossy().to_string()))
    }
    pub fn create_box(&mut self, x: f32, y: f32, z: f32, width: f32, height: f32, depth: f32) -> usize {
        self.state.viewport.add_box([x, y, z], width, height, depth)
    }
    fn select_objects_by_ids(&mut self, object_ids: Vec<usize>) {
        self.state.viewport.select_objects_by_ids(&object_ids);
    }
    fn connect_selected(&mut self) -> usize {
        self.state.viewport.connect_selected()
    }
    fn imported_mesh_data(&self) -> Vec<(usize, bool, usize)> {
        self.state.viewport.imported_meshes
            .iter()
            .map(|mesh| (mesh.id, mesh.selected, mesh.triangles.len()))
            .collect()
    }
    pub fn create_cylinder(&mut self, x: f32, y: f32, z: f32, radius: f32, height: f32, subdivisions: usize) -> usize {
        self.state.viewport.add_cylinder([x, y, z], radius, height, subdivisions)
    }
    pub fn create_sphere(&mut self, x: f32, y: f32, z: f32, radius: f32) -> usize {
        self.state.viewport.add_sphere([x, y, z], radius, 16, 24)
    }
    pub fn create_cone(&mut self, x: f32, y: f32, z: f32, radius: f32, height: f32) -> usize {
        self.state.viewport.add_cone([x, y, z], radius, height, 24)
    }
    pub fn create_torus(&mut self, x: f32, y: f32, z: f32, outer_radius: f32, inner_radius: f32) -> usize {
        self.state.viewport.add_torus([x, y, z], outer_radius, inner_radius, 24, 16)
    }
    pub fn get_gumball_rot_info(&self) -> Option<(String, f32, f32, f32)> {
        let (axis, deg) = self.state.viewport.gumball_rot_info.as_ref()?;
        let gumball = self.state.viewport.gumball.as_ref()?;
        let screen_pos = self.state.viewport.world_to_screen(
            Vector3::new(gumball.origin.x, gumball.origin.y, gumball.origin.z)
        )?;
        Some((axis.clone(), *deg, screen_pos.x, screen_pos.y))
    }
    pub fn clear_gumball_rot_info(&mut self) {
        self.state.viewport.gumball_rot_info = None;
    }
    #[new]
    fn new(hwnd: usize, width: u32, height: u32) -> PyResult<Self> {
        let state = pollster::block_on(State::new_embedded(hwnd as isize, width, height))
            .map_err(|error| pyo3::exceptions::PyRuntimeError::new_err(error.to_string()))?;
        Ok(Self { state })
    }
    fn draw_text(&mut self, text: &str, pos: (f32, f32, f32), size: f32, font_name: &str, fontfamily: &str, depth: Option<f32>) -> bool {
        let entity = TextEntity {
            id: 0,
            selected: false,
            depth: depth.unwrap_or(0.0),
            pos: Vector3::new(pos.0, pos.1, pos.2),
            quat: Quaternion::new(1.0, 0.0, 0.0, 0.0),
            color: self.state.viewport.polyline_color,
            text: text.to_string(),
            size,
            cached_vertices: Vec::new(),
        };
        let font_family = match fontfamily.to_ascii_lowercase().as_str() {
            "serif" => Family::Serif,
            "mono" => Family::Monospace,
            "cursive" => Family::Cursive,
            "fantasy" => Family::Fantasy,
            "sans" => Family::SansSerif,
            _ => Family::Name(fontfamily),
        };
        self.state.add_text(entity, font_name, font_family)
    }
    fn get_fonts(&mut self) -> Vec<String> {
        self.state.get_all_fonts()
    }
    fn join_selected(&mut self) -> usize {
        self.state.viewport.join_selected()}
    fn explode_selected(&mut self) -> usize {
        self.state.viewport.explode_selected()}
    fn selected_geometry_count(&self) -> usize {
        self.state
            .viewport
            .entities
            .iter()
            .filter(|entity| entity.selected)
            .count()
            + self
                .state
                .viewport
                .nurbs_surfaces
                .iter()
                .filter(|surface| surface.selected)
                .count()
            + self.state.viewport.text_entities
                .iter()
                .filter(|text| text.selected)
                .count()
            + self
                .state
                .viewport
                .imported_meshes
                .iter()
                .filter(|mesh| mesh.selected)
                .count()
    }
    fn rotate_objects(
        &mut self,
        object_ids: Vec<usize>,
        degrees: f32,
        mode: &str,
    ) -> usize {
        self.state
            .viewport
            .rotate_objects_by_ids(&object_ids, degrees, mode)
    }
    fn rotate_selected_objects(&mut self, degrees: f32, mode: &str)-> usize {
        self.state
            .viewport
            .rotate_selected_objects(degrees, mode)
    }
    fn undo(&mut self) -> bool {
        self.state.undo_scene()
    }
    fn set_view(&mut self, view: &str) -> bool {
        match view {
            "top" => self.state.viewport.set_top_view(),
            "perspective" => self.state.viewport.set_perspective_view(),
            "front" => self.state.viewport.set_front_view(),
            "right" => self.state.viewport.set_right_view(),
            _=> return false,
        }
        true
    }
    fn import_reference_image(&mut self, path: &str, opacity: f32) -> PyResult<()> {
        self.state
            .import_reference_image(path, opacity)
            .map_err(|error| pyo3::exceptions::PyRuntimeError::new_err(error.to_string()))
    }
    fn set_reference_image_rect(&mut self, first: (f32, f32), second: (f32, f32)) -> bool {
        self.state.set_reference_image_rect(first, second)
    }
    fn revolve_selected(&mut self, degrees: f32, axis: &str) -> usize {
        self.state.viewport.revolve_selected(degrees, axis)
    }
    fn revolve_selected_around(
        &mut self,
        degrees: f32,
        axis_start: (f32, f32, f32),
        axis_end: (f32, f32, f32)
    ) -> usize {
        self.state.viewport.revolve_selected_around(
            degrees,
            cgmath::Vector3::new(axis_start.0, axis_start.1, axis_start.2),
            cgmath::Vector3::new(axis_end.0, axis_end.1, axis_end.2)
        )
    }
    fn revolve_object(&mut self, object_id: usize, degrees: f32, axis: &str) -> usize {
        self.state
            .viewport
            .revolve_object(object_id, degrees, axis)
    }
    fn revolve_object_around(
        &mut self,
        object_id: usize,
        degrees: f32,
        axis_start: (f32, f32, f32), 
        axis_end: (f32, f32, f32)
    ) -> usize {
        self.state.viewport.revolve_object_around(
            object_id,
            degrees,
            cgmath::Vector3::new(axis_start.0, axis_start.1, axis_start.2), cgmath::Vector3::new(axis_end.0, axis_end.1, axis_end.2)
        )
    }
    fn start_rectangle(&mut self) {
        self.state.viewport.start_rectangle();
    }
    fn begin_rectangle(&mut self, x: f32, y: f32) -> bool {
        self.state.viewport.begin_rectangle(x, y)
    }
    fn update_rectangle_preview(&mut self, x: f32, y: f32) -> Option<(f32, f32)> {
        self.state.viewport.update_rectangle_preview(x, y)
    }
    fn commit_rectangle(&mut self) -> bool {
        self.state.viewport.commit_rectangle()
    }
    fn cancel_rectangle(&mut self) {
        self.state.viewport.cancel_rectangle();
    }
    fn remove_reference_image(&mut self) {
        self.state.clear_reference_image();
    }
    fn start_curve(&mut self) {
        self.state.viewport.start_curve();
    }
    fn finish_curve(&mut self) {
        self.state.viewport.finish_curve();
    }
    fn cancel_curve(&mut self) {
        self.state.viewport.cancel_curve();
    }
    fn redo(&mut self) -> bool {
        self.state.redo_scene()
    }
    fn begin_circle(&mut self, x: f32, y: f32) -> bool {
        self.state.viewport.begin_circle(x, y)
    }
    fn update_circle_preview(&mut self, x: f32, y: f32) -> Option<f32> {
        self.state.viewport.update_circle_preview(x, y)
    }
    fn commit_circle(&mut self, subdivisions: usize) -> bool {
        self.state.viewport.commit_circle(subdivisions)
    }
    fn set_circle_radius(&mut self, radius: f32) -> bool {
        self.state.viewport.set_circle_radius(radius)
    }
    fn cancel_circle(&mut self) {
        self.state.viewport.cancel_circle();
    }
    fn mirror_selected(&mut self, start: (f32, f32), end: (f32, f32)) -> usize {
        self.state.viewport.mirror_selected(
            cgmath::Vector2::new(start.0, start.1),
            cgmath::Vector2::new(end.0, end.1),
        )
    }
    fn clear(&mut self) {
        self.state.viewport.clear();
    }

    fn resize(&mut self, width: u32, height: u32) {
        self.state.resize(width, height);
    }

    fn draw_circle_mouse(
        &mut self,
        subdivisions: usize,
        mouse_pos: (f64, f64),
        mouse_button: &str,
    ) {
        let pos = PhysicalPosition::new(mouse_pos.0, mouse_pos.1);
        let button = match mouse_button {
            "left" => MouseButton::Left,
            "middle" => MouseButton::Middle,
            "right" => MouseButton::Right,
            _ => return,
        };
        self.state
            .viewport
            .draw_circle_mouse(subdivisions, pos, button);
    }

    fn draw_circle_command(
        &mut self,
        subdivisions: usize,
        radius: f32,
        circle_pos: (f32, f32, f32),
    ) {
        let pos = Vector3::new(circle_pos.0, circle_pos.1, circle_pos.2);
        self.state
            .viewport
            .draw_circle_command(subdivisions, radius, pos);
    }

    fn draw_curve(&mut self, subdivisions: usize) {
        self.state.viewport.draw_curve(subdivisions);
    }
    fn add_sample_nurbs_surface(&mut self) -> usize {
        self.state.viewport.add_sample_nurbs_surface()
    }
    fn duplicate_selected(&mut self, x: f32, y: f32, z: f32) -> usize {
        self.state
            .viewport
            .duplicate_selected(cgmath::Vector3::new(x, y, z))
    }

    fn begin_copy(&mut self) -> usize {
        self.state.viewport.begin_copy()
    }

    fn set_copy_base(&mut self, x: f32, y: f32) -> bool {
        self.state.viewport.set_copy_base(x, y)
    }

    fn update_copy_preview(&mut self, x: f32, y: f32) -> Option<f32> {
        self.state.viewport.update_copy_preview(x, y)
    }

    fn place_copy(&mut self) -> usize {
        self.state.viewport.place_copy()
    }

    fn cancel_copy(&mut self) {
        self.state.viewport.cancel_copy();
    }

    pub fn render(&mut self) -> PyResult<()> {
        let mut show_popup = false;
        let mut extrude_input = 0.0;
        self.state
            .render(&mut show_popup, &mut extrude_input)
            .map_err(|error| pyo3::exceptions::PyRuntimeError::new_err(error.to_string()))
    }
    fn add_point(&mut self, x: f32, y: f32, z: f32) {
        self.state.viewport.add_point(cgmath::Vector3::new(x, y, z));
    }
    fn import_polyline(
        &mut self,
        vertices: Vec<(f32, f32, f32)>,
        color: (f32, f32, f32, f32),
        closed: bool,
    ) -> bool {
        if vertices.len() < 2 {
            return false;
        }
        let mut points: Vec<cgmath::Vector3<f32>> = vertices
            .iter()
            .map(|&(x, y, z)| cgmath::Vector3::new(x, y, z))
            .collect();

        if closed && vertices.first() != vertices.last() {
            points.push(points[0]);
        }
        self.state
            .viewport
            .add_polyline(points, [color.0, color.1, color.2, color.3], 2.5);

        true
    }
    fn import_mesh_triangles(
        &mut self, triangles: Vec<((f32, f32, f32), (f32, f32, f32), (f32, f32, f32))>,
        color: (f32, f32, f32, f32)
    ) -> usize {
        use cgmath::Vector3;
        let triangles: Vec<[Vector3<f32>; 3]> = triangles
            .into_iter()
            .map(|(a, b, c)| [
                Vector3::new(a.0, a.1, a.2),
                Vector3::new(b.0, b.1, b.2),
                Vector3::new(c.0, c.1, c.2),
            ])
            .collect();
        self.state.viewport.add_triangle_surfaces(
            &triangles,
            [color.0, color.1, color.2, color.3],
        )
    }
    fn hatch_selected(&mut self) -> usize {
        self.state.viewport.hatch_selected()
    }
    fn hatch_objects(&mut self, object_ids: Vec<usize>) -> usize {
        self.state.viewport.hatch_objects(&object_ids)
    }
    fn begin_extrude_drag(&mut self) -> bool {
        self.state.viewport.begin_extrude_drag()
    }
    fn preview_extrude_drag(&mut self, delta_y: f32) -> bool {
        self.state.viewport.preview_extrude_drag(delta_y)
    }
    fn end_extrude_drag(&mut self, commit:bool) {
        self.state.viewport.end_extrude_drag(commit);
    }
    fn extrude(&mut self, height: f32) -> usize {
        self.state.viewport.extrude_selected(height)
    }
    fn extrude_object(&mut self, object_id: usize, height: f32) -> bool {
        self.state.viewport.extrude_object(object_id, height)
    }
    fn mouse_move(&mut self, x: f64, y: f64) -> Option<String> {
        self.state.viewport.mouse_move(x, y)
    }

    fn mouse_button(&mut self, pressed: bool) -> bool {
        self.state.viewport.mouse_button(pressed)
    }
    fn linear_array(&mut self, count: u32, offset: (f32, f32, f32)) -> usize {
        self.state
            .viewport
            .linear_array(count, Vector3::new(offset.0, offset.1, offset.2))
    }
    fn polar_array(&mut self, count: u32, center: (f32, f32, f32)) -> usize {
        self.state
            .viewport
            .polar_array(count, Vector3::new(center.0, center.1, center.2))
    }
    #[pyo3(signature = (x, y, additive=false))]
    fn select_picture_at(&mut self, x: f32, y: f32, additive: bool) -> bool {
        self.state.select_reference_at(x, y, additive)
    }
    fn select_picture_box(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, additive: bool) -> bool {
        self.state.select_reference_box(x1, y1, x2, y2, additive)
    }
    fn clear_picture_selection(&mut self) {
        self.state.clear_reference_selection();
    }
    fn select_at(&mut self, x: f32, y: f32, additive: bool) -> Option<usize> {
        self.state
            .viewport
            .select_shape(cgmath::Vector2::new(x, y), 8.0, additive)
    }
    fn pick_imported_mesh_point(
        &self, x: f32, y:f32
    ) -> Option<(usize, (f32, f32, f32))> {
        let (id, point) = self.state.viewport
            .pick_imported_mesh_point(cgmath::Vector2::new(x, y))?;
        Some((id, (point.x, point.y, point.z)))
    }
    fn connect_mesh_holes(
        &mut self, first_id: usize, first_points: Vec<(f32, f32, f32)>, second_id: usize, second_points: Vec<(f32, f32, f32)>,
    ) -> bool {
        if first_points.len() != 3 || second_points.len() != 3 {
            return false;
        }
        let to_vec = |point: &(f32, f32, f32)| {
            cgmath::Vector3::new(point.0, point.1, point.2)
        };
        self.state.viewport.connect_mesh_holes(first_id,
            [
                to_vec(&first_points[0]), 
                to_vec(&first_points[1]),
                to_vec(&first_points[2])
            ],
            second_id,
            [
                to_vec(&second_points[0]),
                to_vec(&second_points[1]),
                to_vec(&second_points[2]),
            ]
        )
    }
    #[pyo3(signature = (
        start_x,
        start_y,
        end_x,
        end_y,
        additive=false
    ))]
    fn select_box(
        &mut self,
        start_x: f32,
        start_y: f32,
        end_x: f32,
        end_y: f32,
        additive: bool,
    ) -> usize {
        self.state
            .viewport
            .select_box(start_x, start_y, end_x, end_y, additive)
    }

    fn delete_selected(&mut self) -> usize {
        self.state.delete_selected_objects()
    }
    fn translate_objects(
        &mut self,
        object_ids: Vec<usize>,
        offset: (f32, f32, f32)
    ) -> usize {
        self.state.viewport.translate_objects(
            &object_ids,
            cgmath::Vector3::new(offset.0, offset.1, offset.2)
        )
    }
    fn scene_surface_data(&self) -> Vec<(usize, (f32, f32, f32), bool)> {
        self.state
            .viewport
            .nurbs_surfaces
            .iter()
            .map(|surface| {
                let(mut x, mut y, mut z, mut count) = (0.0, 0.0, 0.0, 0usize);
                for row in &surface.surface.control_points {
                    for control_point in row {
                        x+= control_point.point.x;
                        y += control_point.point.y;
                        z+= control_point.point.z;
                        count+= 1;
                    }
                }
                let center = if count == 0 {
                    (0.0, 0.0, 0.0)
                } else {
                    (
                        x / count as f32,
                        y / count as f32,
                        z / count as f32,
                    )
                };
                (surface.id, center, surface.selected)
            })
            .collect()
    }
    fn delete_objects(&mut self, object_ids: Vec<usize>) -> usize {
    self.state.viewport.delete_objects(&object_ids)
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
    fn set_polyline_distance(&mut self, distance: Option<f32>) {
        self.state.viewport.set_polyline_distance(distance);
    }
    fn commit_polyline_distance(&mut self, distance: f32) -> bool {
        self.state.viewport.commit_polyline_distance(distance)
    }
    fn finish_polyline(&mut self) {
        self.state.viewport.finish_polyline();
    }

    fn cancel_polyline(&mut self) {
        self.state.viewport.cancel_polyline();
    }

    fn set_osnap_modes(
        &mut self,
        end_enabled: bool,
        near_enabled: bool,
        mid_enabled: bool,
        int_enabled: bool,
    ) {
        self.state
            .viewport
            .set_osnap_modes(end_enabled, near_enabled, mid_enabled, int_enabled);
    }

    fn set_construction_snap(&mut self, enabled: bool) {
        self.state.viewport.set_construction_snap(enabled);
    }

    fn set_grid_snap(&mut self, enabled: bool) {
        self.state.viewport.set_grid_snap(enabled);
    }

    fn set_grid_spacing(&mut self, spacing: f32) {
        self.state.viewport.set_grid_spacing(spacing);
    }
    fn set_ortho(&mut self, enabled: bool) {
        self.state.viewport.set_ortho(enabled);
    }
    fn snap_cursor_position(&mut self) -> Option<(f32, f32)> {
        self.state.viewport.snap_cursor_position()
    }
    fn cursor_world_position(&mut self) -> Option<(f32, f32, f32)> {
        self.state.viewport.cursor_world_position()
    }
    fn scene_data(&self) -> Vec<(usize, Vec<(f32, f32, f32)>, (f32, f32, f32, f32), f32, bool)> {
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
    fn scene_render_triangles(
        &self,
    ) -> Vec<((f32, f32, f32), (f32, f32, f32), (f32, f32, f32))> {
        let mut triangles = Vec::new();

        for entity in &self.state.viewport.entities {
            let vertices = crate::viewport::Viewport::extrude(
                &entity.vertices,
                entity.color,
                entity.height,
                entity.extrude_axis,
            );
            for triangle in vertices.chunks_exact(3) {
                triangles.push((
                    (triangle[0].position[0], triangle[0].position[1], triangle[0].position[2]),
                    (triangle[1].position[0], triangle[1].position[1], triangle[1].position[2]),
                    (triangle[2].position[0], triangle[2].position[1], triangle[2].position[2]),
                ));
            }
        }

        for surface in &self.state.viewport.nurbs_surfaces {
            let vertices = surface.surface.sample_mesh(
                surface.u_subdivisions,
                surface.v_subdivisions,
                surface.color,
            );
            for triangle in vertices.chunks_exact(3) {
                triangles.push((
                    (triangle[0].position[0], triangle[0].position[1], triangle[0].position[2]),
                    (triangle[1].position[0], triangle[1].position[1], triangle[1].position[2]),
                    (triangle[2].position[0], triangle[2].position[1], triangle[2].position[2]),
                ));
            }
        }

        for mesh in &self.state.viewport.imported_meshes {
            triangles.extend(mesh.triangles.iter().map(|triangle| {
                (
                    (triangle[0].x, triangle[0].y, triangle[0].z),
                    (triangle[1].x, triangle[1].y, triangle[1].z),
                    (triangle[2].x, triangle[2].y, triangle[2].z),
                )
            }));
        }

        triangles
    }
    fn scene_hatched_ids(&self) -> Vec<usize> {
        self.state
            .viewport
            .entities
            .iter()
            .filter(|entity| entity.hatched)
            .map(|entity| entity.id)
            .collect()
    }

    fn active_polyline_data(&self) -> Vec<(f32, f32, f32)> {
        if !self.state.viewport.polyline_active {
            return Vec::new();
        }

        let mut points = self.state.viewport.active_polyline.clone();
        if let Some(preview) = self.state.viewport.preview_point {
            if points.last().copied() != Some(preview) {
                points.push(preview);
            }
        }

        points
            .into_iter()
            .map(|point| (point.x, point.y, point.z))
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
        if self.state.viewport.camera.orthographic {
            self.state.viewport.camera.pan(dx, dy);
        } else {
            self.state.viewport.camera.orbit(dx, dy);
        }
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
        let handled = self.state.viewport.camera_controller.handle_key(
            &mut self.state.viewport.camera,
            code,
            pressed,
        );
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
