use crate::bvh::{Bvh, BvhTriangle};
use crate::exporter::Triangle;
use crate::objects::{BaseShape, SolidEntity};
use crate::{Vertex, gumball};
use crate::gumball::Gumball;
use crate::camera::{Camera, CameraController};
use crate::nurbs::{NurbsControlPoint, NurbsCurve, NurbsSurface};
use anyhow::Ok;
use cgmath::{InnerSpace, Point3, Quaternion, Rad, Rotation, Rotation3, SquareMatrix, Vector2, Vector3, Vector4, Zero};
use lyon::lyon_tessellation::{
    BuffersBuilder, FillOptions, FillRule, FillTessellator, FillVertex, VertexBuffers,
};
use lyon::math::point;
use lyon::path::Path;
use std::mem;
use std::collections::HashMap;
use std::sync::Arc;
use winit::dpi::PhysicalPosition;
use winit::event::MouseButton;
use winit::event_loop::ActiveEventLoop;
use winit::keyboard::KeyCode;
use lyon::path::PathEvent;
use lyon::path::iterator::PathIterator;

const END_SNAP_RADIUS_PIXELS: f32 = 12.0;
const NEAR_SNAP_RADIUS_PIXELS: f32 = 10.0;
const POLYLINE_WIDTH_PIXELS: f32 = 2.5;
const POLYLINE_HEIGHT: f32 = 0.002;
const DEFAULT_POLYLINE_COLOR: [f32; 4] = [214.0 / 255.0, 166.0 / 255.0, 64.0 / 255.0, 1.0];

#[derive(Clone, Copy, Debug)]
enum OutlineCommand {
    MoveTo(f32, f32),
    LineTo(f32, f32),
    QuadTo(f32, f32, f32, f32),
    CurveTo(f32, f32, f32, f32, f32, f32),
    Close,
}

#[derive(Default)]
struct Builder {
    commands: Vec<OutlineCommand>,
}

impl ttf_parser::OutlineBuilder for Builder {
    fn move_to(&mut self, x: f32, y: f32) {
        self.commands.push(OutlineCommand::MoveTo(x, y));
    }

    fn line_to(&mut self, x: f32, y: f32) {
        self.commands.push(OutlineCommand::LineTo(x, y));
    }

    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        self.commands.push(OutlineCommand::QuadTo(x1, y1, x, y));
    }

    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        self.commands.push(OutlineCommand::CurveTo(x1, y1, x2, y2, x, y));
    }

    fn close(&mut self) {
        self.commands.push(OutlineCommand::Close);
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct PolyLineVertex {
    position: [f32; 3],
}

#[derive(Clone, Debug)]
pub struct PolyLine {
    pub hatched: bool,
    pub id: usize,
    pub vertices: Vec<Vector3<f32>>,
    pub color: [f32; 4],
    pub thickness: f32,
    pub selected: bool,
    pub height: f32,
    pub extrude_axis: Vector3<f32>,
    pub show_markers: bool,
}

#[derive(Clone, Debug)]
pub struct NurbsCurveEntity {
    pub id: usize,
    pub curve: NurbsCurve,
    pub color: [f32; 4],
    pub thickness: f32,
    pub selected: bool,
    pub show_control_points: bool,
}

#[derive(Clone, Debug)]
pub struct NurbsSurfaceEntity {
    pub id: usize,
    pub surface: NurbsSurface,
    pub color: [f32; 4],
    pub selected: bool,
    pub u_subdivisions: usize,
    pub v_subdivisions: usize,
}

#[derive(Clone, Debug)]
pub struct ImportedMeshEntity {
    pub id: usize,
    pub triangles: Arc<Vec<[Vector3<f32>; 3]>>,
    pub color: [f32; 4],
    pub selected: bool,
}

#[derive(Clone, Debug)]
pub struct TextEntity {
    pub id: usize,
    pub selected: bool,
    pub depth: f32,
    pub pos: Vector3<f32>,
    pub quat: Quaternion<f32>,
    pub color: [f32; 4],
    pub text: String,
    pub size: f32,
    pub cached_vertices: Vec<Vertex>,
}

pub struct ShapeUndoStore<T> {
    current: Vec<T>,
    undo: Vec<Vec<T>>,
    redo: Vec<Vec<T>>,
}

impl<T: Clone> ShapeUndoStore<T> {
    pub fn new(current: Vec<T>) -> Self {
        Self {
            current,
            undo: Vec::new(),
            redo: Vec::new(),
        }
    }
    pub fn save_undo(&mut self) {
        self.undo.push(self.current.clone());
        self.redo.clear();
    }
    pub fn undo(&mut self) -> bool {
        if let Some(prev) = self.undo.pop() {
            let old = mem::replace(&mut self.current, prev);
            self.redo.push(old);
            true
        } else {
            false
        }
    }
    pub fn redo(&mut self) -> bool {
        if let Some(next) = self.redo.pop() {
            let old = mem::replace(&mut self.current, next);
            self.undo.push(old);
            true
        } else {
            false
        }
    }
}
pub struct Viewport {
    extrude_drag: Option<(usize, f32, Vector3<f32>)>,
    pub hist_entities: ShapeUndoStore<PolyLine>,
    pub hist_surfaces: ShapeUndoStore<NurbsSurfaceEntity>,
    pub hist_imported_meshes: ShapeUndoStore<ImportedMeshEntity>,
    pub hist_solids: ShapeUndoStore<SolidEntity>,
    pub entities: Vec<PolyLine>,
    pub connected_groups: Vec<Vec<usize>>,
    pub active_polyline: Vec<Vector3<f32>>,
    pub preview_point: Option<Vector3<f32>>,
    pub selected_entity: Option<usize>,
    pub osnap: bool,
    pub end_snap_enabled: bool,
    pub near_snap_enabled: bool,
    pub mid_snap_enabled: bool,
    pub int_snap_enabled: bool,
    pub construction_snap_enabled: bool,
    pub grid_snap_enabled: bool,
    pub grid_spacing: f32,
    pub ortho_enabled: bool,
    pub cursor_pos: PhysicalPosition<f64>,
    pub holding_left: bool,
    pub polyline_active: bool,
    pub polyline_color: [f32; 4],
    pub curve_subdivisions: u32,
    pub curve_active: bool,
    pub polyline_distance: Option<f32>,
    pub move_anchor: Option<Vector3<f32>>,
    pub move_surface_snapshots: Vec<(usize, NurbsSurface)>,
    pub move_mesh_snapshots: Vec<(usize, Arc<Vec<[Vector3<f32>; 3]>>)>,
    pub move_text_snapshots: Vec<(usize, Vector3<f32>, Quaternion<f32>, Vec<Vertex>)>,
    pub move_axis_snapshots: Vec<(usize, Vector3<f32>)>,
    pub move_snapshots: Vec<(usize, Vec<Vector3<f32>>)>,
    pub move_solid_snapshots: Vec<(usize, [f32; 3])>,
    pub point_vertices: Vec<Vertex>,
    pub next_entity: usize,
    pub camera: Camera,
    pub saved_perspective_view: Option<(cgmath::Point3<f32>, cgmath::Point3<f32>,Vector3<f32>,)>,
    pub camera_controller: CameraController,
    pub width: u32,
    pub front_view: bool,
    pub right_view: bool,
    pub right_grid_center: Vector3<f32>,
    pub material_render_mode: bool,
    pub object_materials: HashMap<usize, String>,
    pub height: u32,
    pub redraw: bool,
    pub grab_mode: bool,
    pub grab_origin: Option<Vector3<f32>>,
    pub grab_snapshot: Vec<Vector3<f32>>,
    pub circle_center: Option<Vector3<f32>>,
    pub arc_start: Option<Vector3<f32>>,
    pub arc_end: Option<Vector3<f32>>,
    pub arc_cursor: Option<Vector3<f32>>,
    pub arc_degrees: f32,
    pub circle_target: Option<Vector3<f32>>,
    pub rectangle_first: Option<Vector3<f32>>,
    pub rectangle_second: Option<Vector3<f32>>,
    pub copy_sources: Vec<PolyLine>,
    pub copy_base: Option<Vector3<f32>>,
    pub copy_target: Option<Vector3<f32>>,
    pub text_entities: Vec<TextEntity>,
    pub nurbs_curves: Vec<NurbsCurveEntity>,
    pub nurbs_surfaces: Vec<NurbsSurfaceEntity>,
    pub imported_meshes: Vec<ImportedMeshEntity>,
    pub solids: Vec<SolidEntity>,
    pub gumball: Option<Gumball>,
    pub gumball_rot_info: Option<(String, f32)>,
}

impl Viewport {
    pub fn offset_curve_points(points: &[Vector3<f32>], dist: f32) -> Option<Vec<Vector3<f32>>> {
        if points.len() < 2 {
            return None;
        }
        let is_closed = (points.first().unwrap() - points.last().unwrap()).magnitude2() < 1e-6;
        let pts = if is_closed && points.len() > 2 {
            &points[..points.len() - 1]
        } else {
            points
        };
        let n = pts.len();
        let mut seg_norms: Vec<Vector3<f32>> = Vec::with_capacity(n);
        let seg_count = if is_closed {
            n
        } else {
            n - 1
        };
        for i in 0..seg_count {
            let next_i = (i + 1) % n;
            let dir = pts[next_i] - pts[i];
            let len = dir.magnitude();
            if len < 1e-6 {
                seg_norms.push(Vector3::unit_y());
            } else {
                let tan = dir / len;
                seg_norms.push(Vector3::new(-tan.y, tan.x, 0.0));
            }
        }
        let mut offset_pts: Vec<Vector3<f32>> = Vec::with_capacity(pts.len());
        for i in 0..n {
            let (n_prev, n_next) = if is_closed {
                let prev_i = if i == 0 {
                    n - 1
                } else {
                    i - 1
                };
                (seg_norms[prev_i], seg_norms[i])
            } else {
                if i == 0 {
                    offset_pts.push(pts[0] + seg_norms[0] * dist);
                    continue;
                } else if i == n - 1 {
                    offset_pts.push(pts[n - 1] + seg_norms[seg_norms.len() - 1] * dist);
                    continue;
                }
                (seg_norms[i - 1], seg_norms[i])
            };
            let miter = n_prev + n_next;
            let miter_len_sq = miter.magnitude2();
            if miter_len_sq < 1e-6 {
                offset_pts.push(pts[i] + n_next * dist);
            } else {
                let miter_dir = miter.normalize();
                let cos_half = miter_dir.dot(n_next);
                let scale = if cos_half.abs() > 0.1 {
                    (dist / cos_half).clamp(-5.0 * dist.abs(), 5.0 * dist.abs())
                } else {
                    dist
                };
                offset_pts.push(pts[i] + miter_dir * scale);
            }
        }
        if is_closed && !offset_pts.is_empty() {
            offset_pts.push(offset_pts[0]);
        }
        Some(offset_pts)
    }
    pub fn offset_selected_curve(&mut self, dist: f32) -> Option<usize> {
        let sel_id = self.selected_entity?;
        let curve = self.entities.iter().find(|e| e.id == sel_id)?.clone();
        let offset_verts = Self::offset_curve_points(&curve.vertices, dist)?;
        let id = self.next_entity;
        self.next_entity += 1;
        self.save_undo_state();
        self.entities.push(crate::viewport::PolyLine {
            id,
            vertices: offset_verts,
            color: [1.0, 0.8, 0.2, 1.0],
            thickness: curve.thickness,
            selected: true,
            height: curve.height,
            hatched: false,
            extrude_axis: curve.extrude_axis,
            show_markers: false,
        });
        self.selected_entity = Some(id);
        self.rebuild_vertices();
        Some(id)
    }
    pub fn duplicate_selected(&mut self, offset: Vector3<f32>) -> usize {
        let selected: Vec<PolyLine> = self
            .entities
            .iter()
            .filter(|entity| entity.selected)
            .cloned()
            .collect();
        if selected.is_empty() {
            return 0;
        }
        for entity in &mut self.entities {
            entity.selected = false;
        }
        let mut first_new_id = None;
        for source in selected {
            let duplicated_vertices: Vec<Vector3<f32>> = source
                .vertices
                .iter()
                .map(|point| *point + offset)
                .collect();
            let id = self.next_entity;
            self.next_entity += 1;
            if first_new_id.is_none() {
                first_new_id = Some(id);
            }
            self.entities.push(PolyLine {
                id,
                vertices: duplicated_vertices,
                color: source.color,
                thickness: source.thickness,
                selected: true,
                height: source.height,
                extrude_axis: source.extrude_axis,
                show_markers: source.show_markers,
                hatched: source.hatched,
            });
        }
        self.selected_entity = first_new_id;
        self.rebuild_vertices();
        self.entities
            .iter()
            .filter(|entity| entity.selected)
            .count()
    }
    fn text_hit_test(&self, text: &TextEntity, mouse: Vector2<f32>) -> bool {
        for triangle in text.cached_vertices.chunks_exact(3) {
            let point = |vertex: &Vertex| {
                self.world_to_screen(Vector3::new(
                    vertex.position[0],
                    vertex.position[1],
                    vertex.position[2],
                ))
            };
            if let (Some(a), Some(b), Some(c)) = (
                point(&triangle[0]), point(&triangle[1]), point(&triangle[2])) {
                    if Self::point_in_triangle(mouse, a, b, c) {
                        return true;
                    }
                }
        }
        false
    }
    pub fn join_selected(&mut self) -> usize {
        let selected: Vec<PolyLine> = self.entities
            .iter()
            .filter(|entity| entity.selected && entity.vertices.len() >= 2)
            .cloned()
            .collect();
        if selected.len() < 2 || selected.iter().any(|entity| {
            Self::is_closed_shape(&entity.vertices)
        }) {
            return 0;
        }
        let tolerance_sq = 1.0e-6;
        let same_point = |a: Vector3<f32>, b: Vector3<f32>| {
            (a-b).magnitude2() <= tolerance_sq
        };
        let mut joined = selected[0].vertices.clone();
        let mut remaining = selected[1..].to_vec();
        while !remaining.is_empty() {
            let start = joined[0];
            let end = *joined.last().unwrap();
            let mut match_piece = None;
            for (index, piece) in remaining.iter().enumerate() {
                let piece_start = piece.vertices[0];
                let piece_end = *piece.vertices.last().unwrap();
                if same_point(end, piece_start) {
                    match_piece = Some((index, 0));
                    break;
                } else if same_point(end, piece_end) {
                    match_piece = Some((index, 1));
                    break;
                } else if same_point(start, piece_end) {
                    match_piece = Some((index, 2));
                    break;
                } else if same_point(start, piece_start) {
                    match_piece = Some((index, 3));
                    break;
                }
            }
            let Some((index, direction)) = match_piece else {
                return 0;
            };
            let mut piece = remaining.remove(index);
            match direction {
                0 => {
                    piece.vertices[0] = end;
                    joined.extend(piece.vertices.into_iter().skip(1));
                }
                1=> {
                    let last = piece.vertices.len() - 1;
                    piece.vertices[last] = end;
                    joined.extend(piece.vertices.into_iter().rev().skip(1));
                }
                2 => {
                    let last = piece.vertices.len() - 1;
                    piece.vertices[last] = start;
                    piece.vertices.pop();
                    piece.vertices.extend(joined);
                    joined = piece.vertices;
                }
                _ => {
                    piece.vertices[0] = start;
                    let mut prefix: Vec<_> =
                        piece.vertices.into_iter().rev().skip(1).collect();
                    prefix.extend(joined);
                    joined = prefix;
                }
            }
        }
        let source = &selected[0];
        let selected_ids: Vec<usize> =
            selected.iter().map(|entity| entity.id).collect();
        self.save_undo_state();
        self.entities.retain(|entity| !selected_ids.contains(&entity.id));
        let id = self.push_polyline_entity (
            joined, source.color, source.thickness, source.height, source.extrude_axis, source.show_markers, true, false);
        self.selected_entity = Some(id);
        self.rebuild_vertices();
        selected.len()
    }
    pub fn explode_selected(&mut self) -> usize {
        let selected_polylines: Vec<PolyLine> = self.entities
            .iter()
            .filter(|entity| entity.selected)
            .cloned()
            .collect();
        let selected_curves: Vec<NurbsCurveEntity> = self.nurbs_curves
            .iter()
            .filter(|curve| curve.selected)
            .cloned()
            .collect();
        let segment_count: usize = selected_polylines
            .iter()
            .map(|entity| entity.vertices.len().saturating_sub(1))
            .sum::<usize>()
            + selected_curves
                .iter()
                .map(|curve| curve.curve.sample(64).len().saturating_sub(1))
                .sum::<usize>();

        if segment_count == 0 {
            return 0;
        }

        self.save_undo_state();
        self.entities.retain(|entity| !entity.selected);
        self.nurbs_curves.retain(|curve| !curve.selected);
        let mut new_entities = Vec::with_capacity(segment_count);
        let mut first_new_id = None;
        let mut created = 0usize;

        for source in selected_polylines {
            for pair in source.vertices.windows(2) {
                let id = self.next_entity;
                self.next_entity += 1;
                first_new_id.get_or_insert(id);
                new_entities.push(PolyLine {
                    hatched: false,
                    id,
                    vertices: vec![pair[0], pair[1]],
                    color: source.color,
                    thickness: source.thickness,
                    selected: true,
                    height: source.height,
                    extrude_axis: source.extrude_axis,
                    show_markers: source.show_markers,
                });
                created += 1;
            }
        }

        for source in selected_curves {
            let points = source.curve.sample(64);
            for pair in points.windows(2) {
                let id = self.push_polyline_entity(
                    vec![pair[0], pair[1]],
                    source.color,
                    source.thickness,
                    0.0,
                    Vector3::unit_y(),
                    false,
                    true,
                    false,
                );
                first_new_id.get_or_insert(id);
                created += 1;
            }
        }

        self.entities.extend(new_entities);
        self.selected_entity = first_new_id;
        self.rebuild_vertices();
        created
    }
    pub fn rotate_selected_objects(&mut self, degrees: f32, mode: &str) -> usize {
        let object_ids: Vec<usize> = self
            .entities
            .iter()
            .filter(|entity| entity.selected)
            .map(|entity| entity.id)
            .chain(
                self.nurbs_surfaces
                    .iter()
                    .filter(|surface| surface.selected)
                    .map(|surface| surface.id)
            )
            .chain(
                self.imported_meshes
                    .iter()
                    .filter(|mesh| mesh.selected)
                    .map(|mesh| mesh.id)
            )
            .collect();
        self.rotate_objects_by_ids(&object_ids, degrees, mode)
    }
    pub fn rotate_objects_by_ids(
        &mut self,
        object_ids: &[usize],
        degrees: f32,
        mode: &str,
    ) -> usize {
        if object_ids.is_empty()
            || !degrees.is_finite()
            || degrees == 0.0
            || degrees.abs() > 360.0
        {
            return 0;
        }
        let axis: Vector3<f32> = match mode {
            "x" => Vector3::unit_x(),
            "y" => Vector3::unit_y(),
            "z" => Vector3::unit_z(),
            "xy" => {
                if self.front_view {
                    Vector3::unit_z()
                } else{
                    Vector3::unit_y()
                }
            }
            _=> return 0,
        };
        let mut points = Vec::new();
        for entity in self.entities.iter().filter(|entity| object_ids.contains(&entity.id)) {
            points.extend(entity.vertices.iter().copied());
        }
        for surface in self
            .nurbs_surfaces
            .iter()
            .filter(|surface| object_ids.contains(&surface.id))
        {
            for row in &surface.surface.control_points {
                points.extend(row.iter().map(|control_point| control_point.point));
            }
        }
        for mesh in self
            .imported_meshes
            .iter()
            .filter(|mesh| object_ids.contains(&mesh.id))
        {
            points.extend(mesh.triangles.iter().flat_map(|triangle| triangle.iter().copied()));
        }
        if points.is_empty() {
            return 0;
        }
        let min_x = points.iter().map(|p| p.x).fold(f32::INFINITY, f32::min);
        let min_y = points.iter().map(|p| p.y).fold(f32::INFINITY, f32::min);
        let min_z = points.iter().map(|p| p.z).fold(f32::INFINITY, f32::min);
        let max_x = points.iter().map(|p| p.x).fold(f32::NEG_INFINITY, f32::max);
        let max_y = points.iter().map(|p| p.y).fold(f32::NEG_INFINITY, f32::max);
        let max_z = points.iter().map(|p| p.z).fold(f32::NEG_INFINITY, f32::max);
        let pivot = Vector3::new(
            (min_x + max_x) * 0.5,
            (min_y + max_y) * 0.5,
            (min_z + max_z) * 0.5,
        );
        let half_angle = degrees.to_radians() * 0.5;
        let sin_half = half_angle.sin();
        let rotation = Quaternion::new(
            half_angle.cos(),
            axis.x * sin_half, axis.y * sin_half, axis.z * sin_half,
        );
        let polyline_count = self
            .entities
            .iter()
            .filter(|entity| object_ids.contains(&entity.id))
            .count();
        let surface_count = self
            .nurbs_surfaces
            .iter()
            .filter(|surface| object_ids.contains(&surface.id))
            .count();
        let mesh_count = self
            .imported_meshes
            .iter()
            .filter(|mesh| object_ids.contains(&mesh.id))
            .count();
        if polyline_count + surface_count + mesh_count == 0 {
            return 0;
        }
        self.save_undo_state();
        for entity in self.entities.iter_mut().filter(|entity| object_ids.contains(&entity.id)) {
            for point in &mut entity.vertices {
                *point = pivot + rotation.rotate_vector(*point - pivot);
            }
            entity.extrude_axis = rotation.rotate_vector(entity.extrude_axis);
        }
        for surface in self
            .nurbs_surfaces
            .iter_mut()
            .filter(|surface| object_ids.contains(&surface.id))
        {
            for row in &mut surface.surface.control_points {
                for control_point in row {
                    control_point.point =
                        pivot + rotation.rotate_vector(control_point.point - pivot);
                }
            }
        }
        for mesh in self
            .imported_meshes
            .iter_mut()
            .filter(|mesh| object_ids.contains(&mesh.id))
        {
            mesh.triangles = Arc::new(
                mesh.triangles
                    .iter()
                    .map(|triangle| triangle.map(|point| pivot + rotation.rotate_vector(point - pivot)))
                    .collect(),
            );
        }
        self.rebuild_vertices();
        polyline_count + surface_count + mesh_count
    }
    fn is_closed_shape(vertices: &[Vector3<f32>]) -> bool {
        vertices.len() >= 4 && (vertices[0] - vertices[vertices.len() - 1]).magnitude2() < 0.0001
    }
    fn extrusion_axis_for_vertices(
        vertices: &[Vector3<f32>],
        view_direction: Vector3<f32>,
        fallback_axis: Vector3<f32>,
    ) -> Vector3<f32> {
    let origin = vertices[0];
    for i in 1..vertices.len() - 1 {
        for j in i + 1..vertices.len() {
            let cross = (vertices[i] - origin).cross(vertices[j] - origin);
            if cross.magnitude2() > f32::EPSILON {
                let mut normal = cross.normalize();
                if view_direction.magnitude2() > f32::EPSILON
                    && normal.dot(view_direction) < 0.0
                {
                    normal = -normal;
                }
                return normal;
            }
        }
        }
        fallback_axis
    }
    pub fn revolve_object(&mut self, object_id: usize, degrees: f32, axis: &str) -> usize {
        if !self.entities.iter().any(|entity| {
            entity.id == object_id && Self::is_closed_shape(&entity.vertices)
        }) {
            return 0;
        }

        let previous_selection: Vec<usize> = self
            .entities
            .iter()
            .filter(|entity| entity.selected)
            .map(|entity| entity.id)
            .collect();

        for entity in &mut self.entities {
            entity.selected = entity.id == object_id;
        }

        let created = self.revolve_selected(degrees, axis);

        for entity in &mut self.entities {
            entity.selected = previous_selection.contains(&entity.id);
        }
        self.selected_entity = previous_selection.first().copied().or_else(|| {
            self.nurbs_surfaces
                .iter()
                .find(|surface| surface.selected)
                .map(|surface| surface.id)
        });
        self.rebuild_vertices();
        created
    }
    pub fn revolve_object_around(
        &mut self,
        object_id: usize,
        degrees: f32,
        axis_start: Vector3<f32>,
        axis_end: Vector3<f32>,
    ) -> usize {
        if !self.entities.iter().any(|entity| {
            entity.id == object_id && Self::is_closed_shape(&entity.vertices)
        }) {
            return 0;
        }
        let previous_selection: Vec<usize> = self
            .entities
            .iter()
            .filter(|entity| entity.selected)
            .map(|entity| entity.id)
            .collect();
        for entity in &mut self.entities {
            entity.selected = entity.id == object_id;
        }
        let created = 
            self.revolve_selected_around(degrees, axis_start, axis_end);
        for entity in &mut self.entities {
            entity.selected = previous_selection.contains(&entity.id);
        }
        self.selected_entity = previous_selection.first().copied();
        self.rebuild_vertices();
        created
    }
    pub fn hatch_selected(&mut self) -> usize {
        let ids: Vec<usize> = self
            .entities
            .iter()
            .filter(|entity| entity.selected)
            .map(|entity| entity.id)
            .collect();
        self.hatch_objects(&ids)
    }
    pub fn hatch_objects(&mut self, ids: &[usize]) -> usize {
        let hatchable_ids: Vec<usize> = self
            .entities
            .iter()
            .filter(|entity| {
                ids.contains(&entity.id)
                    && !entity.hatched
                    && Self::is_closed_shape(&entity.vertices)
            })
            .map(|entity| entity.id)
            .collect();
        if hatchable_ids.is_empty() {
            return 0;
        }
        self.save_undo_state();
        for entity in &mut self.entities {
            if hatchable_ids.contains(&entity.id) {
                entity.hatched = true;
            }
        }
        self.rebuild_vertices();
        hatchable_ids.len()
    }
    pub fn begin_copy(&mut self) -> usize {
        self.copy_sources = self
            .entities
            .iter()
            .filter(|entity| entity.selected)
            .cloned()
            .collect();
        self.copy_base = None;
        self.copy_target = None;
        self.rebuild_vertices();
        self.copy_sources.len()
    }
    pub fn revolve_selected(&mut self, degrees: f32, axis: &str) -> usize {
        if axis != "x" && axis != "y" && axis != "z" {
            return 0;}
        if !degrees.is_finite() || degrees == 0.0 || degrees.abs() > 360.0 {
            return 0;
        }
        let sources: Vec<PolyLine> = self
            .entities
            .iter()
            .filter(|entity| {
                entity.selected && Self::is_closed_shape(&entity.vertices)
            })
            .cloned()
            .collect();
        let source_ids: Vec<usize> = sources.iter().map(|source| source.id).collect();
        if sources.is_empty() {
            return 0;
        }
        self.save_undo_state();
        let angle_steps = ((degrees.abs() / 5.0).ceil() as usize).clamp(12, 144);
        let profile_angle = degrees.to_radians();
        let mut created = 0;
        for source in sources {
            let profile_count = source.vertices.len() - 1;
            if profile_count < 3 {
                continue;
            }
            let profile_rows = profile_count + 1;
            let angular_columns = angle_steps + 1;
            let mut control_points = Vec::with_capacity(profile_rows);
            for row in 0..profile_rows {
                let point = source.vertices[row % profile_count];
                let mut angular_row = Vec::with_capacity(angular_columns);
                for column in 0..angular_columns {
                    let t = column as f32 / angle_steps as f32;
                    let angle = profile_angle * t;
                    let (sin_angle, cos_angle) = angle.sin_cos();
                    let revolved_point = match axis {
                        "x" => Vector3::new(
                            point.x,
                            point.y * cos_angle - point.z * sin_angle,
                            point.y * sin_angle + point.z * cos_angle,
                        ),
                        "y" => Vector3::new(
                            point.x * cos_angle + point.z * sin_angle,
                            point.y,
                            -point.x * sin_angle + point.z * cos_angle,
                        ),
                        "z" => Vector3::new(
                            point.x * cos_angle - point.y * sin_angle,
                            point.x * sin_angle + point.y * cos_angle,
                            point.z,
                        ),
                        _ => return 0,
                    };
                    angular_row.push(NurbsControlPoint {
                        point: revolved_point,
                        weight: 1.0,
                    });
                }
                control_points.push(angular_row);
            }
            let surface = NurbsSurface {
                degree_u: 1,
                degree_v: 1,
                knots_u: NurbsCurve::clamped_knot_vector(profile_rows, 1), 
                knots_v: NurbsCurve::clamped_knot_vector(angular_columns, 1),
                control_points
            };
            let id = self.next_entity;
            self.next_entity += 1;
            self.nurbs_surfaces.push(NurbsSurfaceEntity {
                id, surface,
                color: source.color,
                selected: true,
                u_subdivisions: (profile_rows * 4).clamp(12, 128),
                v_subdivisions: angle_steps
            });
            created += 1;
        }
        if created > 0 {
            self.entities.retain(|entity| !source_ids.contains(&entity.id));
            self.selected_entity = self
                .nurbs_surfaces
                .iter()
                .find(|surface| surface.selected)
                .map(|surface| surface.id);
            self.rebuild_vertices();
        }
        created
    }
    pub fn revolve_selected_around(
        &mut self,
        degrees: f32,
        axis_start: Vector3<f32>,
        axis_end: Vector3<f32>,
    ) -> usize {
        let axis_delta = axis_end - axis_start;
        let axis_length = axis_delta.magnitude();
        if !degrees.is_finite()
            || degrees == 0.0
            || degrees.abs() > 360.0
            || !axis_length.is_finite()
            || axis_length <= 0.0001
        {
            return 0;
        }
        let axis = axis_delta / axis_length;
        let sources: Vec<PolyLine> = self
            .entities
            .iter()
            .filter(|entity| entity.selected && Self::is_closed_shape(&entity.vertices))
            .cloned()
            .collect();
        let source_ids: Vec<usize> = sources.iter().map(|source| source.id).collect();
        if sources.is_empty() {return 0;}
        self.save_undo_state();
        let angle_steps = ((degrees.abs() / 5.0).ceil() as usize).clamp(12, 144);
        let profile_angle = degrees.to_radians();
        let mut created = 0;
        for source in sources {
            let profile_count = source.vertices.len() - 1;
            if profile_count < 3 {
                continue;
            }
            let profile_rows = profile_count + 1;
            let angular_columns = angle_steps + 1;
            let mut control_points = Vec::with_capacity(profile_rows);
            for row in 0..profile_rows {
                let point = source.vertices[row % profile_count];
                let relative = point - axis_start;
                let mut angular_row = Vec::with_capacity(angular_columns);
                for column in 0..angular_columns {
                    let angle = profile_angle * (column as f32 / angle_steps as f32);
                    let (sin_angle, cos_angle) = angle.sin_cos();
                    let rotated = relative * cos_angle
                        + axis.cross(relative) * sin_angle
                        + axis * (axis.dot(relative) * (1.0 - cos_angle));
                    angular_row.push(NurbsControlPoint {
                        point: axis_start + rotated,
                        weight: 1.0,
                    });
                }
                control_points.push(angular_row);
            }
            let surface = NurbsSurface {
                degree_u: 1,
                degree_v: 1,
                knots_u: NurbsCurve::clamped_knot_vector(profile_rows, 1),
                knots_v: NurbsCurve::clamped_knot_vector(angular_columns, 1),
                control_points
            };
            let id = self.next_entity;
            self.next_entity += 1;
            self.nurbs_surfaces.push(NurbsSurfaceEntity {
                id, surface, color: source.color,
                selected: true, u_subdivisions: (profile_rows * 4).clamp(12, 128), v_subdivisions: angle_steps
            });
            created += 1;
        }
        if created > 0 {
            self.entities.retain(|entity| !source_ids.contains(&entity.id));
            self.selected_entity = self
                .nurbs_surfaces
                .iter()
                .find(|surface| surface.selected)
                .map(|surface| surface.id);
            self.rebuild_vertices();
        }
        created
    }
    pub fn set_copy_base(&mut self, x: f32, y: f32) -> bool {
        let screen = PhysicalPosition::new(x as f64, y as f64);
        let Some(raw_point) = self.fetch_point(screen) else {
            return false;
        };
        let point = self.get_snap_pos(raw_point).0;
        self.copy_base = Some(point);
        self.copy_target = Some(point);
        self.rebuild_vertices();
        true
    }

    pub fn update_copy_preview(&mut self, x: f32, y: f32) -> Option<f32> {
        let base = self.copy_base?;
        self.cursor_pos = PhysicalPosition::new(x as f64, y as f64);
        let raw_point = self.fetch_point(self.cursor_pos)?;
        let target = self.get_snap_pos(raw_point).0;
        self.copy_target = Some(target);
        self.rebuild_vertices();

        let offset = target - base;
        Some((offset.x * offset.x + offset.z * offset.z).sqrt())
    }

    pub fn place_copy(&mut self) -> usize {
        let Some(base) = self.copy_base else {
            return 0;
        };
        let Some(target) = self.copy_target else {
            return 0;
        };
        let offset = target - base;
        if offset.magnitude2() <= f32::EPSILON || self.copy_sources.is_empty() {
            return 0;
        }

        let sources = self.copy_sources.clone();
        self.save_undo_state();

        for source in sources {
            let id = self.next_entity;
            self.next_entity += 1;
            self.entities.push(PolyLine {
                id,
                vertices: source
                    .vertices
                    .iter()
                    .map(|point| *point + offset)
                    .collect(),
                color: source.color,
                thickness: source.thickness,
                selected: false,
                height: source.height,
                extrude_axis: source.extrude_axis,
                show_markers: source.show_markers,
                hatched: source.hatched,
            });
        }

        self.rebuild_vertices();
        self.copy_sources.len()
    }

    pub fn cancel_copy(&mut self) {
        self.copy_sources.clear();
        self.copy_base = None;
        self.copy_target = None;
        self.rebuild_vertices();
    }

    fn save_undo_state(&mut self) {
        self.hist_entities.current = self.entities.clone();
        self.hist_entities.save_undo();
        self.hist_surfaces.current = self.nurbs_surfaces.clone();
        self.hist_surfaces.save_undo();
        self.hist_imported_meshes.current = self.imported_meshes.clone();
        self.hist_imported_meshes.save_undo();
        self.hist_solids.current = self.solids.clone();
        self.hist_solids.save_undo();
    }
    fn restore_history_state(&mut self) {
        self.selected_entity = self
            .entities
            .iter()
            .find(|entity| entity.selected)
            .map(|entity| entity.id)
            .or_else(|| {
                self.nurbs_surfaces
                    .iter()
                    .find(|surface| surface.selected)
                    .map(|surface| surface.id)
            })
            .or_else(|| {
                self.imported_meshes
                    .iter()
                    .find(|mesh| mesh.selected)
                    .map(|mesh| mesh.id)
            });
        self.next_entity = self
            .entities
            .iter()
            .map(|entity| entity.id)
            .chain(self.nurbs_surfaces.iter().map(|surface| surface.id))
            .chain(self.imported_meshes.iter().map(|mesh| mesh.id))
            .chain(self.solids.iter().map(|s| s.id))
            .max()
            .map(|id| id + 1)
            .unwrap_or(0);
        self.active_polyline.clear();
        self.preview_point = None;
        self.polyline_active = false;
        self.circle_center = None;
        self.circle_target = None;
        self.rectangle_first = None;
        self.rectangle_second = None;
        self.copy_sources.clear();
        self.copy_base = None;
        self.copy_target = None;
        self.rebuild_vertices();
    }
    pub fn undo_scene(&mut self) -> bool {
        if self.polyline_active {
            if self.active_polyline.len() <= 1 {
                return false;
            }

            self.active_polyline.pop();
            self.preview_point = None;
            self.rebuild_vertices();
            return true;
        }

        self.hist_entities.current = self.entities.clone();
        self.hist_surfaces.current = self.nurbs_surfaces.clone();
        self.hist_imported_meshes.current = self.imported_meshes.clone();
        self.hist_solids.current = self.solids.clone();
        let h_e = self.hist_entities.undo();
        let h_su = self.hist_surfaces.undo();
        let h_i = self.hist_imported_meshes.undo();
        let h_so = self.hist_solids.undo();
        if !h_e && !h_su && !h_i && !h_so {
            return false;
        }
        self.entities = self.hist_entities.current.clone();
        self.nurbs_surfaces = self.hist_surfaces.current.clone();
        self.imported_meshes = self.hist_imported_meshes.current.clone();
        self.solids = self.hist_solids.current.clone();
        self.restore_history_state();
        true
    }
    pub fn redo_scene(&mut self) -> bool {
        self.hist_entities.current = self.entities.clone();
        self.hist_surfaces.current = self.nurbs_surfaces.clone();
        self.hist_imported_meshes.current = self.imported_meshes.clone();
        self.hist_solids.current = self.solids.clone();
        let h_e = self.hist_entities.redo();
        let h_su = self.hist_surfaces.redo();
        let h_i = self.hist_imported_meshes.redo();
        let h_so = self.hist_solids.redo();
        if !h_e && !h_su && !h_i && !h_so {
            return false;
        }
        self.entities = self.hist_entities.current.clone();
        self.nurbs_surfaces = self.hist_surfaces.current.clone();
        self.imported_meshes = self.hist_imported_meshes.current.clone();
        self.solids = self.hist_solids.current.clone();
        self.restore_history_state();
        true
    }
    pub fn mirror_selected(
        &mut self,
        start: cgmath::Vector2<f32>,
        end: cgmath::Vector2<f32>,
    ) -> usize {
        let direction = end - start;
        let length_squared = direction.x * direction.x + direction.y * direction.y;
        if length_squared <= f32::EPSILON {
            return 0;
        }

        let selected: Vec<PolyLine> = self
            .entities
            .iter()
            .filter(|entity| entity.selected)
            .cloned()
            .collect();
        if selected.is_empty() {
            return 0;
        }
        self.save_undo_state();
        for entity in &mut self.entities {
            entity.selected = false;
        }
        let mut first_new_id = None;

        for source in selected {
            let mirrored_vertices = source
                .vertices
                .iter()
                .map(|point| {
                    let relative_x = point.x - start.x;
                    let relative_z = point.z - start.y;

                    let projection =
                        (relative_x * direction.x + relative_z * direction.y) / length_squared;

                    let projected_x = start.x + projection * direction.x;
                    let projected_z = start.y + projection * direction.y;

                    cgmath::Vector3::new(
                        2.0 * projected_x - point.x,
                        point.y,
                        2.0 * projected_z - point.z,
                    )
                })
                .collect();

            let id = self.next_entity;
            self.next_entity += 1;

            if first_new_id.is_none() {
                first_new_id = Some(id);
            }
            self.entities.push(PolyLine {
                id,
                vertices: mirrored_vertices,
                color: source.color,
                thickness: source.thickness,
                selected: true,
                height: source.height,
                extrude_axis: source.extrude_axis,
                show_markers: source.show_markers,
                hatched: source.hatched,
            });
        }

        self.selected_entity = first_new_id;
        self.rebuild_vertices();

        self.entities
            .iter()
            .filter(|entity| entity.selected)
            .count()
    }
    pub fn new(width: u32, height: u32) -> Self {
        let camera = Camera {
            eye: (0.0, 1.0, 2.0).into(),
            target: (0.0, 0.0, 0.0).into(),
            up: cgmath::Vector3::unit_y(),
            aspect: width as f32 / height.max(1) as f32,
            fov_y: 45.0,
            z_near: 0.1,
            z_far: 100.0,
            orthographic: false,
        };
        let camera_controller = CameraController::new(0.02);
        Self {
            hist_entities: ShapeUndoStore::new(Vec::new()),
            hist_surfaces: ShapeUndoStore::new(Vec::new()),
            hist_imported_meshes: ShapeUndoStore::new(Vec::new()),
            hist_solids: ShapeUndoStore::new(Vec::new()),
            entities: Vec::new(),
            connected_groups: Vec::new(),
            active_polyline: Vec::new(),
            preview_point: None,
            selected_entity: None,
            osnap: true,
            extrude_drag: None,
            end_snap_enabled: false,
            near_snap_enabled: false,
            mid_snap_enabled: false,
            int_snap_enabled: false,
            construction_snap_enabled: false,
            saved_perspective_view: None,
            front_view: false,
            right_view: false,
            right_grid_center: Vector3::zero(),
            material_render_mode: false,
            object_materials: HashMap::new(),
            grid_snap_enabled: false,
            grid_spacing: 0.1,
            ortho_enabled: false,
            cursor_pos: PhysicalPosition::new(0.0, 0.0),
            holding_left: false,
            polyline_active: false,
            polyline_color: DEFAULT_POLYLINE_COLOR,
            polyline_distance: None,
            curve_subdivisions: 24,
            curve_active: false,
            move_anchor: None,
            move_snapshots: Vec::new(),
            move_solid_snapshots: Vec::new(),
            move_surface_snapshots: Vec::new(),
            move_mesh_snapshots: Vec::new(),
            move_text_snapshots: Vec::new(),
            point_vertices: Vec::new(),
            next_entity: 0,
            camera,
            camera_controller,
            width,
            height,
            redraw: true,
            grab_mode: false,
            grab_origin: None,
            move_axis_snapshots: Vec::new(),
            grab_snapshot: Vec::new(),
            circle_center: None,
            arc_start: None,
            arc_end: None,
            arc_cursor: None,
            arc_degrees: 90.0,
            circle_target: None,
            rectangle_first: None,
            rectangle_second: None,
            copy_sources: Vec::new(),
            copy_base: None,
            copy_target: None,
            text_entities: Vec::new(),
            nurbs_curves: Vec::new(),
            nurbs_surfaces: Vec::new(),
            imported_meshes: Vec::new(),
            solids: Vec::new(),
            gumball: None,
            gumball_rot_info: None,
        }
    }
}

impl Viewport {
    pub fn extrude_step(&mut self, step: f32) {
        let index = if let Some(selected_id) = self.selected_entity {
            self.entities
                .iter()
                .position(|entity| entity.id == selected_id)
        } else {
            self.entities.len().checked_sub(1)
        };
        let Some(index) = index else {
            return;
        };
        let fallback_axis = if self.right_view {
            Vector3::unit_x()
        } else if self.front_view {
            Vector3::unit_z()
        } else {
            Vector3::unit_y()  
        };
        let view_direction = self.camera.eye - self.camera.target;
        let axis = Self::extrusion_axis_for_vertices(
            &self.entities[index].vertices,
            view_direction,
            fallback_axis
        );
        let entity = &mut self.entities[index];
        entity.extrude_axis = axis;
        entity.height += step;
        self.rebuild_vertices();
    }
    fn connected_ids(&self, start_id: usize) -> Vec<usize> {
        let mut ids = vec![start_id];
        loop {
            let old_len = ids.len();
            for group in &self.connected_groups {
                if group.iter().any(|id| ids.contains(id)) {
                    for id in group {
                        if !ids.contains(id) {
                            ids.push(*id);
                        }
                    }
                }
            }
            if ids.len() == old_len {
                break;
            }
        }
        ids
    }
    pub fn connect_selected(&mut self)  -> usize {
        let mut ids: Vec<usize> = self.entities.iter()
            .filter(|item| item.selected)
            .map(|item| item.id)
            .chain(self.nurbs_surfaces.iter()
                .filter(|item| item.selected)
                .map(|item| item.id))
            .chain(self.imported_meshes.iter()
                .filter(|item| item.selected)
                .map(|item| item.id))
            .collect();
        ids.sort_unstable();
        ids.dedup();
        if ids.len() <2 {
            return 0;}
        let mut merged = ids;
        let mut remaining_groups = Vec::new();
        for group in self.connected_groups.drain(..) {
            if group.iter().any(|id| merged.contains(id)) {
                merged.extend(group);
            } else {
                remaining_groups.push(group);
            }
        }
        merged.sort_unstable();
        merged.dedup();
        for item in &mut self.entities {
            item.selected = merged.contains(&item.id);
        }
        for item in &mut self.nurbs_surfaces {
            item.selected = merged.contains(&item.id);
        }
        for item in &mut self.imported_meshes {
            item.selected = merged.contains(&item.id);
        }
        let count = merged.len();
        remaining_groups.push(merged);
        self.connected_groups = remaining_groups;
        self.rebuild_vertices();
        count
    }
    pub fn set_top_view(&mut self) {
        if !self.camera.orthographic {
            self.saved_perspective_view = Some((
                self.camera.eye,
                self.camera.target,
                self.camera.up
            ));
        }
        self.right_view = false;
        self.front_view = false;
        let distance = (self.camera.eye - self.camera.target).magnitude().max(2.0);
        self.camera.eye = cgmath::Point3::new(
            self.camera.target.x,
            self.camera.target.y + distance,
            self.camera.target.z
        );
        self.camera.up = -Vector3::unit_z();
        self.camera.orthographic = true;
        self.rebuild_vertices();
    }
    pub fn set_right_view(&mut self) {
        if !self.camera.orthographic {
            self.saved_perspective_view = Some((
                self.camera.eye, self.camera.target, self.camera.up
            ));
        }
        self.front_view = false;
        self.right_view = true;
        let distance = (self.camera.eye - self.camera.target).magnitude().max(2.0);
        self.camera.eye = cgmath::Point3::new(
            self.camera.target.x + distance,
            self.camera.target.y,
            self.camera.target.z);
        self.camera.up = Vector3::unit_y();
        self.camera.orthographic = true;
        self.rebuild_vertices();
    }
    pub fn set_front_view(&mut self) {
        if !self.camera.orthographic {
            self.saved_perspective_view = Some((
                self.camera.eye,
                self.camera.target, self.camera.up,
            ));
        }
        self.right_view = false;
        self.front_view = true;
        let distance = (self.camera.eye - self.camera.target).magnitude().max(2.0);
        self.camera.eye = cgmath::Point3::new(
            self.camera.target.x,
            self.camera.target.y,
            self.camera.target.z + distance
        );
        self.camera.up = Vector3::unit_y();
        self.camera.orthographic = true;
        self.rebuild_vertices();
    }
    pub fn set_perspective_view(&mut self) {
        self.front_view = false;
        self.right_view = false;
        if let Some((eye, target, up)) = self.saved_perspective_view.take() {
            self.camera.eye = eye;
            self.camera.target = target;
            self.camera.up = up;
        } else {
            self.camera.eye = self.camera.target + Vector3::new(0.0, 2.0, 4.0);
            self.camera.up = Vector3::unit_y()
        }
        self.camera.orthographic = false;
        self.rebuild_vertices();
    }
    pub fn draw_text(
        &mut self,
        text: String,
        pos: Vector3<f32>,
        quat: Quaternion<f32>,
        color: [f32; 4],
        size: f32,
        depth: f32,
    ) -> bool {
        if text.is_empty()
            || !size.is_finite()
            || size <= 0.0
            || !pos.x.is_finite()
            || !pos.y.is_finite()
            || !pos.z.is_finite()
            || !quat.s.is_finite()
            || !quat.v.x.is_finite()
            || !quat.v.y.is_finite()
            || !quat.v.z.is_finite()
            || quat.is_zero()
            || color.iter().any(|c| !c.is_finite())
            || !depth.is_finite()
            || depth < 0.0
        {
            return false;
        }
        let id = self.next_entity;
        self.next_entity += 1;
        self.text_entities.push(TextEntity {
            id,
            depth,
            pos,
            selected: false,
            quat: quat.normalize(),
            color,
            text,
            size,
            cached_vertices: Vec::new(),
        });
        self.rebuild_vertices();
        true
    }
    fn point_in_triangle(
        p: cgmath::Vector2<f32>,
        a: cgmath::Vector2<f32>,
        b: cgmath::Vector2<f32>,
        c: cgmath::Vector2<f32>,
    ) -> bool {
        let cross = |p1: cgmath::Vector2<f32>,
                     p2: cgmath::Vector2<f32>,
                     p3: cgmath::Vector2<f32>| {
            (p2.x - p1.x) * (p3.y - p1.y) - (p2.y - p1.y) * (p3.x - p1.x)
        };
        let d1 = cross(a, b, p);
        let d2 = cross(b, c, p);
        let d3 = cross(c, a, p);
        let has_negative = d1 < 0.0 || d2 < 0.0 || d3 < 0.0;
        let has_positive = d1 > 0.0 || d2 > 0.0 || d3 > 0.0;
        !(has_negative && has_positive)
    }

    fn surface_hit_test(
        &self,
        surface: &NurbsSurfaceEntity,
        mouse: cgmath::Vector2<f32>,
    ) -> Option<f32> {
        let ray = self.camera.screen_to_ray(
            mouse,
            Vector2::new(self.width as f32, self.height as f32)
        )?;
        let vertices = surface.surface.sample_mesh(
            surface.u_subdivisions,
            surface.v_subdivisions,
            surface.color,
        );
        let mut min_t: Option<f32> = None;
        for triangle in vertices.chunks_exact(3) {
            let p0 = Point3::new(triangle[0].position[0], triangle[0].position[1], triangle[0].position[2]);
            let p1 = Point3::new(triangle[1].position[0], triangle[1].position[1], triangle[1].position[2]);
            let p2 = Point3::new(triangle[2].position[0], triangle[2].position[1], triangle[2].position[2]);
            if let Some(t) = ray.intersect_triangle(p0, p1, p2) {
                if let Some(cur) = min_t {
                    if t < cur {
                        min_t = Some(t);
                    }
                } else {
                    min_t = Some(t);
                }
            }
        }
        min_t
    }
    pub fn pick_imported_mesh_point(
        &self, mouse: Vector2<f32>
    ) -> Option<(usize, Vector3<f32>)> {
        let ray = self.camera.screen_to_ray(
            mouse, Vector2::new(self.width as f32, self.height as f32)
        )?;
        let mut nearest: Option<(usize, f32)> = None;
        for mesh in &self.imported_meshes {
            for triangle in mesh.triangles.iter() {
                let [a, b, c] = *triangle;
                let hit = ray.intersect_triangle(
                    Point3::new(a.x, a.y, a.z), Point3::new(b.x, b.y, b.z), Point3::new(c.x, c.y, c.z)
                );
                if let Some(distance) = hit {
                    if nearest.map(|(_, best)| distance < best).unwrap_or(true) {
                        nearest = Some((mesh.id, distance));
                    }
                }
            }
        }
        let (id, distance) = nearest?;
        let point = ray.origin + ray.direction * distance;
        Some((id, Vector3::new(point.x, point.y, point.z)))
    }
    fn circle_from_three_points(
        points: [Vector3<f32>; 3], 
    )-> Option<(Vector3<f32>, Vector3<f32>)> {
        let [a, b, c] = points;
        let u = b-a;
        let v = c-a;
        let normal = u.cross(v);
        let normal_sq = normal.magnitude2();
        if normal_sq < 1.0e-10 {
            return None;
        }
        let center = a
            + (v.cross(normal) * u.magnitude2()
                + normal.cross(u) * v.magnitude2())
                / (2.0 * normal_sq);
        Some((center, normal.normalize()))
    }
    pub fn connect_mesh_holes(
        &mut self, first_id: usize, first_points: [Vector3<f32>; 3], second_id: usize, second_points: [Vector3<f32>; 3]
    ) -> bool {
        if first_id == second_id {
            return false;
        }
        let Some((first_center, first_axis)) = Self::circle_from_three_points(first_points)
        else {
            return false;
        };
        let Some((second_center, second_axis)) = Self::circle_from_three_points(second_points)
        else {
            return false;
        };
        let first_radius = (first_points[0] - first_center).magnitude();
        let second_radius= (second_points[0]-second_center).magnitude();
        if first_radius <= 1.0e-6
            || second_radius <= 1.0e-6
            || (first_radius- second_radius).abs()>first_radius.max(second_radius)* 0.2
        {
            return false;
        }
        let target_axis = -first_axis;
        let dot = second_axis.dot(target_axis).clamp(-1.0, 1.0);
        let cross = second_axis.cross(target_axis);
        let rotation = if dot < -0.99999 {
            let basis = if second_axis.x.abs() < 0.9 {
                Vector3::unit_x()
            } else {
                Vector3::unit_y()
            };
            let axis = second_axis.cross(basis).normalize();
            Quaternion::new(0.0, axis.x, axis.y, axis.z)
        } else {
            Quaternion::new(1.0 +dot, cross.x, cross.y, cross.z).normalize()
        };
        self.save_undo_state();
        let Some(mesh) = self.imported_meshes
            .iter_mut()
            .find(|mesh| mesh.id == second_id)
        else {
            return false;
        };
        mesh.triangles = Arc::new(
            mesh.triangles
                .iter()
                .map(|triangle| {
                    triangle.map(|point| {
                        first_center + rotation.rotate_vector(point - second_center)
                    })
                })
                .collect()
        );
        for mesh in &mut self.imported_meshes{
            mesh.selected = mesh.id == first_id || mesh.id == second_id;
        }
        self.connect_selected();
        self.rebuild_vertices();
        true
    }
    fn imported_mesh_hit_test(
        &self,
        mesh: &ImportedMeshEntity,
        mouse: cgmath::Vector2<f32>,
    ) -> Option<f32> {
        let ray = self.camera.screen_to_ray(
            mouse,
            Vector2::new(self.width as f32, self.height as f32),
        )?;
        mesh.triangles
            .iter()
            .filter_map(|triangle| {
                ray.intersect_triangle(
                    Point3::new(triangle[0].x, triangle[0].y, triangle[0].z),
                    Point3::new(triangle[1].x, triangle[1].y, triangle[1].z),
                    Point3::new(triangle[2].x, triangle[2].y, triangle[2].z),
                )
            })
            .filter(|distance| *distance >= 0.0)
            .min_by(f32::total_cmp)
    }

    pub fn render_text(
        entity: &TextEntity,
        font_data: &[u8],
    ) -> Result<Vec<Vertex>, anyhow::Error> {
        let face = ttf_parser::Face::parse(font_data, 0)?;
        let units = face.units_per_em() as f32;
        let scale = entity.size / units;
        let mut verts = Vec::new();
        let mut pen_x = 0.0;
        for char in entity.text.chars() {
            let Some(glyph_id) = face.glyph_index(char) else {
                continue;
            };
            let mut outline = Builder::default();
            face.outline_glyph(glyph_id, &mut outline);
            let mut path_builder = Path::builder();
            for command in outline.commands {
                match command {
                    OutlineCommand::MoveTo(x, y) => {
                        path_builder.begin(point(
                            x * scale + pen_x,
                            -y * scale,
                        ));
                    }
                    OutlineCommand::LineTo(x, y) => {
                        path_builder.line_to(point(
                            x * scale + pen_x,
                            -y * scale,
                        ));
                    }
                    OutlineCommand::QuadTo(x1, y1, x, y) => {
                        path_builder.quadratic_bezier_to(
                            point(x1 * scale + pen_x, -y1 * scale),
                            point(x * scale + pen_x, -y * scale),
                        );
                    }
                    OutlineCommand::CurveTo(x1, y1, x2, y2, x, y) => {
                        path_builder.cubic_bezier_to(
                            point(x1 * scale + pen_x, -y1 * scale),
                            point(x2 * scale + pen_x, -y2 * scale),
                            point(x * scale + pen_x, -y * scale),
                        );
                    }
                    OutlineCommand::Close => {
                        path_builder.end(true);
                    }
                };
            }
            let path = path_builder.build();
            let up = entity.quat.rotate_vector(Vector3::unit_y());
            let cap_normal = [up.x, up.y, up.z];
            let back_normal = [-up.x, -up.y, -up.z];
            let height = if entity.depth > 0.0 {
                POLYLINE_HEIGHT + entity.depth
            } else {
                POLYLINE_HEIGHT
            };
            let mut buffers: VertexBuffers<Vertex, usize> = VertexBuffers::new();
            FillTessellator::new().tessellate_path(
                &path,
                &FillOptions::default(),
                &mut BuffersBuilder::new(
                    &mut buffers,
                    |v: FillVertex| {
                        let p = v.position();
                        let local = Vector3::new(p.x, height, p.y);
                        let world = entity.pos + entity.quat.rotate_vector(local);
                        let norm_coords = if entity.depth > 0.0 { cap_normal } else { [0.0;3 ] };
                        Vertex {
                            position: world.into(),
                            coords: norm_coords,
                            color: entity.color,
                        }
                    },
                ),
            )?;
            for &index in &buffers.indices {
                verts.push(buffers.vertices[index as usize]);
            }
            if entity.depth > 0.0 {
                let offset = entity.quat.rotate_vector(Vector3::unit_y() * entity.depth);
                for i in buffers.indices.chunks_exact(3) {
                    for &idx in &[i[0], i[2], i[1]] {
                        let vertex_data = buffers.vertices[idx];
                        let pos = Vector3::new(vertex_data.position[0], vertex_data.position[1], vertex_data.position[2]) - offset;
                        let vertex_struct = Vertex {
                            position: pos.into(),
                            coords: back_normal,
                            color: entity.color,
                        };
                        verts.push(vertex_struct);
                    }
                }
                let mut add_wall = |from: lyon::math::Point, to: lyon::math::Point| {
                    let local_bottom_a = Vector3::new(from.x, POLYLINE_HEIGHT, from.y);
                    let local_bottom_b = Vector3::new(to.x, POLYLINE_HEIGHT, to.y);
                    let local_top_a = Vector3::new(from.x, height, from.y);
                    let local_top_b = Vector3::new(to.x, height, to.y);
                    let bottom_a = entity.pos + entity.quat.rotate_vector(local_bottom_a);
                    let bottom_b = entity.pos + entity.quat.rotate_vector(local_bottom_b);
                    let top_a = entity.pos + entity.quat.rotate_vector(local_top_a);
                    let top_b = entity.pos + entity.quat.rotate_vector(local_top_b);
                    let edge = Vector3::new(to.x - from.x, 0.0, to.y - from.y);
                    let product = edge.cross(Vector3::unit_y());
                    let n = if product.magnitude2() > f32::EPSILON {
                        product.normalize()
                    } else {
                        Vector3::unit_y()
                    };
                    let world_n = entity.quat.rotate_vector(n);
                    let wall_norm = [world_n.x, world_n.y, world_n.z];
                    verts.push(Vertex {
                        position: bottom_a.into(),
                        coords: wall_norm,
                        color: entity.color,
                    });
                    verts.push(Vertex {
                        position: bottom_b.into(),
                        coords: wall_norm,
                        color: entity.color,
                    });
                    verts.push(Vertex {
                        position: top_b.into(),
                        coords: wall_norm,
                        color: entity.color,
                    });
                    verts.push(Vertex {
                        position: bottom_a.into(),
                        coords: wall_norm,
                        color: entity.color,
                    });
                    verts.push(Vertex {
                        position: top_b.into(),
                        coords: wall_norm,
                        color: entity.color,
                    });
                    verts.push(Vertex {
                        position: top_a.into(),
                        coords: wall_norm,
                        color: entity.color,
                    });
                };
                for evt in path.iter().flattened(0.1) {
                    match evt {
                        PathEvent::Line { from, to } => {
                            add_wall(from, to);
                        }
                        PathEvent::End {
                            last,
                            first,
                            close,
                        } => {
                            if close && last != first {
                                add_wall(last, first);
                            }
                        }
                        _ => {}
                    }
                }
            }
            pen_x += face.glyph_hor_advance(glyph_id).unwrap_or(0) as f32 * scale;
        }
        Ok(verts)
    }
    pub fn store_shape(&mut self, points: &[cgmath::Vector3<f32>], color: [f32; 4]) {
        self.hist_entities.save_undo();
        self.hist_surfaces.current = self.nurbs_surfaces.clone();
        self.hist_surfaces.save_undo();
        self.hist_imported_meshes.current = self.imported_meshes.clone();
        self.hist_imported_meshes.save_undo();
        let shape = PolyLine {
            id: self.next_entity,
            vertices: points.to_vec(),
            color,
            thickness: 1.0,
            selected: false,
            height: 0.0,
            extrude_axis: Vector3::unit_y(),
            show_markers: true,
            hatched: false,
        };
        self.next_entity += 1;
        self.hist_entities.current.push(shape);
        self.redraw = true;
    }
    pub fn extrude(
        points: &[cgmath::Vector3<f32>],
        color: [f32; 4],
        height: f32,
        axis: Vector3<f32>,
    ) -> Vec<Vertex> {
        let mut vertices = Vec::new();
        if points.len() < 3
            || !height.is_finite()
            || height.abs() <= f32::EPSILON
            || !axis.x.is_finite()
            || !axis.y.is_finite()
            || !axis.z.is_finite()
            || axis.magnitude2() <= f32::EPSILON
        {
            return vertices;
        }
        let offset = axis.normalize() * height;
        let signed_axis = axis.normalize() * height.signum();
        for window in points.windows(2) {
            let point0 = window[0];
            let point1 = window[1];
            let product = (point1 - point0).cross(signed_axis);
            let wall_normal = if product.magnitude2() > f32::EPSILON {
                let normal = product.normalize();
                [normal.x, normal.y, normal.z]
            } else {
                let normal = signed_axis;
                [normal.x, normal.y, normal.z]
            };
            let bottom0 = [point0.x, point0.y, point0.z];
            let bottom1 = [point1.x, point1.y, point1.z];
            let top_point0 = point0 + offset;
            let top_point1 = point1 + offset;
            let top0 = [top_point0.x, top_point0.y, top_point0.z];
            let top1 = [top_point1.x, top_point1.y, top_point1.z];
            vertices.extend([
                Vertex { position: bottom0, coords: wall_normal, color },
                Vertex { position: bottom1, coords: wall_normal, color },
                Vertex { position: top1, coords: wall_normal, color },
                Vertex { position: bottom0, coords: wall_normal, color },
                Vertex { position: top1, coords: wall_normal, color },
                Vertex { position: top0, coords: wall_normal, color },
            ]);
        }
        vertices
    }
    fn hatch_plane(
        points: &[Vector3<f32>]
    ) -> Option<(Vector3<f32>, Vector3<f32>, Vector3<f32>, Vector3<f32>)> {
        let count = if Self::is_closed_shape(points) {
            points.len() -1
        } else {
            points.len()
        };
        if count < 3 {
            return None;
        }
        let origin = points[0];
        let u = points[1..count]
            .iter()
            .map(|point| *point - origin)
            .find(|direction| direction.magnitude2()> 1.0e-10)?
            .normalize();
        let normal = points[1..count]
            .iter()
            .map(|point| u.cross(*point - origin))
            .find(|candidate| candidate.magnitude2() > 1.0e-10)?
            .normalize();
        let v = normal.cross(u).normalize();
        let planar = points[..count]
            .iter()
            .all(|point| ((*point - origin).dot(normal)).abs() <= 0.001);
        if !planar {
            return None;
        }
        Some((origin, u, v, normal))
    }
    fn polygon_area_on_plane(
        points: &[Vector3<f32>],
        plane: &(Vector3<f32>, Vector3<f32>, Vector3<f32>, Vector3<f32>)
    ) -> f32 {
        let (origin, u, v, _) = *plane;
        let count = if Self::is_closed_shape(points) {
            points.len() -1
        } else {
            points.len()
        };
        if count < 3 {
            return 0.0;
        }
        let mut doubled_area = 0.0;
        for index in 0..count {
            let a = points[index] - origin;
            let b = points[(index + 1) % count] - origin;
            let ax = a.dot(u);
            let ay = a.dot(v);
            let bx = b.dot(u);
            let by = b.dot(v);
            doubled_area += ax*by - bx * ay;
        }
        doubled_area.abs() * 0.5
    }
    fn point_in_polygon_on_plane(
        point_3d: Vector3<f32>,
        polygon: &[Vector3<f32>],
        plane: &(Vector3<f32>, Vector3<f32>, Vector3<f32>, Vector3<f32>)) -> bool {
            let (origin, u, v, normal) = *plane;
            let count = if Self::is_closed_shape(polygon) {
                polygon.len() - 1
            } else {
                polygon.len()
            };
            if count < 3 || ((point_3d - origin).dot(normal)).abs() > 0.001 {
                return false;
            }
            let project = |point: Vector3<f32>| {
                let offset = point-origin;
                (offset.dot(u), offset.dot(v))
            };
            let (px, py) = project(point_3d);
            let mut inside = false;
            let mut previous = count -1;
            for current in 0..count {
                let(ax, ay) = project(polygon[current]);
                let (bx, by) = project(polygon[previous]);
                if (ay > py) != (by > py)
                    && px < (bx - ax) * (py-ay) / (by-ay) + ax
                {
                    inside = !inside;
                }
                previous = current;
            }
            inside
        }
    fn is_coplanar_with_plane(
        points: &[Vector3<f32>],
        plane: &(Vector3<f32>, Vector3<f32>, Vector3<f32>, Vector3<f32>)
    )-> bool {
        let (origin, _, _, normal) = *plane;
        points
            .iter()
            .all(|point| ((*point - origin).dot(normal)).abs()<= 0.001)
    }
    pub fn tessellate_fill(points: &[Vector3<f32>], color: [f32; 4]) -> Vec<Vertex> {
        Self::tessellate_fill_contours(&[points], color)
    }
    fn tessellate_fill_contours(
        contours: &[&[Vector3<f32>]],
        color: [f32; 4]
    ) -> Vec<Vertex> {
        let Some(outer) = contours.iter().find(|points| points.len() >= 3) else {
            return Vec::new();
        };
        let Some((origin, u, v, normal)) = Self::hatch_plane(outer) else {
            return Vec::new();
        };
        let mut builder = Path::builder();
        for contour in contours {
            if contour.len()<3
                || !contour.iter().all(|point| {
                    ((*point - origin).dot(normal)).abs() <= 0.001
                })
            {
                continue
            }
            let project = |point: Vector3<f32>| {
                let offset = point - origin;
                (offset.dot(u), offset.dot(v))
            };
            let (x, y) = project(contour[0]);
            builder.begin(point(x, y));
            for vertex in &contour[1..] {
                let (x, y) = project(*vertex);
                builder.line_to(point(x, y));
            }
            builder.end(true);
        }
        let path = builder.build();
        let mut geometry: VertexBuffers<Vertex, u16> = VertexBuffers::new();
        let mut tessellator = FillTessellator::new();
        let result = tessellator.tessellate_path(
            &path,
            &FillOptions::default().with_fill_rule(FillRule::EvenOdd),
            &mut BuffersBuilder::new(&mut geometry, |vertex: FillVertex| {
                let position = vertex.position();
                let world = origin + u*position.x + v * position.y;
                Vertex {
                    position: [world.x, world.y, world.z],
                    coords: [normal.x, normal.y, normal.z], color
                }
            })
        );
        if result.is_err() {
            return Vec::new();
        }
        geometry
            .indices
            .iter()
            .map(|&index| geometry.vertices[index as usize])
            .collect()
    }
    fn segment_intersection_parameters(a: Vector2<f32>, b: Vector2<f32>, c: Vector2<f32>, d: Vector2<f32>) -> Option<(f32, f32)> {
        let r = b-a;
        let s = d-c;
        let denominator = r.x*s.y - r.y * s.x;
        if denominator.abs() < 1.0e-6{
            return None;
        }
        let offset = c-a;
        let t = (offset.x * s.y - offset.y * s.x) / denominator;
        let u = (offset.x * r.y - offset.y * r.x) / denominator;
        if (-1.0e-4..=1.0001).contains(&t) && (-1.0e-4..=1.0001).contains(&u) {
            Some((t, u))
        } else {
            None
        }
    }
    pub fn dist_to_segment(
        p: cgmath::Vector2<f32>,
        a: cgmath::Vector2<f32>,
        b: cgmath::Vector2<f32>,
    ) -> f32 {
        let ab = b - a;
        let ap = p - a;
        let len_sq = ab.magnitude2();
        if len_sq == 0.0 {
            return ap.magnitude2();
        }
        let t = (ap.dot(ab) / len_sq).clamp(0.0, 1.0);
        let projection = a + ab * t;
        (p - projection).magnitude2()
    }
    pub fn move_shape(&mut self, original_points: &[Vector3<f32>], move_pos: &[Vector3<f32>]) {
        let temp_points: Vec<Vector3<f32>> = original_points
            .iter()
            .zip(move_pos.iter())
            .map(|(point, delta)| *point + *delta)
            .collect();
        let points = temp_points
            .iter()
            .map(|p| self.grid_snap_point(*p))
            .collect::<Vec<_>>();
        let vertices: Vec<Vertex> = points
            .into_iter()
            .map(|position| Vertex {
                position: position.into(),
                coords: position.into(),
                color: [0.1, 0.1, 0.2, 0.5],
            })
            .collect();
        self.vertice_append(&vertices);
        self.rebuild_vertices();
    }
    pub fn move_shape_key(&mut self, code: KeyCode, step_size: f32) -> bool {
        let mut move_pos = Vector3::zero();
        match code {
            KeyCode::ArrowLeft => move_pos.x -= step_size,
            KeyCode::ArrowRight => move_pos.x += step_size,
            KeyCode::ArrowUp => move_pos.z += step_size,
            KeyCode::ArrowDown => move_pos.z -= step_size,
            _ => return false,
        }
        let offset = self.grid_snap_offset(move_pos);
        let Some(sel_id) = self.selected_entity else {
            return false;
        };
        let Some(entity) = self.entities.iter_mut().find(|entity| entity.id == sel_id) else {
            return false;
        };
        for point in &mut entity.vertices {
            *point += offset;
        }
        self.rebuild_vertices();
        true
    }
    pub fn select_objects_by_ids(&mut self, ids: &[usize]) {
        let mut expanded_ids = ids.to_vec();
        for id in ids {
            expanded_ids.extend(self.connected_ids(*id));
        }
        expanded_ids.sort_unstable();
        expanded_ids.dedup();
        for entity in &mut self.entities {
            entity.selected = expanded_ids.contains(&entity.id);
        }
        for curve in &mut self.nurbs_curves {
            curve.selected = expanded_ids.contains(&curve.id);
        }
        for solid in &mut self.solids {
            solid.selected = expanded_ids.contains(&solid.id);
        }
        for surface in &mut self.nurbs_surfaces {
            surface.selected = expanded_ids.contains(&surface.id);
        }
        for mesh in &mut self.imported_meshes {
            mesh.selected = expanded_ids.contains(&mesh.id);
        }
        for text in &mut self.text_entities {
            text.selected = expanded_ids.contains(&text.id);
        }
        self.selected_entity = self.entities
            .iter()
            .find(|entity| entity.selected)
            .map(|entity| entity.id)
            .or_else(|| {
                self.nurbs_surfaces
                    .iter()
                    .find(|surface| surface.selected)
                    .map(|surface| surface.id)
            })
            .or_else(|| {
                self.imported_meshes
                    .iter()
                    .find(|mesh| mesh.selected)
                    .map(|mesh| mesh.id)
            });
        self.rebuild_vertices();
    }

    pub fn material_object_data(&self) -> Vec<(usize, bool)> {
        let mut objects = Vec::new();
        objects.extend(self.entities.iter().map(|item| (item.id, item.selected)));
        objects.extend(self.nurbs_curves.iter().map(|item| (item.id, item.selected)));
        objects.extend(self.nurbs_surfaces.iter().map(|item| (item.id, item.selected)));
        objects.extend(self.imported_meshes.iter().map(|item| (item.id, item.selected)));
        objects.extend(self.solids.iter().map(|item| (item.id, item.selected)));
        objects.extend(self.text_entities.iter().map(|item| (item.id, item.selected)));
        objects.sort_unstable_by_key(|(id, _)| *id);
        objects.dedup_by_key(|(id, _)| *id);
        objects
    }
    pub fn trim_at(&mut self, mouse_x: f32, mouse_y: f32) -> bool {
        let mouse = Vector2::new(mouse_x, mouse_y);
        let mut closest: Option<(usize, usize, f32, f32)> = None;
        for entity in &self.entities {
            for (segment_index, pair) in entity.vertices.windows(2).enumerate() {
                let (Some(a), Some(b)) = (
                    self.world_to_screen(pair[0]),
                    self.world_to_screen(pair[1]),
                ) else {
                    continue;
                };
            let ab = b-a;
            let length_sq = ab.magnitude2();
            if length_sq <= 1.0e-8 {
                continue;
            }
            let click_t = ((mouse - a).dot(ab)/length_sq).clamp(0.0, 1.0);
            let projected_click = a + ab * click_t;
            let distance_sq = (mouse - projected_click).magnitude2();
            if closest.map(|(_, _, best, _)| distance_sq < best).unwrap_or(true) {
                closest = Some((entity.id, segment_index, distance_sq, click_t));
            }
        }
        }
        let Some((target_id, target_segment, click_distance_sq, click_t)) = closest else {
            return false;
        };
        if click_distance_sq > 12.0 * 12.0 {
            return false;
        }
        let target = match self.entities.iter().find(|entity| entity.id == target_id) {
            Some(entity) => entity, None => return false,
        };
        if target.vertices.len() >= 4
            && (target.vertices[0] - *target.vertices.last().unwrap()).magnitude2() <= 1.0e-8
        {
            let vertices = target.vertices.clone();
            let segment_count = vertices.len() - 1;
            let click_position = target_segment as f32 + click_t;
            let view_proj = self.camera.build_view_projection_matrix();
            let mut hits: Vec<(f32, Vector3<f32>)> = Vec::new();
            for segment_index in 0..segment_count {
                let a_world = vertices[segment_index];
                let b_world = vertices[segment_index + 1];
                let (Some(a_screen), Some(b_screen)) =
                    (self.world_to_screen(a_world), self.world_to_screen(b_world))
                else { continue; };
                for cutter in &self.entities {
                    for cutter_pair in cutter.vertices.windows(2) {
                        if cutter.id == target_id {
                            continue;
                        }
                        let (Some(c_screen), Some(d_screen)) = (
                            self.world_to_screen(cutter_pair[0]),
                            self.world_to_screen(cutter_pair[1]),
                        ) else { continue; };
                        let Some((t, _)) = Self::segment_intersection_parameters(
                            a_screen, b_screen, c_screen, d_screen,
                        ) else { continue; };

                        let clip_a = view_proj * Vector4::new(a_world.x, a_world.y, a_world.z, 1.0);
                        let clip_b = view_proj * Vector4::new(b_world.x, b_world.y, b_world.z, 1.0);
                        let denom = (1.0 - t) * clip_b.w + t * clip_a.w;
                        let world_t = if denom.abs() > 1.0e-6 {
                            (t * clip_a.w / denom).clamp(0.0, 1.0)
                        } else { t.clamp(0.0, 1.0) };
                        let hit_point = a_world + (b_world - a_world) * world_t;
                        hits.push((segment_index as f32 + t, hit_point));
                    }
                }
            }

            hits.sort_by(|a, b| a.0.total_cmp(&b.0));
            hits.dedup_by(|a, b| (a.0 - b.0).abs() < 1.0e-3);
            if hits.len() < 2 { return false; }

            let previous = hits.iter().rev().find(|hit| hit.0 < click_position)
                .cloned().unwrap_or_else(|| hits[hits.len() - 1].clone());
            let next = hits.iter().find(|hit| hit.0 > click_position)
                .cloned().unwrap_or_else(|| hits[0].clone());

            let mut start_position = next.0;
            let mut end_position = previous.0;
            if end_position <= start_position { end_position += segment_count as f32; }
            let mut kept = vec![next.1];
            let mut vertex_position = start_position.floor() as usize + 1;
            while (vertex_position as f32) < end_position {
                kept.push(vertices[vertex_position % segment_count]);
                vertex_position += 1;
            }
            kept.push(previous.1);
            kept.dedup_by(|a, b| (*a - *b).magnitude2() <= 1.0e-10);
            if kept.len() < 2 { return false; }

            self.save_undo_state();
            if let Some(entity) = self.entities.iter_mut().find(|entity| entity.id == target_id) {
                entity.vertices = kept;
            } else { return false; }
            self.rebuild_vertices();
            return true;
        }
        let (Some(target_a), Some(target_b)) = (
            self.world_to_screen(target.vertices[target_segment]),
            self.world_to_screen(target.vertices[target_segment + 1]),
        ) else {
            return false;
        };
        let mut nearest_hit: Option<f32> = None;
        for cutter in &self.entities {
            for (cutter_segment, pair) in cutter.vertices.windows(2).enumerate() {
                if cutter.id == target_id
                    && cutter_segment.abs_diff(target_segment) <= 1
                {
                    continue;
                }
                let (Some(c), Some(d)) = (
                    self.world_to_screen(pair[0]),
                    self.world_to_screen(pair[1]),
                ) else {
                    continue;
                };
                let Some((t, _)) =
                    Self::segment_intersection_parameters(target_a, target_b, c, d)
                else {
                    continue;
                };
                if nearest_hit
                    .map(|best_t| (t - click_t).abs() < (best_t - click_t).abs())
                    .unwrap_or(true)
                {
                    nearest_hit = Some(t);
                }
            }
        }
        let Some(hit_t) = nearest_hit else {
            return false;
        };
        let original_a = target.vertices[target_segment];
        let original_b = target.vertices[target_segment + 1];
        let view_proj = self.camera.build_view_projection_matrix();
        let clip_a = view_proj * Vector4::new(original_a.x, original_a.y, original_a.z, 1.0);
        let clip_b = view_proj * Vector4::new(original_b.x, original_b.y, original_b.z, 1.0);
        let perspective_denom = (1.0 - hit_t) * clip_b.w + hit_t * clip_a.w;
        let world_t = if perspective_denom.abs() > 1.0e-6 {
            (hit_t * clip_a.w / perspective_denom).clamp(0.0, 1.0)
        } else {
            hit_t
        };
        let new_endpoint = original_a + (original_b - original_a) * world_t;
        self.save_undo_state();
        let mut remove_target = false;
        if let Some(entity) = self.entities.iter_mut().find(|entity| entity.id == target_id) {
            if click_t < hit_t {
                entity.vertices.drain(..target_segment);
                entity.vertices[0] = new_endpoint;
            } else {
                entity.vertices.truncate(target_segment + 2);
                entity.vertices[target_segment + 1] = new_endpoint;
            }
            entity.vertices.dedup_by(|a, b| (*a - *b).magnitude2() <= 1.0e-10);
            remove_target = entity.vertices.len() < 2;
        } else {
            return false;
        }
        if remove_target {
            self.entities.retain(|entity| entity.id != target_id);
        }
        self.rebuild_vertices();
        true
    }
    pub fn select_shape(
        &mut self,
        mouse_px: cgmath::Vector2<f32>,
        hit_threshold_px: f32,
        additive: bool,
    ) -> Option<usize> {
        let threshold_sq = hit_threshold_px * hit_threshold_px;
        let mut closest = None;
        let mut min_dist_sq = threshold_sq;

        for entity in &self.entities {
            if entity.vertices.len() < 2 {
                continue;
            }
            let screen_vertices: Vec<cgmath::Vector2<f32>> = entity
                .vertices
                .iter()
                .filter_map(|point| self.world_to_screen(*point))
                .collect();
            if self.hit_extruded_side(entity, mouse_px, threshold_sq) {
                closest = Some(entity.id);
                min_dist_sq = 0.0;
                continue;
            }
            if entity.hatched
                && Self::is_closed_shape(&entity.vertices)
                && Self::point_in_polygon(mouse_px, &screen_vertices)
            {
                closest = Some(entity.id);
                min_dist_sq = 0.0;
                continue;
            }
            for segment in screen_vertices.windows(2) {
                let distance = Self::dist_to_segment(mouse_px, segment[0], segment[1]);

                if distance < min_dist_sq {
                    min_dist_sq = distance;
                    closest = Some(entity.id);
                }
            }
        }
        for text in &self.text_entities {
            if self.text_hit_test(text, mouse_px) {
                closest = Some(text.id);
                min_dist_sq = 0.0;
                break;
            }
        }
        for curve in &self.nurbs_curves {
            let screen_points: Vec<cgmath::Vector2<f32>> = curve.curve.sample(64)
                .iter()
                .filter_map(|point| self.world_to_screen(*point))
                .collect();
            for segment in screen_points.windows(2) {
                let distance = Self::dist_to_segment(mouse_px, segment[0], segment[1]);
                if distance < min_dist_sq {
                    min_dist_sq = distance;
                    closest = Some(curve.id);
                }
            }
        }

        let mut closest_3d_dist = f32::MAX;
        for s in &self.nurbs_surfaces {
            if let Some(t) = self.surface_hit_test(s, mouse_px) {
                if t < closest_3d_dist {
                    closest_3d_dist = t;
                    closest = Some(s.id);
                    min_dist_sq = 0.0;
                }
            }
        }
        for mesh in &self.imported_meshes {
            if let Some(t) = self.imported_mesh_hit_test(mesh, mouse_px) {
                if t < closest_3d_dist {
                    closest_3d_dist = t;
                    closest = Some(mesh.id);
                    min_dist_sq = 0.0;
                }
            }
        }
        for solid in &self.solids {
            if let Some(t) = self.solid_hit_test(solid, mouse_px) {
                if t < closest_3d_dist {
                    closest_3d_dist = t;
                    closest = Some(solid.id);
                    min_dist_sq = 0.0;
                }
            }
        }
        for text in &mut self.text_entities {
            text.selected = if additive {
                text.selected || closest == Some(text.id)
            } else {
                closest == Some(text.id)
            };
        }
        let connected_selection = closest
            .map(|id| self.connected_ids(id))
            .unwrap_or_default();
        if additive {
            for entity in &mut self.entities {
                if connected_selection.contains(&entity.id) {
                    entity.selected = true;
                }
            }
            for surface in &mut self.nurbs_surfaces {
                if connected_selection.contains(&surface.id) {
                    surface.selected = true;
                }
            }
            for mesh in &mut self.imported_meshes {
                if connected_selection.contains(&mesh.id) {
                    mesh.selected = true;
                }
            }
            for curve in &mut self.nurbs_curves {
                if connected_selection.contains(&curve.id) {
                    curve.selected = true;
                }
            }
            for solid in &mut self.solids {
                if connected_selection.contains(&solid.id) {
                    solid.selected = true;
                }
            }
            for text in &mut self.text_entities {
                if connected_selection.contains(&text.id) {
                    text.selected = true;
                }
            }
        } else {
            for entity in &mut self.entities {
                entity.selected = connected_selection.contains(&entity.id);
            }
            for surface in &mut self.nurbs_surfaces {
                surface.selected = connected_selection.contains(&surface.id);
            }
            for mesh in &mut self.imported_meshes {
                mesh.selected = connected_selection.contains(&mesh.id);
            }
            for curve in &mut self.nurbs_curves {
                curve.selected = connected_selection.contains(&curve.id);
            }
            for solid in &mut self.solids {
                solid.selected = connected_selection.contains(&solid.id);
            }
            for text in &mut self.text_entities {
                text.selected = text.id == closest.unwrap_or(usize::MAX);
            }
        }
        self.selected_entity = self
            .entities
            .iter()
            .find(|entity| entity.selected)
            .map(|entity| entity.id)
            .or_else(|| {
                self.nurbs_surfaces
                    .iter()
                    .find(|surface| surface.selected)
                    .map(|surface| surface.id)
            })
            .or_else(|| {
                self.imported_meshes
                    .iter()
                    .find(|mesh| mesh.selected)
                    .map(|mesh| mesh.id)
            })
            .or_else(|| {
                self.solids
                    .iter()
                    .find(|s| s.selected)
                    .map(|s| s.id)  
            })
            .or_else(|| self.text_entities.iter().find(|t| t.selected).map(|t| t.id));
        if let Some(s) = self.nurbs_surfaces.iter().find(|s| s.selected) {
            let mut sum = Vector3::new(0.0, 0.0, 0.0);
            let mut count = 0.0;
            for row in &s.surface.control_points {
                for cp in row {
                    sum += cp.point;
                    count += 1.0;
                }
            }
            let centroid = if count > 0.0 {
                sum / count
            } else {
                Vector3::new(0.0, 0.0, 0.0)
            };
            self.gumball = Some(Gumball::new(Point3::new(centroid.x, centroid.y, centroid.z), 0.25));
        } else if let Some(mesh) = self.imported_meshes.iter().find(|mesh| mesh.selected) {
            let (sum, count) = mesh
                .triangles
                .iter()
                .flat_map(|triangle| triangle.iter())
                .fold((Vector3::zero(), 0usize), |(sum, count), point| {
                    (sum + *point, count + 1)
                });
            if count > 0 {
                let center = sum / count as f32;
                self.gumball = Some(Gumball::new(
                    Point3::new(center.x, center.y, center.z),
                    0.25,
                ));
            } else {
                self.gumball = None;
            }
        } else if let Some(text) = self.text_entities.iter().find(|text| text.selected) {
            self.gumball = Some(Gumball::new(
                Point3::new(text.pos.x, text.pos.y, text.pos.z),
                0.25,
            ));
        } else if let Some(solid) = self.solids.iter().find(|s| s.selected) {
            let origin = solid.centroid();
            self.gumball = Some(Gumball::new(Point3::new(origin[0], origin[1], origin[2]), 0.25))
        } else if let Some(e) = self.entities.iter().find(|e| e.selected) {
            if !e.vertices.is_empty() {
                let mut sum = Vector3::new(0.0, 0.0, 0.0);
                for v in &e.vertices {
                    sum += *v;
                }
                let centroid = sum / e.vertices.len() as f32;
                self.gumball = Some(Gumball::new(Point3::new(centroid.x, centroid.y, centroid.z), 0.25))
            } else {
                self.gumball = None;
            }
        } else {
            self.gumball = None;
        }
        self.rebuild_vertices();
        closest
    }

    fn point_in_rect(
        point: cgmath::Vector2<f32>,
        min: cgmath::Vector2<f32>,
        max: cgmath::Vector2<f32>,
    ) -> bool {
        point.x >= min.x && point.x <= max.x && point.y >= min.y && point.y <= max.y
    }
    fn point_in_polygon(point: cgmath::Vector2<f32>, vertices: &[cgmath::Vector2<f32>]) -> bool {
        if vertices.len() < 3 {
            return false;
        }
        let mut inside = false;
        let mut previous = vertices.len() - 1;
        for current in 0..vertices.len() {
            let a = vertices[current];
            let b = vertices[previous];
            let crosses = (a.y > point.y) != (b.y > point.y);
            if crosses {
                let intersection_x = (b.x - a.x) * (point.y - a.y) / (b.y - a.y) + a.x;
                if point.x < intersection_x {
                    inside = !inside;
                }
            }
            previous = current;
        }
        inside
    }

    fn segment_intersects_rect(
        start: cgmath::Vector2<f32>,
        end: cgmath::Vector2<f32>,
        min: cgmath::Vector2<f32>,
        max: cgmath::Vector2<f32>,
    ) -> bool {
        if Self::point_in_rect(start, min, max) || Self::point_in_rect(end, min, max) {
            return true;
        }

        let delta = end - start;
        let checks = [
            (-delta.x, start.x - min.x),
            (delta.x, max.x - start.x),
            (-delta.y, start.y - min.y),
            (delta.y, max.y - start.y),
        ];
        let mut enter: f32 = 0.0;
        let mut leave: f32 = 1.0;
        for (direction, distance) in checks {
            if direction.abs() <= f32::EPSILON {
                if distance < 0.0 {
                    return false;
                }
                continue;
            }
            let ratio = distance / direction;
            if direction < 0.0 {
                enter = enter.max(ratio);
            } else {
                leave = leave.min(ratio);
            }
            if enter > leave {
                return false;
            }
        }
        true
    }

    pub fn select_box(
        &mut self,
        start_x: f32,
        start_y: f32,
        end_x: f32,
        end_y: f32,
        additive: bool,
    ) -> usize {
        let min = cgmath::Vector2::new(start_x.min(end_x), start_y.min(end_y));

        let max = cgmath::Vector2::new(start_x.max(end_x), start_y.max(end_y));

        let window_selection = end_x >= start_x;
        let mut selected_ids = Vec::new();

        for entity in &self.entities {
            let screen_vertices: Vec<cgmath::Vector2<f32>> = entity
                .vertices
                .iter()
                .filter_map(|point| self.world_to_screen(*point))
                .collect();

            if screen_vertices.len() < 2 {
                continue;
            }

            let selected = if window_selection {
                screen_vertices
                    .iter()
                    .all(|point| Self::point_in_rect(*point, min, max))
            } else {
                screen_vertices
                    .windows(2)
                    .any(|segment| Self::segment_intersects_rect(segment[0], segment[1], min, max))
            };

            if selected {
                selected_ids.push(entity.id);
            }
        }

        for surface in &self.nurbs_surfaces {
            let mesh = surface.surface.sample_mesh(
                surface.u_subdivisions,
                surface.v_subdivisions,
                surface.color,
            );
            let screen_points: Vec<cgmath::Vector2<f32>> = mesh
                .iter()
                .filter_map(|vertex| {
                    self.world_to_screen(Vector3::new(
                        vertex.position[0],
                        vertex.position[1],
                        vertex.position[2],
                    ))
                })
                .collect();

            let selected = if window_selection {
                !screen_points.is_empty()
                    && screen_points
                        .iter()
                        .all(|point| Self::point_in_rect(*point, min, max))
            } else {
                screen_points.chunks_exact(3).any(|triangle| {
                    let a = triangle[0];
                    let b = triangle[1];
                    let c = triangle[2];
                    Self::point_in_rect(a, min, max)
                        || Self::point_in_rect(b, min, max)
                        || Self::point_in_rect(c, min, max)
                        || Self::segment_intersects_rect(a, b, min, max)
                        || Self::segment_intersects_rect(b, c, min, max)
                        || Self::segment_intersects_rect(c, a, min, max)
                })
            };

            if selected {
                selected_ids.push(surface.id);
            }
        }

        for mesh in &self.imported_meshes {
            let projected: Vec<Option<Vector2<f32>>> = mesh
                .triangles
                .iter()
                .flat_map(|triangle| triangle.iter().copied())
                .map(|point| self.world_to_screen(point))
                .collect();
            let selected = if window_selection {
                !projected.is_empty()
                    && projected.iter().all(|point| {
                        point.map(|point| Self::point_in_rect(point, min, max)).unwrap_or(false)
                    })
            } else {
                projected.chunks_exact(3).any(|triangle| {
                    let (Some(a), Some(b), Some(c)) = (triangle[0], triangle[1], triangle[2]) else {
                        return false;
                    };
                    Self::point_in_rect(a, min, max)
                        || Self::point_in_rect(b, min, max)
                        || Self::point_in_rect(c, min, max)
                        || Self::segment_intersects_rect(a, b, min, max)
                        || Self::segment_intersects_rect(b, c, min, max)
                        || Self::segment_intersects_rect(c, a, min, max)
                })
            };
            if selected {
                selected_ids.push(mesh.id);
            }
        }

        for text in &self.text_entities {
            let projected: Vec<Option<Vector2<f32>>> = text
                .cached_vertices
                .iter()
                .map(|vertex| {
                    self.world_to_screen(Vector3::new(
                        vertex.position[0],
                        vertex.position[1],
                        vertex.position[2],
                    ))
                })
                .collect();
            let selected = if window_selection {
                !projected.is_empty()
                    && projected.iter().all(|point| {
                        point
                            .map(|point| Self::point_in_rect(point, min, max))
                            .unwrap_or(false)
                    })
            } else {
                projected.chunks_exact(3).any(|triangle| {
                    let (Some(a), Some(b), Some(c)) =
                        (triangle[0], triangle[1], triangle[2])
                    else {
                        return false;
                    };
                    Self::point_in_rect(a, min, max)
                        || Self::point_in_rect(b, min, max)
                        || Self::point_in_rect(c, min, max)
                        || Self::segment_intersects_rect(a, b, min, max)
                        || Self::segment_intersects_rect(b, c, min, max)
                        || Self::segment_intersects_rect(c, a, min, max)
                })
            };
            if selected {
                selected_ids.push(text.id);
            }
        }

        let directly_selected_ids = selected_ids.clone();
        for id in directly_selected_ids {
            selected_ids.extend(self.connected_ids(id));
        }
        selected_ids.sort_unstable();
        selected_ids.dedup();

        for entity in &mut self.entities {
            entity.selected = if additive {
                entity.selected || selected_ids.contains(&entity.id)
            } else {
                selected_ids.contains(&entity.id)
            };
        }
        for surface in &mut self.nurbs_surfaces {
            surface.selected = if additive {
                surface.selected || selected_ids.contains(&surface.id)
            } else {
                selected_ids.contains(&surface.id)
            };
        }
        for mesh in &mut self.imported_meshes {
            mesh.selected = if additive {
                mesh.selected || selected_ids.contains(&mesh.id)
            } else {
                selected_ids.contains(&mesh.id)
            };
        }
        for text in &mut self.text_entities {
            text.selected = if additive {
                text.selected || selected_ids.contains(&text.id)
            } else {
                selected_ids.contains(&text.id)
            };
        }
        self.selected_entity = self
            .entities
            .iter()
            .find(|item| item.selected)
            .map(|item| item.id)
            .or_else(|| {
                self.nurbs_surfaces
                    .iter()
                    .find(|item| item.selected)
                    .map(|item| item.id)
            })
            .or_else(|| {
                self.imported_meshes
                    .iter()
                    .find(|item| item.selected)
                    .map(|item| item.id)
            })
            .or_else(|| self.text_entities.iter().find(|item| item.selected).map(|item| item.id));
        let selected_text: Vec<Vector3<f32>> = self
            .text_entities
            .iter()
            .filter(|text| text.selected)
            .map(|text| text.pos)
            .collect();
        if !selected_text.is_empty() {
            let center = selected_text.iter().copied().fold(Vector3::zero(), |sum, p| sum + p)
                / selected_text.len() as f32;
            self.gumball = Some(Gumball::new(Point3::new(center.x, center.y, center.z), 0.25));
        }
        self.rebuild_vertices();
        self.entities
            .iter()
            .filter(|entity| entity.selected)
            .count()
            + self
                .nurbs_surfaces
                .iter()
                .filter(|surface| surface.selected)
                .count()
            + self
                .imported_meshes
                .iter()
                .filter(|mesh| mesh.selected)
                .count()
            + self.text_entities.iter().filter(|text| text.selected).count()
    }

    pub fn delete_selected(&mut self) -> usize {
        let deleted_polylines = self
            .entities
            .iter()
            .filter(|entity| entity.selected)
            .count();
        let deleted_surfaces = self
            .nurbs_surfaces
            .iter()
            .filter(|surface| surface.selected)
            .count();
        let deleted_meshes = self
            .imported_meshes
            .iter()
            .filter(|mesh| mesh.selected)
            .count();
        let deleted_solids = self
            .solids
            .iter()
            .filter(|s| s.selected)
            .count();
        let deleted = deleted_polylines + deleted_surfaces + deleted_meshes + deleted_solids;
        if deleted == 0 {
            return 0;
        }
        self.save_undo_state();
        self.entities.retain(|entity| !entity.selected);
        self.nurbs_surfaces.retain(|surface| !surface.selected);
        self.imported_meshes.retain(|mesh| !mesh.selected);
        self.solids.retain(|s| !s.selected);
        self.selected_entity = None;
        self.rebuild_vertices();
        deleted
    }
    pub fn translate_objects(
        &mut self,
        object_ids: &[usize],
        offset:Vector3<f32>,
    ) -> usize {
        if object_ids.is_empty()
            || !offset.x.is_finite()
            || !offset.y.is_finite()
            || !offset.z.is_finite()
        {
            return 0;
        }
        let polyline_count = self
            .entities
            .iter()
            .filter(|entity| object_ids.contains(&entity.id))
            .count();
        let surface_count = self
            .nurbs_surfaces
            .iter()
            .filter(|surface| object_ids.contains(&surface.id))
            .count();
        let mesh_count = self
            .imported_meshes
            .iter()
            .filter(|mesh| object_ids.contains(&mesh.id))
            .count();
        let solid_count = self
            .solids
            .iter()
            .filter(|s| object_ids.contains(&s.id))
            .count();
        let moved_count = polyline_count + surface_count + mesh_count + solid_count;
        if moved_count == 0 {
            return 0;
        }
        self.save_undo_state();
        for entity in &mut self.entities {
            if object_ids.contains(&entity.id) {
                for point in &mut entity.vertices {
                    *point += offset;
                }
            }
        }
        for surface in &mut self.nurbs_surfaces {
            if object_ids.contains(&surface.id) {
                for row in &mut surface.surface.control_points {
                    for control_point in row {
                        control_point.point += offset;
                    }
                }
            }
        }
        for solid in &mut self.solids {
            if object_ids.contains(&solid.id) {
                solid.origin[0] += offset.x;
                solid.origin[1] += offset.y;
                solid.origin[2] += offset.z;
            }
        }
        for mesh in &mut self.imported_meshes {
            if object_ids.contains(&mesh.id) {
                mesh.triangles = Arc::new(
                    mesh.triangles
                        .iter()
                        .map(|triangle| triangle.map(|point| point + offset))
                        .collect(),
                );
            }
        }
        self.rebuild_vertices();
        moved_count
    }
    pub fn delete_objects(&mut self, ids: &[usize]) -> usize {
        let deleted_polylines = self
            .entities
            .iter()
            .filter(|entity| ids.contains(&entity.id))
            .count();
        let deleted_surfaces = self
            .nurbs_surfaces
            .iter()
            .filter(|surface| ids.contains(&surface.id))
            .count();
        let deleted_meshes = self
            .imported_meshes
            .iter()
            .filter(|mesh| ids.contains(&mesh.id))
            .count();
        let deleted_solids = self
            .solids
            .iter()
            .filter(|s| ids.contains(&s.id))
            .count();
        let deleted = deleted_polylines + deleted_surfaces + deleted_meshes + deleted_solids;
        if deleted == 0 {
            return 0;
        }
        self.save_undo_state();
        self.entities.retain(|entity| !ids.contains(&entity.id));
        self.nurbs_surfaces.retain(|surface| !ids.contains(&surface.id));
        self.imported_meshes.retain(|mesh| !ids.contains(&mesh.id));
        self.solids.retain(|s| !ids.contains(&s.id));
        self.selected_entity = self
            .entities
            .iter()
            .find(|entity| entity.selected)
            .map(|entity| entity.id)
            .or_else(|| {
                self.nurbs_surfaces
                    .iter()
                    .find(|surface| surface.selected)
                    .map(|surface| surface.id)
            })
            .or_else(|| {
                self.imported_meshes
                    .iter()
                    .find(|mesh| mesh.selected)
                    .map(|mesh| mesh.id)
            });
        self.rebuild_vertices();
        deleted
    }
    fn hit_extruded_side(
        &self, entity: &PolyLine,
        mouse: Vector2<f32>,
        threshold_sq: f32
    ) -> bool {
        if entity.height.abs() <= f32::EPSILON
            || !Self::is_closed_shape(&entity.vertices)
            || entity.extrude_axis.magnitude2() <= f32::EPSILON
        {
            return false;
        }
        let offset = entity.extrude_axis.normalize() * entity.height;
        let point_count = entity.vertices.len() -1;
        for i in 0..point_count {
            let next = (i + 1) % point_count;
            let base_a = entity.vertices[i];
            let base_b = entity.vertices[next];
            let top_b = base_b + offset;
            let top_a = base_a + offset;
            let quad_world = [base_a, base_b, top_b, top_a];
            let quad_screen: Vec<Vector2<f32>> = quad_world
                .iter()
                .filter_map(|point| self.world_to_screen(*point))
                .collect();
            if quad_screen.len() != 4{
                continue;
            }
            if Self::point_in_polygon(mouse, &quad_screen) {
                return true;
            }
            for edge in 0..4 {
                let next_edge = (edge+1) % 4;
                if Self::dist_to_segment(
                    mouse, quad_screen[edge], quad_screen[next_edge]
                ) <= threshold_sq
                {
                    return true;
                }
            }
        }
        false
    }
    pub fn begin_move_selected(&mut self, x: f32, y: f32) -> bool {
        let mouse = cgmath::Vector2::new(x, y);
        let threshold_sq = 8.0 * 8.0;
        let hit_selected = self
            .entities
            .iter()
            .filter(|entity| entity.selected)
            .any(|entity| {
                let screen_vertices: Vec<cgmath::Vector2<f32>> =entity
                    .vertices
                    .iter()
                    .filter_map(|point| self.world_to_screen(*point))
                    .collect();
                let segment_hit = screen_vertices.windows(2).any(|segment| {
                    Self::dist_to_segment(mouse, segment[0], segment[1]) <= threshold_sq
                });
                let interior_hit = entity.hatched
                    && Self::is_closed_shape(&entity.vertices)
                    && Self::point_in_polygon(mouse, &screen_vertices);
                interior_hit
                    || segment_hit
                    || self.hit_extruded_side(entity, mouse, threshold_sq)
                    || Self::circle_control_points(&entity.vertices)
                        .map(|(center, endpoint)| {
                            [center, endpoint]
                                .into_iter()
                                .filter_map(|point| self.world_to_screen(point))
                                .any(|screen| (screen - mouse).magnitude2() <= threshold_sq)
                        })
                        .unwrap_or(false)
            })
            || self
                .text_entities
                .iter()
                .filter(|text| text.selected)
                .any(|text| self.text_hit_test(text, mouse))
            || self
                .nurbs_surfaces
                .iter()
                .filter(|surface| surface.selected)
                .any(|surface| self.surface_hit_test(surface, mouse).is_some())
            || self
                .imported_meshes
                .iter()
                .filter(|mesh| mesh.selected)
                .any(|mesh| self.imported_mesh_hit_test(mesh, mouse).is_some());
        let screen_size = Vector2::new(self.width as f32, self.height as f32);
        let mut hit_gumball = false;
        if let Some(ray) = self.camera.screen_to_ray(mouse, screen_size) {
            if let Some(gumball) = &mut self.gumball {
                if let Some(handle) = gumball.hit_test(&ray) {
                    gumball.active = Some(handle);
                    gumball.drag_start_origin = gumball.origin;
                    gumball.drag_start_point = Some(Point3::new(x, y, 0.0));
                    hit_gumball = true;
                }
            }
        }
        if !hit_selected && !hit_gumball {
            self.move_anchor = None;
            self.move_snapshots.clear();
            self.move_solid_snapshots.clear();
            self.move_surface_snapshots.clear();
            self.move_mesh_snapshots.clear();
            self.move_text_snapshots.clear();
            return false;
        }
        self.move_axis_snapshots = self
            .entities
            .iter()
            .filter(|entity| entity.selected)
            .map(|entity| (entity.id, entity.extrude_axis))
            .collect();
        self.save_undo_state();
        self.move_anchor = self.fetch_point(PhysicalPosition::new(x as f64, y as f64));
        if self.move_anchor.is_some() {
            self.move_snapshots = self
                .entities
                .iter()
                .filter(|entity| entity.selected)
                .map(|entity| (entity.id, entity.vertices.clone()))
                .collect();
            self.move_surface_snapshots = self
                .nurbs_surfaces
                .iter()
                .filter(|surface| surface.selected)
                .map(|surface| (surface.id, surface.surface.clone()))
                .collect();
            self.move_mesh_snapshots = self
                .imported_meshes
                .iter()
                .filter(|mesh| mesh.selected)
                .map(|mesh| (mesh.id, mesh.triangles.clone()))
                .collect();
            self.move_text_snapshots = self
                .text_entities
                .iter()
                .filter(|text| text.selected)
                .map(|text| (text.id, text.pos, text.quat, text.cached_vertices.clone()))
                .collect();
            self.move_solid_snapshots = self
                .solids
                .iter()
                .filter(|mesh| mesh.selected)
                .map(|solid| (solid.id, solid.origin))
                .collect();
        }
        self.move_anchor.is_some()
    }
    fn rotate_gumball_snapshots(
        &mut self,
        axis: Vector3<f32>,
        angle: f32,
        pivot: Point3<f32>,
    ) {
        let axis = axis.normalize();
        let center = Vector3::new(pivot.x, pivot.y, pivot.z);
        let (sin_angle, cos_angle) = angle.sin_cos();
        let rotate_point = |point: Vector3<f32>| {
            let offset = point - center;
            center
                + offset * cos_angle
                + axis.cross(offset) * sin_angle
                + axis * axis.dot(offset) * (1.0 - cos_angle)
        };
        let rotate_vector = |vector: Vector3<f32>| {
            vector * cos_angle
                + axis.cross(vector) * sin_angle
                + axis * axis.dot(vector) * (1.0 - cos_angle)
        };
        for (entity_id, original_vertices) in &self.move_snapshots {
            let original_axis = self
                .move_axis_snapshots
                .iter()
                .find_map(|(id, axis)| (*id == *entity_id).then_some(*axis));
            if let Some(entity) = self
                .entities
                .iter_mut()
                .find(|entity| entity.id == *entity_id)
            {
                entity.vertices = original_vertices
                    .iter()
                    .copied()
                    .map(rotate_point)
                    .collect();
                if let Some(original_axis) = original_axis {
                    entity.extrude_axis = rotate_vector(original_axis);
                }
            }
        }
        for (surface_id, original_surface) in &self.move_surface_snapshots {
            if let Some(entity) = self
                .nurbs_surfaces
                .iter_mut()
                .find(|entity| entity.id == *surface_id)
            {
                entity.surface = original_surface.clone();
                for row in &mut entity.surface.control_points {
                    for control_point in row {
                        control_point.point = rotate_point(control_point.point);
                    }
                }
            }
        }
        for (mesh_id, original_triangles) in &self.move_mesh_snapshots {
            if let Some(mesh) = self
                .imported_meshes
                .iter_mut()
                .find(|mesh| mesh.id == *mesh_id)
            {
                mesh.triangles = Arc::new(
                    original_triangles
                        .iter()
                        .map(|triangle| triangle.map(rotate_point))
                        .collect(),
                );
            }
        }
        for (text_id, original_pos, original_quat, original_vertices) in &self.move_text_snapshots {
            if let Some(text) = self.text_entities.iter_mut().find(|text| text.id == *text_id) {
                text.pos = rotate_point(*original_pos);
                text.quat = (Quaternion::from_axis_angle(axis.normalize(), Rad(angle))
                    * *original_quat)
                    .normalize();
                text.cached_vertices = original_vertices
                    .iter()
                    .map(|vertex| {
                        let mut rotated = *vertex;
                        let point = rotate_point(Vector3::new(
                            vertex.position[0], vertex.position[1], vertex.position[2],
                        ));
                        rotated.position = [point.x, point.y, point.z];
                        rotated
                    })
                    .collect();
            }
        }
        for (solid_id, original_origin) in &self.move_solid_snapshots {
            if let Some(solid) = self.solids.iter_mut().find(|s| s.id == *solid_id) {
                let pt = Vector3::new(original_origin[0], original_origin[1],original_origin[2]);
                let rotated = rotate_point(pt);
                solid.origin = [rotated.x, rotated.y, rotated.z];
            }
        }
    }
    pub fn move_selected(&mut self, x: f32, y: f32) -> bool {
        let Some(anchor) = self.move_anchor else {
            return false;
        };
        self.cursor_pos = PhysicalPosition::new(x as f64, y as f64);
        let Some(current) = self.fetch_point(self.cursor_pos) else {
            return false;
        };
        let mut offset = self.grid_snap_offset(current - anchor);
        let mut pending_rotation: Option<(Vector3<f32>, f32, Point3<f32>)> = None;
        if let Some(gumball) = &self.gumball {
            if let Some(handle) = gumball.active {
                match handle {
                    gumball::GumballHandle::translate_x => {
                        offset = Vector3::new(offset.x, 0.0, 0.0);
                    }
                    gumball::GumballHandle::translate_y => {
                        if let Some(start) = gumball.drag_start_point {
                            let units_per_pixel = self.world_width_for_pixels(1.0);
                            offset = Vector3::new(0.0,(start.y - y) * units_per_pixel,0.0,);
                            }
                    }
                    gumball::GumballHandle::translate_z => {
                        offset = Vector3::new(0.0, 0.0, offset.z);
                    }
                    gumball::GumballHandle::translate_xy => {
                        offset.z = 0.0;
                    }
                    gumball::GumballHandle::translate_xz => {
                        offset.y = 0.0;
                    }
                    gumball::GumballHandle::translate_yz => {
                        offset.x = 0.0;
                    }
                    gumball::GumballHandle::rotate_x => {
                        if let Some(start) = gumball.drag_start_point {
                            let angle_rad = (y - start.y) * 0.02;
                            pending_rotation = Some((
                                Vector3::unit_x(), angle_rad, gumball.drag_start_origin
                            ));
                            self.gumball_rot_info = Some(("x".to_string(), angle_rad.to_degrees()));
                            offset = Vector3::new(0.0, 0.0, 0.0);
                        }
                    }
                    gumball::GumballHandle::rotate_y => {
                        if let Some(start) = gumball.drag_start_point {
                            let angle_rad = (x - start.x) * 0.02;
                            pending_rotation = Some((
                                Vector3::unit_y(), (x-start.x) * 0.02, gumball.drag_start_origin
                            ));
                            self.gumball_rot_info = Some(("x".to_string(), angle_rad.to_degrees()));
                            offset = Vector3::new(0.0, 0.0, 0.0);
                        }
                    }
                    gumball::GumballHandle::rotate_z => {
                        if let Some(start) = gumball.drag_start_point {
                            let angle_rad = (start.x - x) * 0.02;
                            pending_rotation = Some((
                                Vector3::unit_z(), (start.x-x) * 0.02, gumball.drag_start_origin));
                            self.gumball_rot_info = Some(("x".to_string(), angle_rad.to_degrees()));
                            offset = Vector3::new(0.0, 0.0, 0.0);
                        }
                    }
                }
            }
        }
        for (entity_id, original_vertices) in &self.move_snapshots {
            if let Some(entity) = self
                .entities
                .iter_mut()
                .find(|entity| entity.id == *entity_id)
            {
                entity.vertices = original_vertices
                    .iter()
                    .map(|vertex| *vertex + offset)
                    .collect();
            }
        }
        for (surface_id, original_surface) in &self.move_surface_snapshots {
            if let Some(surface_entity) = self
                .nurbs_surfaces
                .iter_mut()
                .find(|surface| surface.id == *surface_id)
            {
                surface_entity.surface = original_surface.clone();
                for row in &mut surface_entity.surface.control_points {
                    for control_point in row {
                        control_point.point += offset;
                    }
                }
            }
        }
        for (mesh_id, original_triangles) in &self.move_mesh_snapshots {
            if let Some(mesh) = self
                .imported_meshes
                .iter_mut()
                .find(|mesh| mesh.id == *mesh_id)
            {
                mesh.triangles = Arc::new(
                    original_triangles
                        .iter()
                        .map(|triangle| triangle.map(|point| point + offset))
                        .collect(),
                );
            }
        }
        for (text_id, original_pos, _original_quat, original_vertices) in &self.move_text_snapshots {
            if let Some(text) = self.text_entities.iter_mut().find(|text| text.id == *text_id) {
                text.pos = *original_pos + offset;
                text.cached_vertices = original_vertices
                    .iter()
                    .map(|vertex| {
                        let mut moved = *vertex;
                        moved.position[0] += offset.x;
                        moved.position[1] += offset.y;
                        moved.position[2] += offset.z;
                        moved
                    })
                    .collect();
            }
        }
        for (solid_id, original_origin) in &self.move_solid_snapshots {
            if let Some(solid) = self.solids.iter_mut().find(|s| s.id == *solid_id) {
                solid.origin = [
                    original_origin[0] + offset.x,
                    original_origin[1] + offset.y,
                    original_origin[2] + offset.z,
                ];
            }
        }
        if let Some((axis, angle, pivot)) = pending_rotation {
            self.rotate_gumball_snapshots(axis, angle, pivot);}
        if let Some(gumball) = &mut self.gumball {
            gumball.origin = Point3::new(
                gumball.drag_start_origin.x + offset.x,
                gumball.drag_start_origin.y + offset.y,
                gumball.drag_start_origin.z + offset.z);
        }
        self.rebuild_vertices();
        true
    }

    pub fn end_move_selected(&mut self) {
        if let Some(gumball) = &mut self.gumball {
            gumball.active = None;
            gumball.drag_start_origin = gumball.origin;
        }
        self.move_anchor = None;
        self.move_snapshots.clear();
        self.move_surface_snapshots.clear();
        self.move_mesh_snapshots.clear();
        self.move_text_snapshots.clear();
        self.move_solid_snapshots.clear();
    }
    pub fn world_to_screen(&self, world_pos: cgmath::Vector3<f32>) -> Option<cgmath::Vector2<f32>> {
        let view_proj = self.camera.build_view_projection_matrix();
        let clip_pos = view_proj * cgmath::Vector4::new(world_pos.x, world_pos.y, world_pos.z, 1.0);
        if clip_pos.w <= 0.0 {
            return None;
        }
        let ndc = clip_pos.truncate() / clip_pos.w;
        if ndc.z < 0.0 || ndc.z > 1.0 {
            return None;
        }
        let screen_x = (ndc.x + 1.0) * 0.5 * self.width as f32;
        let screen_y = (1.0 - ndc.y) * 0.5 * self.height as f32;
        Some(cgmath::Vector2::new(screen_x, screen_y))
    }

    pub fn click_sel_handle(&mut self, mouse_pos: PhysicalPosition<f64>) {
        let mouse_px = cgmath::Vector2::new(mouse_pos.x as f32, mouse_pos.y as f32);
        let hit_threshold = 10.0;
        self.select_shape(mouse_px, hit_threshold, false);
    }

    fn circle_control_points(points: &[Vector3<f32>]) -> Option<(Vector3<f32>, Vector3<f32>)> {
        if points.len() < 33 {
            return None;
        }

        let first = *points.first()?;
        let last = *points.last()?;
        if (first - last).magnitude2() > 0.0001 {
            return None;
        }

        let unique = &points[..points.len() - 1];
        let mut center = Vector3::new(0.0, 0.0, 0.0);
        for point in unique {
            center += *point;
        }
        center /= unique.len() as f32;

        let radii: Vec<f32> = unique
            .iter()
            .map(|point| {
                let offset = *point - center;
                (offset.x * offset.x + offset.z * offset.z).sqrt()
            })
            .collect();
        let radius = radii.iter().sum::<f32>() / radii.len() as f32;
        if radius <= f32::EPSILON {
            return None;
        }

        let largest_error = radii
            .iter()
            .map(|value| (value - radius).abs())
            .fold(0.0_f32, f32::max);
        if largest_error > radius * 0.02 {
            return None;
        }

        Some((center, first))
    }
    pub fn rebuild_vertices(&mut self) {
        let mut new_vertices = Vec::new();
        if self.right_view {
            let spacing = self.grid_spacing.clamp(0.01, 10.0);
            let extent = 20.0_f32;
            let center_y = self.camera.target.y;
            let center_z = self.camera.target.z;
            self.right_grid_center = Vector3::new(0.0, center_y, center_z);
            let first_y = ((center_y - extent) / spacing).floor() * spacing;
            let last_y = ((center_y + extent) / spacing).ceil() * spacing;
            let first_z = ((center_z - extent) / spacing).floor() * spacing;
            let last_z = ((center_z + extent) / spacing).ceil() * spacing;
            let grid_color = [0.30, 0.32, 0.35, 0.72];
            let grid_x = POLYLINE_HEIGHT * 0.5;
            let mut y = first_y;
            while y <= last_y {
                let line = [
                    Vector3::new(grid_x, y, center_z - extent),
                    Vector3::new(grid_x, y, center_z + extent),
                ];
                new_vertices.extend(self.tessellate_polyline(&line, 1.0, grid_color));
                y += spacing;
            }
            let mut z = first_z;
            while z <= last_z {
                let line = [
                    Vector3::new(grid_x, center_y - extent, z),
                    Vector3::new(grid_x, center_y + extent, z),
                ];
                new_vertices.extend(self.tessellate_polyline(&line, 1.0, grid_color));
                z += spacing;
            }
        }
        let marker_radius = self.world_width_for_pixels(3.5);
        let marker_color = [1.0, 1.0, 1.0, 1.0];
        for entity in &self.nurbs_surfaces {
            let draw_color = self.object_draw_color(entity.id, entity.selected, entity.color);
            let points = &entity.surface.control_points;
            let is_imported_triangle = 
                entity.surface.degree_u == 1
                && entity.surface.degree_v == 1
                && points.len() == 2
                && points[0].len() == 2
                && points[1].len() == 2
                && (points[1][0].point - points[1][1].point).magnitude2() <= 1.0e-12;
            if is_imported_triangle {
                let a = points[0][0].point;
                let b = points[0][1].point;
                let c = points[1][0].point;
                let cross = (b-a).cross(c-a);
                let normal = if cross.magnitude2() > 1.0e-12 {
                    cross.normalize()
                } else {
                    Vector3::unit_y()
                };
                let make_vertex = |p: Vector3<f32>| Vertex {
                    position: [p.x, p.y, p.z],
                    coords: [normal.x, normal.y, normal.z], color: draw_color};
                new_vertices.extend([make_vertex(a), make_vertex(b), make_vertex(c)]);
                continue;
            }
            let mesh_verts = entity.surface.sample_mesh(entity.u_subdivisions, entity.v_subdivisions, draw_color);
            new_vertices.extend(mesh_verts);
        }
        for mesh in &self.imported_meshes {
            let draw_color = self.object_draw_color(mesh.id, mesh.selected, mesh.color);
            for triangle in mesh.triangles.iter() {
                let [a, b, c] = *triangle;
                let cross = (b - a).cross(c - a);
                let normal = if cross.magnitude2() > 1.0e-12 {
                    cross.normalize()
                } else {
                    Vector3::unit_y()
                };
                for point in [a, b, c] {
                    new_vertices.push(Vertex {
                        position: [point.x, point.y, point.z],
                        coords: [normal.x, normal.y, normal.z],
                        color: draw_color,
                    });
                }
            }
        }
        for solid in &self.solids {
            let draw_color = self.object_draw_color(solid.id, solid.selected, solid.color);
            new_vertices.extend(solid.to_verts(draw_color));   
        }
        for entity in &self.nurbs_curves {
            let draw_color = self.object_draw_color(entity.id, entity.selected, entity.color);
            let curve_points = entity.curve.sample(64);
            new_vertices.extend(self.tessellate_polyline(&curve_points, entity.thickness, draw_color));
        }
        for entity in &self.entities {
            let draw_color = self.object_draw_color(entity.id, entity.selected, entity.color);
            let closed = Self::is_closed_shape(&entity.vertices);
            if entity.hatched && closed && entity.height.abs() <= f32::EPSILON {
                if let Some(plane) = Self::hatch_plane(&entity.vertices) {
                    let entity_area = Self::polygon_area_on_plane(&entity.vertices, &plane);
                    let is_inside_another_hatched_shape = self.entities.iter().any(|other| {
                        if other.id == entity.id
                            || !other.hatched
                            || !Self::is_closed_shape(&other.vertices)
                            || other.vertices.is_empty()
                            || !Self::is_coplanar_with_plane(&other.vertices, &plane)
                        {
                            return false;
                        }
                        Self::polygon_area_on_plane(&other.vertices, &plane) > entity_area
                            && Self::point_in_polygon_on_plane(
                                entity.vertices[0],
                                &other.vertices,
                                &plane,
                            )
                    });
                    if !is_inside_another_hatched_shape {
                        let mut contours: Vec<&[Vector3<f32>]> = vec![&entity.vertices];
                        for inner in &self.entities {
                            if inner.id == entity.id
                                || !inner.hatched
                                || !Self::is_closed_shape(&inner.vertices)
                                || inner.vertices.is_empty()
                                || !Self::is_coplanar_with_plane(&inner.vertices, &plane)
                            {
                                continue;
                            }
                            if Self::polygon_area_on_plane(&inner.vertices, &plane) < entity_area
                                && Self::point_in_polygon_on_plane(
                                    inner.vertices[0],
                                    &entity.vertices,
                                    &plane,
                                )
                            {
                                contours.push(inner.vertices.as_slice());
                            }
                        }
                        let mut fill_color = [draw_color[0], draw_color[1], draw_color[2], 0.45];
                        if draw_color[3] < 0.0 {
                            fill_color[3] = draw_color[3];
                        }
                        new_vertices.extend(Self::tessellate_fill_contours(&contours, fill_color));
                    }
                }
            }
            if closed && entity.height.abs() > f32::EPSILON {
                new_vertices.extend(Self::extrude(
                    &entity.vertices,
                    draw_color,
                    entity.height,
                    entity.extrude_axis,
                ));
            }
            let outline_color = if self.material_render_mode
                && self.object_materials.contains_key(&entity.id)
                && !entity.selected
            {
                entity.color
            } else {
                draw_color
            };
            let entity_vertices =
                self.tessellate_polyline(&entity.vertices, entity.thickness, outline_color);
            new_vertices.extend_from_slice(&entity_vertices);
            if entity.show_markers && entity.height.abs() <= f32::EPSILON && Self::circle_control_points(&entity.vertices).is_none(){
                let marker_count = if closed {
                    entity.vertices.len() - 1
                } else {
                    entity.vertices.len()
                };
                for point in entity.vertices.iter().take(marker_count) {
                    new_vertices.extend(self.tessellate_vertex_circle(
                        *point,
                        marker_radius,
                        marker_color,
                    ));
                }
            }
        }
        if let (Some(first), Some(second)) = (self.rectangle_first, self.rectangle_second) {
            let rectangle = Self::rectangle_points(first, second, self.right_view);
            new_vertices.extend(self.tessellate_polyline(
                &rectangle,
                POLYLINE_WIDTH_PIXELS,
                self.polyline_color,
            ));
            for point in rectangle.iter().take(4) {
                new_vertices.extend(self.tessellate_vertex_circle(
                    *point,
                    marker_radius,
                    marker_color,
                ));
            }
        }
        if let (Some(start), Some(end), Some(cursor)) = 
            (self.arc_start, self.arc_end, self.arc_cursor)
        {
            let points =Self::arc_points(
                start, end, cursor, self.arc_degrees, self.front_view, self.right_view
            );
            if points.len() >= 2 {
                new_vertices.extend(self.tessellate_polyline(
                    &points, POLYLINE_WIDTH_PIXELS, self.polyline_color
                ));
            }
        }
        if let (Some(center), Some(target)) = (self.circle_center, self.circle_target) {
            let preview_points = Self::circle_points_from_target(center, target, 64, self.right_view);
            if preview_points.len() >= 2 {
                new_vertices.extend(self.tessellate_polyline(
                    &preview_points,
                    POLYLINE_WIDTH_PIXELS,
                    self.polyline_color,
                ));
            }

            if (target - center).magnitude2() > f32::EPSILON {
                let radius_guide = [center, target];
                new_vertices.extend(self.tessellate_polyline(
                    &radius_guide,
                    1.0,
                    [1.0, 0.72, 0.25, 0.8],
                ));
            }

            for point in [center, target] {
                new_vertices.extend(self.tessellate_vertex_circle(
                    point,
                    marker_radius,
                    marker_color,
                ));
            }
        }


        if let (Some(base), Some(target)) = (self.copy_base, self.copy_target) {
            let offset = target - base;
            let preview_color = [1.0, 0.65, 0.15, 0.85];

            for source in &self.copy_sources {
                let preview_vertices: Vec<Vector3<f32>> = source
                    .vertices
                    .iter()
                    .map(|point| *point + offset)
                    .collect();
                if source.hatched && Self::is_closed_shape(&preview_vertices) {
                    let preview_fill = [source.color[0], source.color[1], source.color[2], 0.35];
                    new_vertices.extend(Self::tessellate_fill(&preview_vertices, preview_fill));
                }
                new_vertices.extend(self.tessellate_polyline(
                    &preview_vertices,
                    source.thickness,
                    preview_color,
                ))
            }

            if offset.magnitude2() > f32::EPSILON {
                let guide = [
                    base + Vector3::new(0.0, POLYLINE_HEIGHT * 3.0, 0.0),
                    target + Vector3::new(0.0, POLYLINE_HEIGHT * 3.0, 0.0),
                ];
                new_vertices.extend(self.tessellate_polyline(&guide, 1.0, [1.0, 0.72, 0.25, 0.75]));
            }
        }

        let mut preview_controls = self.active_polyline.clone();
        let raw_preview = self.preview_point;
        let distance_target = self.polyline_distance_target();
        let displayed_preview = distance_target.or(raw_preview);
        if let Some(preview_point) = displayed_preview {
            let should_append = preview_controls
                .last()
                .map(|last| (*last - preview_point).magnitude2() > f32::EPSILON)
                .unwrap_or(false);
            if should_append {
                preview_controls.push(preview_point);
            }
        }
        if let (Some(target), Some(cursor)) = (distance_target, raw_preview) {
            if (target - cursor).magnitude2() > f32::EPSILON {
                let guide = [
                    target + Vector3::new(0.0, POLYLINE_HEIGHT * 3.0, 0.0),
                    cursor + Vector3::new(0.0, POLYLINE_HEIGHT * 3.0, 0.0),
                ];
                new_vertices.extend(self.tessellate_polyline(&guide, 1.0, [1.0, 1.0, 1.0, 0.75]));
            }
        }
        let renderer_preview = if self.curve_active {
            if let Some(preview) = displayed_preview {
                Self::sample_curve_preview(
                    &self.active_polyline,
                    preview,
                    self.curve_subdivisions.max(4) as usize,
                )
            } else {
                Self::sample_curve(
                    &self.active_polyline,
                    self.curve_subdivisions.max(4) as usize,
                )
            }
        } else {
            preview_controls.clone()
        };
        if renderer_preview.len() > 1 {
            new_vertices.extend(self.tessellate_polyline(
                &renderer_preview,
                POLYLINE_WIDTH_PIXELS,
                self.polyline_color,
            ));
        }
        for point in &preview_controls {
            new_vertices.extend(self.tessellate_vertex_circle(
                *point,
                marker_radius,
                marker_color,
            ));
        }
        for entity in &self.text_entities {
            let draw_color = self.object_draw_color(entity.id, entity.selected, entity.color);
            let has_render_material = self.material_render_mode
                && self.object_materials.contains_key(&entity.id);
            if entity.selected || has_render_material {
                new_vertices.extend(entity.cached_vertices.iter().map(|vertex| {
                    let mut colored = *vertex;
                    colored.color = draw_color;
                    colored
                }));
            } else {
                new_vertices.extend_from_slice(&entity.cached_vertices);
            }
        }
        for entity in &self.nurbs_surfaces {
            let draw_color = self.object_draw_color(entity.id, entity.selected, entity.color);
            let mesh_verts = entity.surface.sample_mesh(entity.u_subdivisions, entity.v_subdivisions, draw_color);
            new_vertices.extend(mesh_verts);
            if entity.selected {
                let cage_color = [0.8, 0.8, 0.8, 0.6];
                let pt_color = [1.0, 0.5, 0.0, 1.0];
                let num_rows = entity.surface.control_points.len();
                let num_cols = if num_rows > 0 {
                    entity.surface.control_points[0].len()
                } else {
                    0
                };
                for row in &entity.surface.control_points {
                    let row_pts: Vec<Vector3<f32>> = row.iter().map(|cp| cp.point).collect();
                    if row_pts.len() >= 2 {
                        new_vertices.extend(self.tessellate_polyline(&row_pts, 1.0, cage_color));
                    }
                }
                for c_i in 0..num_cols {
                    let col_pts: Vec<Vector3<f32>> = (0..num_rows).map(|r_i| entity.surface.control_points[r_i][c_i].point).collect();
                    if col_pts.len() >= 2 {
                        new_vertices.extend(self.tessellate_polyline(&col_pts, 1.0, cage_color));
                    }
                }
                for row in &entity.surface.control_points {
                    for cp in row {
                        new_vertices.extend(self.tessellate_vertex_circle(cp.point, marker_radius, pt_color));
                    }
                }
            }
        }
        for entity in &self.nurbs_curves {
            let draw_color = if entity.selected { [1.0, 0.0, 0.0, 1.0] } else { entity.color };
            let curve_points = entity.curve.sample(64);
            new_vertices.extend(self.tessellate_polyline(&curve_points, entity.thickness, draw_color));
            if entity.selected || entity.show_control_points {
                let cp_points: Vec<Vector3<f32>> = entity.curve.control_points.iter().map(|cp| cp.point).collect();
                new_vertices.extend(self.tessellate_polyline(&cp_points, 1.0, [0.8, 0.8, 0.8, 0.5]));
                for pt in &cp_points {
                    new_vertices.extend(self.tessellate_vertex_circle(*pt, marker_radius, [1.0, 0.5, 0.0, 1.0]));
                }
            }
        }
        if let Some(g) = &self.gumball {
            new_vertices.extend(g.build_vertices());
        }
        self.point_vertices = new_vertices;
        self.redraw = true;
    }
    fn push_polyline_entity(
        &mut self, vertices: Vec<Vector3<f32>>, color: [f32; 4], thickness: f32, height:f32, extrude_axis: Vector3<f32>, show_markers: bool,
        selected:bool, hatched: bool
    )-> usize {
        let id = self.next_entity;
        self.next_entity += 1;
        self.entities.push(PolyLine {
            id, vertices, color, thickness, selected, height, extrude_axis, show_markers, hatched
        });
        id
    }
    pub fn add_nurbs_curve(&mut self, curve: NurbsCurve, color: [f32; 4], thickness: f32) -> usize {
        let id = self.next_entity;
        self.next_entity += 1;
        self.nurbs_curves.push(NurbsCurveEntity {
            id,
            curve,
            color,
            thickness,
            selected: false,
            show_control_points: false,
        });
        self.rebuild_vertices();
        id
    }
    pub fn add_nurbs_surface(&mut self, surface: NurbsSurface, color: [f32; 4], u_subdivisions: usize, v_subdivisions: usize) -> usize {
        let id = self.next_entity;
        self.next_entity += 1;
        self.nurbs_surfaces.push(NurbsSurfaceEntity {
            id,
            surface,
            color,
            selected: false,
            u_subdivisions,
            v_subdivisions,
        });
        self.rebuild_vertices();
        id
    }
    pub fn add_triangle_surfaces(
        &mut self,
        triangles: &[[Vector3<f32>; 3]],
        color: [f32; 4],
    ) -> usize {
        if triangles.is_empty() {
            return 0;
        }

        self.save_undo_state();
        let id = self.next_entity;
        self.next_entity += 1;
        let triangle_count = triangles.len();
        self.imported_meshes.push(ImportedMeshEntity {
            id,
            triangles: Arc::new(triangles.to_vec()),
            color,
            selected: false,
        });
        self.rebuild_vertices();
        triangle_count
    }
    pub fn extrude_selected(&mut self, height:f32) -> usize {
        if !height.is_finite() || height.abs() > 100_000.0 {
            return 0;
        }
        let selected_ids: Vec<usize> = self
            .entities
            .iter()
            .filter(|entity| {
                entity.selected && Self::is_closed_shape(&entity.vertices)
            })
            .map(|entity| entity.id)
            .collect();
        if selected_ids.is_empty() {
            return 0;
        }
        let fallback_axis = if self.right_view {
            Vector3::unit_x()
        } else if self.front_view {
            Vector3::unit_z()
        } else {
            Vector3::unit_y()
        };
        let view_direction = self.camera.eye - self.camera.target;
        self.save_undo_state();
        for entity in &mut self.entities {
            if selected_ids.contains(&entity.id) {
                entity.extrude_axis = Self::extrusion_axis_for_vertices(
                    &entity.vertices,
                    view_direction,
                    fallback_axis
                );
                entity.height = height;
            }
        }
        self.rebuild_vertices();
        selected_ids.len()
    }
    pub fn extrude_object(&mut self, object_id: usize, height: f32) -> bool {
        if !height.is_finite() || height.abs() > 100_000.0 {
            return false;
        }
        let Some(existing) = self
            .entities
            .iter()
            .find(|entity| entity.id == object_id)
        else {
            return false;
        };
        if !Self::is_closed_shape(&existing.vertices) {
            return false;
        }
        let fallback_axis = if self.right_view {
            Vector3::unit_x()
        } else if self.front_view {
            Vector3::unit_z()
        } else {
            Vector3::unit_y()
        };
        let view_direction = self.camera.eye - self.camera.target;
        let axis = Self::extrusion_axis_for_vertices(
            &existing.vertices,
            view_direction,
            fallback_axis
        );
        self.save_undo_state();
        let Some(entity) = self
            .entities
            .iter_mut()
            .find(|entity| entity.id == object_id)
        else{
            return false;
        };
        entity.height = height;
        entity.extrude_axis = axis;
        self.rebuild_vertices();
        true
    }
    pub fn begin_extrude_drag(&mut self) -> bool {
        let Some(id) = self.selected_entity.or_else(|| {
            self.entities
                .iter()
                .find(|entity| entity.selected)
                .map(|entity| entity.id)
        }) else {
            return false;
        };
        let Some(existing) = self.entities.iter().find(|entity| {
            entity.id == id && Self::is_closed_shape(&entity.vertices)
        }) else {
            return false;
        };
        let start_height = existing.height;
        let start_axis = existing.extrude_axis;
        let fallback_axis = if self.right_view {
            Vector3::unit_x()
        } else if self.front_view {
            Vector3::unit_z()
        } else {
            Vector3::unit_y()
        };
        let view_direction = self.camera.eye - self.camera.target;
        let axis = Self::extrusion_axis_for_vertices(
            &existing.vertices,
            view_direction,
            fallback_axis,
        );
        self.save_undo_state();
        if let Some(entity) = self.entities.iter_mut().find(|entity| entity.id== id) {
            entity.extrude_axis = axis;
        }
        self.extrude_drag = Some((id, start_height, start_axis));
        true
    }
    pub fn preview_extrude_drag(&mut self, delta_y: f32) -> bool {
        if !delta_y.is_finite() {
            return false;
        }
        let Some((id, start_height, _)) = self.extrude_drag else {
            return false;
        };
        let Some(entity) = self.entities.iter_mut().find(|entity| entity.id == id) else {
            return false;
        };
        entity.height = start_height + delta_y;
        self.rebuild_vertices();
        true
    }
    pub fn end_extrude_drag(&mut self, commit: bool) {
        let Some((id, start_height, start_axis)) = self.extrude_drag.take() else {
            return;
        };
        if !commit {
            if let Some(entity) = self.entities.iter_mut().find(|entity| entity.id == id) {
                entity.height = start_height;
                entity.extrude_axis = start_axis;
            }
            self.hist_entities.undo.pop();
            self.hist_entities.current = self.entities.clone();
            self.hist_surfaces.undo.pop();
            self.hist_surfaces.current = self.nurbs_surfaces.clone();
            if !self.hist_imported_meshes.undo.is_empty() {
                self.hist_imported_meshes.undo.pop();
            }
            self.hist_imported_meshes.current = self.imported_meshes.clone();
        }
        self.rebuild_vertices();
    }
    pub fn tessellate_polyline(
        &self,
        points: &[cgmath::Vector3<f32>],
        thickness: f32,
        color: [f32; 4],
    ) -> Vec<Vertex> {
        let mut vertices = Vec::new();
        if points.len() < 2 {
            return vertices;
        }
        let closed =
            points.len() > 2 && (points[0] - points[points.len() - 1]).magnitude2() <= f32::EPSILON;
        let point_count = if closed {
            points.len() - 1
        } else {
            points.len()
        };
        if point_count < 2 {
            return vertices;
        }

        let half_width = self.world_width_for_pixels(thickness) * 0.5;
        let lift = if self.right_view {
            Vector3::new(POLYLINE_HEIGHT, 0.0, 0.0)
        } else if self.front_view {
            Vector3::new(0.0, 0.0, POLYLINE_HEIGHT)
        } else {
            Vector3::new(0.0, POLYLINE_HEIGHT, 0.0)
        };
        let normal_for = |segment: Vector3<f32>| {
            let flat = if self.right_view {
                Vector3::new(0.0, segment.y, segment.z)
            } else if self.front_view {
                Vector3::new(segment.x, segment.y, 0.0)
            } else {
                Vector3::new(segment.x, 0.0, segment.z)
            };
            if flat.magnitude2() <= f32::EPSILON {
                None
            } else {
                let direction = flat.normalize();
                if self.right_view {
                    Some(Vector3::new(0.0, -direction.z, direction.y))
                } else if self.front_view {
                    Some(Vector3::new(-direction.y, direction.x, 0.0))
                } else {
                    Some(Vector3::new(-direction.z, 0.0, direction.x))
                }
            }
        };

        let mut edges = Vec::with_capacity(point_count);
        for index in 0..point_count {
            let current = points[index];
            let previous = if index == 0 {
                if closed {
                    points[point_count - 1]
                } else {
                    current
                }
            } else {
                points[index - 1]
            };
            let next = if index + 1 == point_count {
                if closed { points[0] } else { current }
            } else {
                points[index + 1]
            };

            let previous_normal = normal_for(current - previous);
            let next_normal = normal_for(next - current);
            let offset = match (previous_normal, next_normal) {
                (Some(previous_normal), Some(next_normal)) => {
                    let combined = previous_normal + next_normal;
                    if combined.magnitude2() <= f32::EPSILON {
                        next_normal * half_width
                    } else {
                        let miter = combined.normalize();
                        let denominator = miter.dot(next_normal).abs().max(0.25);
                        miter * (half_width / denominator).min(half_width * 4.0)
                    }
                }
                (Some(normal), None) | (None, Some(normal)) => normal * half_width,
                (None, None) => Vector3::new(half_width, 0.0, 0.0),
            };
            edges.push((current + offset + lift, current - offset + lift));
        }

        let segment_count = if closed { point_count } else { point_count - 1 };
        for index in 0..segment_count {
            let next = (index + 1) % point_count;
            let (left_start, right_start) = edges[index];
            let (left_end, right_end) = edges[next];
            for position in [
                left_start,
                right_start,
                left_end,
                left_end,
                right_start,
                right_end,
            ] {
                vertices.push(Vertex {
                    position: position.into(),
                    coords: [0.0; 3],
                    color,
                });
            }
        }
        vertices
    }

    fn world_width_for_pixels(&self, pixel_width: f32) -> f32 {
        let camera_distance = (self.camera.eye - self.camera.target)
            .magnitude()
            .max(0.001);
        let visible_height = 2.0 * camera_distance * (self.camera.fov_y.to_radians() * 0.5).tan();
        pixel_width * visible_height / self.height.max(1) as f32
    }

    fn tessellate_ground_point(
        point: Vector3<f32>,
        half_size: f32,
        color: [f32; 4],
    ) -> [Vertex; 6] {
        let y = point.y + POLYLINE_HEIGHT;
        let p0 = [point.x - half_size, y, point.z - half_size];
        let p1 = [point.x + half_size, y, point.z - half_size];
        let p2 = [point.x - half_size, y, point.z + half_size];
        let p3 = [point.x + half_size, y, point.z + half_size];
        [
            Vertex {
                position: p0,
                coords: [0.0; 3],
                color,
            },
            Vertex {
                position: p1,
                coords: [0.0; 3],
                color,
            },
            Vertex {
                position: p2,
                coords: [0.0; 3],
                color,
            },
            Vertex {
                position: p2,
                coords: [0.0; 3],
                color,
            },
            Vertex {
                position: p1,
                coords: [0.0; 3],
                color,
            },
            Vertex {
                position: p3,
                coords: [0.0; 3],
                color,
            },
        ]
    }
    fn tessellate_vertex_circle(
        &self,
        point: Vector3<f32>,
        radius: f32,
        color: [f32; 4],
    ) -> Vec<Vertex> {
        const SEGMENTS: usize = 16;

        let mut vertices = Vec::with_capacity(SEGMENTS * 3);
        let center = if self.right_view {
            Vector3::new(point.x + POLYLINE_HEIGHT * 2.0, point.y, point.z)
        } else if self.front_view {
            Vector3::new(point.x, point.y, point.z + POLYLINE_HEIGHT * 2.0)
        } else {
            Vector3::new(point.x, point.y + POLYLINE_HEIGHT * 2.0, point.z)
        };

        for index in 0..SEGMENTS {
            let angle1 = index as f32 / SEGMENTS as f32 * std::f32::consts::TAU;
            let angle2 = (index + 1) as f32 / SEGMENTS as f32 * std::f32::consts::TAU;

            let point1 = if self.right_view {
                Vector3::new(center.x, center.y + angle1.cos() * radius, center.z + angle1.sin() * radius)
            } else if self.front_view {
                Vector3::new(
                    center.x + angle1.cos() * radius,
                    center.y + angle1.sin() * radius,
                    center.z,
                )
            } else {
                Vector3::new(
                    center.x + angle1.cos() * radius,
                    center.y,
                    center.z + angle1.sin() * radius,
                )
            };
            let point2 = if self.right_view {
                Vector3::new(center.x, center.y + angle2.cos() * radius, center.z + angle2.sin() * radius)
            } else if self.front_view {
                Vector3::new(
                    center.x + angle2.cos() * radius,
                    center.y + angle2.sin() * radius,
                    center.z,
                )
            } else {
                Vector3::new(
                    center.x + angle2.cos() * radius,
                    center.y,
                    center.z + angle2.sin() * radius,
                )
            };

            for position in [center, point1, point2] {
                vertices.push(Vertex {
                    position: position.into(),
                    coords: [0.0; 3],
                    color,
                });
            }
        }

        vertices
    }
    pub fn add_sample_nurbs_surface(&mut self) -> usize {
        let mut control_points = Vec::new();
        for i in 0..4 {
            let mut row = Vec::new();
            for j in 0..4 {
                let x = (i as f32) * 1.0 - 1.5;
                let z = (j as f32) * 1.0 - 1.5;
                let dist_from_center = (x * x + z * z).sqrt();
                let y = (1.5 - dist_from_center).max(0.0) * 0.75;
                row.push(NurbsControlPoint {
                    point: Vector3::new(x, y, z),
                    weight: 1.0,
                });
            }
            control_points.push(row);
        }
        let knots_u = NurbsCurve::clamped_knot_vector(4, 3);
        let knots_v = NurbsCurve::clamped_knot_vector(4, 3);
        let surface = NurbsSurface {
            degree_u: 3,
            degree_v: 3,
            control_points,
            knots_u,
            knots_v,
        };
        self.add_nurbs_surface(surface, [0.2, 0.65, 0.95, 1.0], 32, 32)
    }
    pub fn add_polyline(&mut self, vertices: Vec<Vector3<f32>>, color: [f32; 4], thickness: f32) {
        self.save_undo_state();
        let id = self.next_entity;
        self.next_entity += 1;
        let hatched = false;
        self.entities.push(PolyLine {
            id,
            vertices,
            color,
            thickness,
            selected: false,
            height: 0.0,
            extrude_axis: Vector3::unit_y(),
            show_markers: true,
            hatched,
        })
    }
    pub fn graph_handle_key(&mut self, code: KeyCode, is_pressed: bool) -> bool {
        if code == KeyCode::KeyG {
            if is_pressed {
                if !self.grab_mode {
                    if let Some(selected_id) = self.selected_entity {
                        if let Some(entity) =
                            self.entities.iter().find(|entity| entity.id == selected_id)
                        {
                            self.grab_snapshot = entity.vertices.clone();
                            self.grab_origin = self.fetch_point(self.cursor_pos);
                            self.grab_mode = self.grab_origin.is_some();
                        }
                    }
                }
            } else {
                self.grab_mode = false;
                self.grab_origin = None;
                self.grab_snapshot.clear();
            }
            return true;
        }
        if !is_pressed {
            return false;
        }
        match code {
            KeyCode::Tab => {
                self.osnap = !self.osnap;
                self.redraw = true;
                true
            }
            _ => false,
        }
    }
    pub fn clear(&mut self) {
        if !self.entities.is_empty() {
            self.save_undo_state();
        }
        self.entities.clear();
        self.point_vertices.clear();
        self.active_polyline.clear();
        self.preview_point = None;
        self.selected_entity = None;
        self.polyline_active = false;
        self.curve_active = false;
        self.circle_center = None;
        self.circle_target = None;
        self.rectangle_first = None;
        self.rectangle_second = None;
        self.copy_sources.clear();
        self.copy_base = None;
        self.copy_target = None;
        self.rebuild_vertices();
        self.text_entities.clear();
        self.text_entities.clear();
        self.solids.clear();
        self.imported_meshes.clear();
        self.nurbs_surfaces.clear();
    }
    fn polyline_distance_target(&self) -> Option<Vector3<f32>> {
        let distance = self.polyline_distance?;
        let start = *self.active_polyline.last()?;
        let cursor = self.preview_point?;
        let direction = cursor - start;
        let horizontal_length = (direction.x * direction.x + direction.z * direction.z).sqrt();
        if horizontal_length <= f32::EPSILON {
            return None;
        }
        Some(Vector3::new(
            start.x + direction.x / horizontal_length * distance,
            start.y,
            start.z + direction.z / horizontal_length * distance,
        ))
    }
    pub fn set_polyline_distance(&mut self, distance: Option<f32>) {
        self.polyline_distance = distance.filter(|value| value.is_finite() && *value > 0.0);
        self.rebuild_vertices();
    }
    pub fn commit_polyline_distance(&mut self, distance: f32) -> bool {
        if !self.polyline_active
            || self.active_polyline.is_empty()
            || !distance.is_finite()
            || distance <= 0.0
        {
            return false;
        }
        self.polyline_distance = Some(distance);
        let Some(target) = self.polyline_distance_target() else {
            self.polyline_distance = None;
            return false;
        };
        self.active_polyline.push(target);
        self.preview_point = None;
        self.polyline_distance = None;
        self.rebuild_vertices();
        true
    }
    pub fn start_polyline(&mut self) {
        self.polyline_distance = None;
        self.active_polyline.clear();
        self.preview_point = None;
        self.curve_active = false;
        self.holding_left = false;
        self.polyline_active = true;
        self.rebuild_vertices();
    }
    pub fn finish_polyline(&mut self) {
        self.polyline_distance = None;
        self.holding_left = false;
        self.polyline_active = false;
        self.preview_point = None;
        if self.active_polyline.len() >= 2 {
            let points = mem::take(&mut self.active_polyline);
            self.add_polyline(points, self.polyline_color, POLYLINE_WIDTH_PIXELS);
        } else {
            self.active_polyline.clear();
        }
        self.rebuild_vertices();
    }
    pub fn cancel_polyline(&mut self) {
        self.polyline_distance = None;
        self.holding_left = false;
        self.polyline_active = false;
        self.active_polyline.clear();
        self.curve_active = false;
        self.preview_point = None;
        self.rebuild_vertices();
    }
    pub fn start_curve(&mut self) {
        self.active_polyline.clear();
        self.preview_point = None;
        self.holding_left = false;
        self.polyline_active = true;
        self.curve_active = true;
        self.rebuild_vertices();
    }
    pub fn finish_curve(&mut self) {
        let subdivisions = self.curve_subdivisions.max(4) as usize;
        self.draw_curve(subdivisions)
    }
    pub fn cancel_curve(&mut self) {
        self.curve_active = false;
        self.cancel_polyline();
    }
    pub fn set_osnap_modes(
        &mut self,
        end_enabled: bool,
        near_enabled: bool,
        mid_enabled: bool,
        int_enabled: bool,
    ) {
        self.end_snap_enabled = end_enabled;
        self.near_snap_enabled = near_enabled;
        self.mid_snap_enabled = mid_enabled;
        self.int_snap_enabled = int_enabled;
        self.osnap = end_enabled || near_enabled || mid_enabled || int_enabled;
        if !self.osnap && !self.construction_snap_enabled {
            self.preview_point = None;
            self.rebuild_vertices();
        }
    }
    pub fn set_construction_snap(&mut self, enabled: bool) {
        self.construction_snap_enabled = enabled;
    }

    pub fn set_grid_snap(&mut self, enabled: bool) {
        self.grid_snap_enabled = enabled;
    }

    pub fn set_grid_spacing(&mut self, spacing: f32) {
        if spacing.is_finite() && spacing > 0.0 {
            self.grid_spacing = spacing;
        }
    }
    pub fn set_ortho(&mut self, enabled: bool) {
        self.ortho_enabled = enabled;
        if self.polyline_active {
            self.preview_point = None;
            self.redraw = true;
        }
    }
    fn ortho_snap_point(&self, point: Vector3<f32>) -> Option<Vector3<f32>> {
        if !self.ortho_enabled {
            return None;
        }
        let anchor = *self.active_polyline.last()?;
        if self.right_view {
            let difference = point - anchor;
            if difference.y.abs() >= difference.z.abs() {
                Some(Vector3::new(anchor.x, point.y, anchor.z))
            } else {
                Some(Vector3::new(anchor.x, anchor.y, point.z))
            }
        } else if self.front_view {
            let difference = point-anchor;
            if difference.x.abs() >= difference.y.abs() {
                Some(Vector3::new(point.x, anchor.y, anchor.z))
            } else {
                Some(Vector3::new(anchor.x, point.y, anchor.z))
            }
        } else {
            let difference = point - anchor;
            if difference.x.abs() >= difference.z.abs() {
                Some(Vector3::new(point.x, anchor.y, anchor.z))
            } else {
                Some(Vector3::new(anchor.x, anchor.y, point.z))
            }
        }
    }
    fn grid_snap_point(&self, point: Vector3<f32>) -> Vector3<f32> {
        if !self.grid_snap_enabled {
            return point;
        }
        if self.right_view {
            Vector3::new(
                0.0,
                (point.y / self.grid_spacing).round() * self.grid_spacing,
                (point.z / self.grid_spacing).round() * self.grid_spacing,
            )
        } else if self.front_view {
            Vector3::new(
                (point.x / self.grid_spacing).round() * self.grid_spacing,
                (point.y / self.grid_spacing).round() * self.grid_spacing,
                0.0
            )
        } else {
            Vector3::new(
                (point.x / self.grid_spacing).round() * self.grid_spacing,
                0.0,
                (point.z / self.grid_spacing).round() * self.grid_spacing
            )
        }
    }

    fn grid_snap_offset(&self, offset: Vector3<f32>) -> Vector3<f32> {
        if !self.grid_snap_enabled {
            return offset;
        }
        if self.right_view {
            Vector3::new(
                offset.x,
                (offset.y / self.grid_spacing).round() * self.grid_spacing,
                (offset.z / self.grid_spacing).round() * self.grid_spacing,
            )
        } else if self.front_view {
            Vector3::new(
                (offset.x / self.grid_spacing).round() * self.grid_spacing,
                (offset.y / self.grid_spacing).round() * self.grid_spacing,
                offset.z
            )
        } else {
            Vector3::new(
                (offset.x / self.grid_spacing).round() * self.grid_spacing, offset.y,
                (offset.z / self.grid_spacing).round() * self.grid_spacing,
            )
        }
    }

    pub fn snap_cursor_position(&mut self) -> Option<(f32, f32)> {
        let point = self.fetch_point(self.cursor_pos)?;
        let snapped = if self.polyline_active {
            let (snapped, _, snap_kind) = self.get_snap_pos(point);
            snap_kind?;
            snapped
        } else if let Some(target) = self.circle_target {
            target
        } else if let Some(target) = self.copy_target {
            target
        } else if self.grid_snap_enabled {
            if let Some(anchor) = self.move_anchor {
                anchor + self.grid_snap_offset(point - anchor)
            } else if self.grab_mode {
                let origin = self.grab_origin?;
                origin + self.grid_snap_offset(point - origin)
            } else {
                return None;
            }
        } else {
            return None;
        };
        let screen = self.world_to_screen(snapped)?;
        Some((screen.x, screen.y))
    }
    pub fn cursor_world_position(&mut self) -> Option<(f32, f32, f32)> {
        let raw_point = self.fetch_point(self.cursor_pos)?;

        let point = if self.polyline_active {
            if let Some(preview) = self.preview_point {
                preview
            } else {
                self.get_snap_pos(raw_point).0
            }
        } else if self.grid_snap_enabled {
            self.grid_snap_point(raw_point)
        } else {
            raw_point
        };
        Some((point.x, point.y, point.z))
    }

    pub fn set_polyline_color(&mut self, red: f32, green: f32, blue:f32, alpha: f32) {
        let color = [
            red.clamp(0.0, 1.0),
            green.clamp(0.0, 1.0),
            blue.clamp(0.0, 1.0),
            alpha.clamp(0.0, 1.0)
        ];
        self.polyline_color = color;
        for entity in &mut self.entities {
            entity.color =color;
        }
        for curve in &mut self.nurbs_curves {
            curve.color = color;
        }
        for surface in &mut self.nurbs_surfaces{
            surface.color = color;
        }
        self.rebuild_vertices();
    }
    pub fn set_object_material(&mut self, object_id: usize, material: Option<String>) -> bool {
        if !self.entities.iter().any(|e| e.id == object_id)
            && !self.nurbs_curves.iter().any(|e| e.id == object_id)
            && !self.solids.iter().any(|e| e.id == object_id)
            && !self.imported_meshes.iter().any(|e| e.id == object_id)
            && !self.nurbs_surfaces.iter().any(|e| e.id == object_id)
            && !self.text_entities.iter().any(|e| e.id == object_id)
        {
            return false;}
        if let Some(name) = material {
            self.object_materials.insert(object_id, name);
        } else {
            self.object_materials.remove(&object_id);
        }
        self.rebuild_vertices();
        true
    }
    fn object_draw_color(&self, object_id: usize, selected: bool, base: [f32; 4]) -> [f32; 4] {
        if self.material_render_mode {
            if let Some(material) = self.object_materials.get(&object_id) {
                if let Some(index) = Self::material_texture_index(material) {
                    return [1.0, 1.0, 1.0, -((index + 1) as f32) / 8.0];
                }
            }
        }
        if selected {
            return [1.0, 0.8, 0.0, 1.0];
        }
        base
    }

    fn material_texture_index(name: &str) -> Option<usize> {
        let key: String = name
            .chars()
            .filter(|character| character.is_ascii_alphanumeric())
            .flat_map(char::to_lowercase)
            .collect();
        match key.as_str() {
            "concrete049" => Some(0),
            "diamondplate005b" => Some(1),
            "fabric004" => Some(2),
            "metal050a" => Some(3),
            "metal063" => Some(4),
            "plastic007" => Some(5),
            "rubber001" => Some(6),
            "wood051" => Some(7),
            _ => None,
        }
    }

    pub fn set_material_render_mode(&mut self, enabled: bool) {
        self.material_render_mode = enabled;
        self.rebuild_vertices();
    }

    fn segment_intersection_on_view_plane(
        first_start: Vector3<f32>,
        first_end: Vector3<f32>,
        second_start: Vector3<f32>,
        second_end: Vector3<f32>,
        front_view: bool,
        right_view: bool,
    ) -> Option<Vector3<f32>> {
        let plane_point = |point: Vector3<f32>| {
            if right_view {
                (point.y, point.z)
            } else if front_view {
                (point.x, point.y)
            } else {
                (point.x, point.z)
            }
        };
        let (a_x, a_y) = plane_point(first_start);
        let (b_x, b_y) = plane_point(first_end);
        let (c_x, c_y) = plane_point(second_start);
        let (d_x, d_y) = plane_point(second_end);
        let rx = b_x - a_x;
        let ry = b_y - a_y;
        let sx = d_x - c_x;
        let sy = d_y - c_y;
        let denominator = rx * sy - ry * sx;
        if denominator.abs() <= 0.000001 {
            return None;
        }
        let qx = c_x - a_x;
        let qy = c_y - a_y;
        let t = (qx * sy - qy * sx) / denominator;
        let u = (qx * ry - qy * rx) / denominator;
        if !(0.0..=1.0).contains(&t) || !(0.0..=1.0).contains(&u) {
            return None;
        }
        let (x, y) = (a_x + rx * t, a_y + ry * t);
        Some(if right_view {
            Vector3::new(0.0, x, y)
        } else if front_view {
            Vector3::new(x, y, 0.0)
        } else {
            Vector3::new(x, first_start.y, y)
        })
    }

    pub fn get_snap_pos(&self, point: Vector3<f32>) -> (Vector3<f32>, bool, Option<&'static str>) {
        if self.end_snap_enabled || self.construction_snap_enabled {
            let radius = self.world_width_for_pixels(END_SNAP_RADIUS_PIXELS);
            let mut best_end: Option<(f32, Vector3<f32>, bool)> = None;
            let mut consider_end = |candidate: Vector3<f32>, closes_active: bool| {
                let distance = (point - candidate).magnitude();
                if distance <= radius
                    && best_end
                        .as_ref()
                        .map(|(best_distance, _, _)| distance < *best_distance)
                        .unwrap_or(true)
                {
                    best_end = Some((distance, candidate, closes_active));
                }
            };

            if self.active_polyline.len() >= 3 {
                consider_end(self.active_polyline[0], true);
            }
            for entity in &self.entities {
                for &vertex in &entity.vertices {
                    consider_end(vertex, false);
                }
            }
            if let Some((_, position, closes)) = best_end {
                return (position, closes, Some("End"));
            }
        }
        if self.mid_snap_enabled || self.construction_snap_enabled {
            let mut best_mid: Option<(f32, Vector3<f32>)> = None;
            if let Some(mouse_screen) = self.world_to_screen(point) {
                let mut check_segment = |start: Vector3<f32>, end: Vector3<f32>| {
                    let midpoint = (start + end) * 0.5;
                    if let Some(mid_screen) = self.world_to_screen(midpoint) {
                        let distance = (mid_screen - mouse_screen).magnitude();
                        if distance <= END_SNAP_RADIUS_PIXELS
                            && best_mid
                                .as_ref()
                                .map(|(best, _)| distance < *best)
                                .unwrap_or(true)
                        {
                            best_mid = Some((distance, midpoint));
                        }
                    }
                };
                for entity in &self.entities {
                    for segment in entity.vertices.windows(2) {
                        check_segment(segment[0], segment[1]);
                    }
                }
                for segment in self.active_polyline.windows(2) {
                    check_segment(segment[0], segment[1]);
                }
            }
            if let Some((_, midpoint)) = best_mid {
                return (midpoint, false, Some("Mid"));
            }
        }

        if self.int_snap_enabled || self.construction_snap_enabled {
            let mut segments = Vec::new();
            for entity in &self.entities {
                for segment in entity.vertices.windows(2) {
                    segments.push((segment[0], segment[1]));
                }
            }
            let mut best_intersection: Option<(f32, Vector3<f32>)> = None;
            if let Some(mouse_screen) = self.world_to_screen(point) {
                for first_index in 0..segments.len() {
                    for second_index in (first_index + 1)..segments.len() {
                        let (a, b) = segments[first_index];
                        let (c, d) = segments[second_index];
                        let shares_endpoint = (a - c).magnitude2() < 0.000001
                            || (a - d).magnitude2() < 0.000001
                            || (b - c).magnitude2() < 0.000001
                            || (b - d).magnitude2() < 0.000001;
                        if shares_endpoint {
                            continue;
                        }
                        let Some(intersection) = Self::segment_intersection_on_view_plane(
                            a, b, c, d, self.front_view, self.right_view,
                        ) else {
                            continue;
                        };
                        let Some(screen) = self.world_to_screen(intersection) else {
                            continue;
                        };
                        let distance = (screen - mouse_screen).magnitude();
                        if distance <= END_SNAP_RADIUS_PIXELS
                            && best_intersection
                                .as_ref()
                                .map(|(best, _)| distance < *best)
                                .unwrap_or(true)
                        {
                            best_intersection = Some((distance, intersection));
                        }
                    }
                }
            }
            if let Some((_, intersection)) = best_intersection {
                return (intersection, false, Some("Int"));
            }
        }
        if self.near_snap_enabled {
            let mut best_near: Option<(f32, Vector3<f32>)> = None;
            if let Some(mouse_screen) = self.world_to_screen(point) {
                let mut consider_segment = |start: Vector3<f32>, end: Vector3<f32>| {
                    let (Some(start_screen), Some(end_screen)) =
                        (self.world_to_screen(start), self.world_to_screen(end))
                    else {
                        return;
                    };
                    let screen_direction = end_screen - start_screen;
                    let screen_length_squared = screen_direction.magnitude2();
                    if screen_length_squared <= f32::EPSILON {
                        return;
                    }
                    let amount = ((mouse_screen - start_screen).dot(screen_direction)
                        / screen_length_squared)
                        .clamp(0.0, 1.0);
                    let projected_screen = start_screen + screen_direction * amount;
                    let distance_pixels = (mouse_screen - projected_screen).magnitude();
                    if distance_pixels <= NEAR_SNAP_RADIUS_PIXELS
                        && best_near
                            .as_ref()
                            .map(|(best_distance, _)| distance_pixels < *best_distance)
                            .unwrap_or(true)
                    {
                        let candidate = start + (end - start) * amount;
                        best_near = Some((distance_pixels, candidate));
                    }
                };

                for segment in self.active_polyline.windows(2) {
                    consider_segment(segment[0], segment[1]);
                }
                for entity in &self.entities {
                    for segment in entity.vertices.windows(2) {
                        consider_segment(segment[0], segment[1]);
                    }
                }
            }
            if let Some((_, position)) = best_near {
                return (position, false, Some("Near"));
            }
        }

        let grid_point = self.grid_snap_point(point);
        if let Some(ortho_point) = self.ortho_snap_point(grid_point) {
            return (ortho_point, false, Some("Ortho"));
        }
        if self.grid_snap_enabled {
            return (grid_point, false, Some("Grid"));
        }
        (point, false, None)
    }
    pub fn fetch_point(&mut self, mouse_pos: PhysicalPosition<f64>) -> Option<Vector3<f32>> {
        let (mouse_x, mouse_y) = (mouse_pos.x, mouse_pos.y);
        let (width, height) = (self.width, self.height);
        if width == 0 || height == 0 {
            return Some(Vector3::zero());
        }
        let ndc_x = (2.0 * mouse_x / width as f64) - 1.0;
        let ndc_y = 1.0 - (2.0 * mouse_y / height as f64);
        let _ndc = Vector4::new(ndc_x as f32, ndc_y as f32, 1.0, 1.0);
        let view_projection = self.camera.build_view_projection_matrix();
        let inverse = view_projection.invert()?;
        let near_ndc = Vector4::new(ndc_x as f32, ndc_y as f32, 0.0, 1.0);
        let far_ndc = Vector4::new(ndc_x as f32, ndc_y as f32, 1.0, 1.0);
        let near_world = inverse * near_ndc;
        let far_world = inverse * far_ndc;
        if near_world.w == 0.0 || far_world.w == 0.0 {
            return None;
        }
        let ray_origin = near_world.truncate() / near_world.w;
        let far_point = far_world.truncate() / far_world.w;
        let ray_dir = (far_point - ray_origin).normalize();
        if self.right_view {
            if ray_dir.x.abs() > 1e-6 {
                let t = -ray_origin.x / ray_dir.x;
                return Some(ray_origin + ray_dir * t);
            }
        } else if self.front_view {
            if ray_dir.z.abs() > 1e-6 {
                let t = -ray_origin.z / ray_dir.z;
                return Some(ray_origin +ray_dir * t);
            }
        } else {
            let t = -ray_origin.y / ray_dir.y;
            return Some(ray_origin + ray_dir * t);
        }
        Some(Vector3::zero())
    }
    pub fn drawing(
        &mut self,
        mouse_pos: PhysicalPosition<f64>,
        mouse_button: MouseButton,
        is_pressed: bool,
    ) {
        if !is_pressed {
            return;
        }
        match mouse_button {
            MouseButton::Right => {
                let hit_pos = self.fetch_point(mouse_pos);
                self.add_point(hit_pos.expect("Error unwrapping hit_pos"));
            }
            MouseButton::Left => {
                let _ = self.polyline(mouse_pos);
            }
            _ => {}
        }
    }
    pub fn vertice_append(&mut self, vertices: &[Vertex]) {
        if vertices.is_empty() {
            return;
        }
        self.point_vertices.extend_from_slice(vertices);
        self.redraw = true;
    }
    pub fn polyline(&mut self, mouse_pos: PhysicalPosition<f64>) -> bool {
        let hit_pos = match self.fetch_point(mouse_pos) {
            Some(hit_pos) => hit_pos,
            None => return false,
        };
        let (normal_snap, shape_close, _) = self.get_snap_pos(hit_pos);
        let distance_target = self.polyline_distance_target();
        let snap_pos = distance_target.unwrap_or(normal_snap);
        let should_close = distance_target.is_none() && shape_close;
        self.polyline_distance = None;
        if should_close && !self.curve_active {
            self.active_polyline.push(snap_pos);
            let points = self.active_polyline.clone();
            self.add_polyline(points, self.polyline_color, POLYLINE_WIDTH_PIXELS);
            self.active_polyline.clear();
            self.preview_point = None;
            self.polyline_active = false;
            self.holding_left = false;
            self.rebuild_vertices();
            true
        } else {
            self.active_polyline.push(snap_pos);
            self.preview_point = None;
            self.rebuild_vertices();
            false
        }
    }
    fn circle_points_from_target(
        center: Vector3<f32>,
        target: Vector3<f32>,
        subdivisions: usize,
        right_view: bool,
    ) -> Vec<Vector3<f32>> {
        let subdivisions = subdivisions.max(16);
        let delta = target - center;
        let radius = if right_view {
            (delta.y * delta.y + delta.z * delta.z).sqrt()
        } else {
            (delta.x * delta.x + delta.z * delta.z).sqrt()
        };
        if radius <= f32::EPSILON {
            return Vec::new();
        }
        let start_angle = if right_view { delta.z.atan2(delta.y) } else { delta.z.atan2(delta.x) };
        let mut points = Vec::with_capacity(subdivisions + 1);
        for index in 0..=subdivisions {
            let angle = start_angle + std::f32::consts::TAU * index as f32 / subdivisions as f32;
            points.push(if right_view {
                Vector3::new(center.x, center.y + radius * angle.cos(), center.z + radius * angle.sin())
            } else {
                Vector3::new(
                    center.x + radius * angle.cos(),
                    center.y,
                    center.z + radius * angle.sin(),
                )
            });
        }
        points
    }

    fn rectangle_points(first: Vector3<f32>, second: Vector3<f32>, right_view: bool) -> Vec<Vector3<f32>> {
        if right_view {
            return vec![
                first,
                Vector3::new(first.x, second.y, first.z),
                Vector3::new(first.x, second.y, second.z),
                Vector3::new(first.x, first.y, second.z),
                first,
            ];
        }
        vec![
            first,
            Vector3::new(second.x, first.y, first.z),
            Vector3::new(second.x, first.y, second.z),
            Vector3::new(first.x, first.y, second.z),
            first,
        ]
    }
    pub fn start_rectangle(&mut self) {
        self.rectangle_first = None;
        self.rectangle_second = None;
        self.active_polyline.clear();
        self.preview_point = None;
        self.polyline_distance = None;
        self.curve_active = false;
        self.holding_left = false;
        self.polyline_active = true;
        self.rebuild_vertices();
    }
    pub fn begin_rectangle(&mut self, x: f32, y: f32) -> bool {
        self.cursor_pos = PhysicalPosition::new(x as f64, y as f64);
        let Some(raw_point) = self.fetch_point(self.cursor_pos) else {
            return false;
        };
        let first = self.get_snap_pos(raw_point).0;
        self.rectangle_first = Some(first);
        self.rectangle_second = Some(first);
        self.rebuild_vertices();
        true
    }
    pub fn update_rectangle_preview(&mut self, x: f32, y: f32) -> Option<(f32, f32)> {
        let first = self.rectangle_first?;
        self.cursor_pos = PhysicalPosition::new(x as f64, y as f64);
        let raw_point = self.fetch_point(self.cursor_pos)?;
        let second = self.get_snap_pos(raw_point).0;
        self.rectangle_second = Some(second);
        self.rebuild_vertices();
        Some(((second.x - first.x).abs(), (second.z - first.z).abs()))
    }
    pub fn commit_rectangle(&mut self) -> bool {
        let (Some(first), Some(second)) = (self.rectangle_first, self.rectangle_second) else {
            return false;
        };
        let width = (second.x - first.x).abs();
        let height = (second.z - first.z).abs();
        if width <= f32::EPSILON || height <= f32::EPSILON {
            return false;
        }
        let points = Self::rectangle_points(first, second, self.right_view);
        self.rectangle_first = None;
        self.rectangle_second = None;
        self.polyline_active = false;
        self.active_polyline.clear();
        self.preview_point = None;
        self.add_polyline(points, self.polyline_color, POLYLINE_WIDTH_PIXELS);
        true
    }
    pub fn cancel_rectangle(&mut self) {
        self.rectangle_first = None;
        self.rectangle_second = None;
        self.polyline_active = false;
        self.active_polyline.clear();
        self.preview_point = None;
        self.rebuild_vertices();
    }

    pub fn begin_circle(&mut self, x: f32, y: f32) -> bool {
        self.cursor_pos = PhysicalPosition::new(x as f64, y as f64);
        let Some(raw_point) = self.fetch_point(self.cursor_pos) else {
            return false;
        };
        let center = self.get_snap_pos(raw_point).0;
        self.circle_center = Some(center);
        self.circle_target = Some(center);
        self.rebuild_vertices();
        true
    }

    pub fn update_circle_preview(&mut self, x: f32, y: f32) -> Option<f32> {
        let center = self.circle_center?;
        self.cursor_pos = PhysicalPosition::new(x as f64, y as f64);
        let raw_point = self.fetch_point(self.cursor_pos)?;
        let target = self.get_snap_pos(raw_point).0;
        self.circle_target = Some(target);
        let difference = target - center;
        let radius = if self.right_view {
            (difference.y * difference.y + difference.z * difference.z).sqrt()
        } else {
            (difference.x * difference.x + difference.z * difference.z).sqrt()
        };
        self.rebuild_vertices();
        Some(radius)
    }
    pub fn set_circle_radius(&mut self, radius: f32) -> bool {
        if !radius.is_finite() || radius <= 0.0 {
            return false;
        }
        let Some(center) = self.circle_center else {
            return false;
        };
        let current_target = self
            .circle_target
            .unwrap_or(center + Vector3::new(1.0, 0.0, 0.0));
        let offset = current_target - center;
        let planar_length = if self.right_view {
            (offset.y * offset.y + offset.z * offset.z).sqrt()
        } else {
            (offset.x * offset.x + offset.z * offset.z).sqrt()
        };
        let direction = if planar_length > 0.0001 {
            if self.right_view {
                Vector3::new(0.0, offset.y / planar_length, offset.z / planar_length)
            } else {
                Vector3::new(offset.x / planar_length, 0.0, offset.z / planar_length)
            }
        } else {
            if self.right_view { Vector3::unit_y() } else { Vector3::unit_x() }
        };
        self.circle_target = Some(center + direction * radius);
        self.rebuild_vertices();
        true
    }
    pub fn commit_circle(&mut self, subdivisions: usize) -> bool {
        let (Some(center), Some(target)) = (self.circle_center, self.circle_target) else {
            return false;
        };
        let points = Self::circle_points_from_target(center, target, subdivisions, self.right_view);
        self.circle_center = None;
        self.circle_target = None;
        if points.is_empty() {
            self.rebuild_vertices();
            return false;
        }
        self.add_polyline(points, self.polyline_color, POLYLINE_WIDTH_PIXELS);
        true
    }

    pub fn cancel_circle(&mut self) {
        self.circle_center = None;
        self.circle_target = None;
        self.rebuild_vertices();
    }
    fn arc_points(center: Vector3<f32>, radius_point: Vector3<f32>, cursor: Vector3<f32>, degrees: f32, front_view: bool, right_view: bool) -> Vec<Vector3<f32>> {
        let (cx, cy, start_x, start_y, cursor_x, cursor_y) = if right_view {
            (center.y, center.z, radius_point.y, radius_point.z, cursor.y, cursor.z)
        } else if front_view {
            (
                center.x, center.y, radius_point.x, radius_point.y, cursor.x, cursor.y
            )
        } else {
            (
                center.x, center.z,
                radius_point.x, radius_point.z, cursor.x, cursor.z
            )
        };
        let dx = start_x - cx;
        let dy = start_y - cy;
        let radius = (dx * dx + dy * dy).sqrt();
        if radius < 1.0e-5 || !degrees.is_finite() || ! (1.0..359.0).contains(&degrees) {
            return Vec::new();
        }
        let cursor_dx = cursor_x - cx;
        let cursor_dy = cursor_y - cy;
        let cross = dx * cursor_dy - dy * cursor_dx;
        let direction = if cross >= 0.0 {1.0} else {-1.0};
        let start_angle = dy.atan2(dx);
        let sweep  =degrees.to_radians() * direction;
        let segments = ((degrees / 3.0).ceil() as usize).clamp(12, 120);
        let mut points = Vec::with_capacity(segments +1);
        for i in 0..=segments {
            let angle = start_angle + sweep * i as f32 / segments as f32;
            let u = cx + radius * angle.cos();
            let v = cy +radius * angle.sin();
            points.push(if right_view {
                Vector3::new(center.x, u, v)
            } else if front_view {
                Vector3::new(u, v, center.z)
            } else {
                Vector3::new(u, center.y, v)
            });
        }
        points
    }
    pub fn start_arc(&mut self) {
        self.arc_start = None;
        self.arc_end = None;
        self.arc_cursor = None;
        self.arc_degrees = 90.0;
        self.rebuild_vertices();
    }
    pub fn begin_arc_start(&mut self, x: f32, y:f32) -> bool {
        self.cursor_pos = PhysicalPosition::new(x as f64, y as f64);
        let Some(point) = self.fetch_point(self.cursor_pos) else {
            return false;
        };
        self.arc_start =Some(self.get_snap_pos(point).0);
        self.rebuild_vertices();
        true
    }
    pub fn set_arc_end(&mut self, x: f32, y:f32) -> bool {
        let Some(start) = self.arc_start else {
            return false;
        };
        self.cursor_pos = PhysicalPosition::new(x as f64, y as f64);
        let Some(point) = self.fetch_point(self.cursor_pos) else {
            return false;
        };
        let end = self.get_snap_pos(point).0;
        if (end-start).magnitude2() < 1.0e-8 {
            return false;
        }
        self.arc_end = Some(end);
        self.arc_cursor =Some(end);
        self.rebuild_vertices();
        true
    }
    pub fn update_arc_preview(&mut self, x:f32, y :f32) {
        if self.arc_end.is_none() {
            return;
        }
        self.cursor_pos = PhysicalPosition::new(x as f64, y as f64);
        if let Some(point) = self.fetch_point(self.cursor_pos) {
            self.arc_cursor = Some(self.get_snap_pos(point).0);
            self.rebuild_vertices();
        }
    }
    pub fn set_arc_degrees(&mut self, degrees: f32) -> bool {
        if !degrees.is_finite() || !(1.0..359.0).contains(&degrees) {
            return false;
        }
        self.arc_degrees= degrees;
        self.rebuild_vertices();
        true
    }
    pub fn commit_arc(&mut self) -> bool {
        let (Some(start), Some(end), Some(cursor)) = 
            (self.arc_start, self.arc_end, self.arc_cursor)
        else {
            return false;
        };
        let points = Self::arc_points(
            start, end, cursor, self.arc_degrees, self.front_view, self.right_view
        );
        self.cancel_arc();
        if points.len() < 2 {
            return false;
        }
        self.add_polyline(points, self.polyline_color, POLYLINE_WIDTH_PIXELS);
        true
    }
    pub fn cancel_arc(&mut self) {
        self.arc_start = None;
        self.arc_end = None;
        self.arc_cursor = None;
        self.rebuild_vertices();
    }

    pub fn draw_circle_mouse(
        &mut self,
        subdivisions: usize,
        mouse_pos: PhysicalPosition<f64>,
        mouse_button: MouseButton,
    ) {
        if mouse_button != MouseButton::Left {
            return;
        }
        if self.circle_center.is_none() {
            self.begin_circle(mouse_pos.x as f32, mouse_pos.y as f32);
        } else {
            self.update_circle_preview(mouse_pos.x as f32, mouse_pos.y as f32);
            self.commit_circle(subdivisions);
        }
    }
    pub fn draw_circle_command(&mut self, subdivisions: usize, radius: f32, center: Vector3<f32>) {
        if subdivisions < 3 || !radius.is_finite() || radius <= 0.0 {
            return;
        }
        let target = center + if self.right_view {
            Vector3::new(0.0, radius, 0.0)
        } else {
            Vector3::new(radius, 0.0, 0.0)
        };
        let points = Self::circle_points_from_target(center, target, subdivisions, self.right_view);
        if !points.is_empty() {
            self.add_polyline(points, self.polyline_color, POLYLINE_WIDTH_PIXELS);
        }
    }
    fn curve_knot(previous: f32, first: Vector3<f32>, second: Vector3<f32>) -> f32 {
        previous + (second - first).magnitude().max(0.00001).sqrt()
    }

    fn curve_lerp(
        first: Vector3<f32>,
        second: Vector3<f32>,
        first_time: f32,
        second_time: f32,
        time: f32,
    ) -> Vector3<f32> {
        let difference = second_time - first_time;
        if difference.abs() <= 0.000001 {
            return first;
        }
        first * ((second_time - time) / difference)
            + second * ((time - first_time) / difference)
    }

    fn centripetal_curve_point(
        p0: Vector3<f32>,
        p1: Vector3<f32>,
        p2: Vector3<f32>,
        p3: Vector3<f32>,
        amount: f32,
    ) -> Vector3<f32> {
        let t0 = 0.0;
        let t1 = Self::curve_knot(t0, p0, p1);
        let t2 = Self::curve_knot(t1, p1, p2);
        let t3 = Self::curve_knot(t2, p2, p3);
        let time = t1 + (t2 - t1) * amount;
        let a1 = Self::curve_lerp(p0, p1, t0, t1, time);
        let a2 = Self::curve_lerp(p1, p2, t1, t2, time);
        let a3 = Self::curve_lerp(p2, p3, t2, t3, time);
        let b1 = Self::curve_lerp(a1, a2, t0, t2, time);
        let b2 = Self::curve_lerp(a2, a3, t1, t3, time);
        Self::curve_lerp(b1, b2, t1, t2, time)
    }

    fn sample_curve(points: &[Vector3<f32>], subdivisions: usize) -> Vec<Vector3<f32>> {
        let mut fit_points = Vec::new();
        for point in points {
            let different = fit_points
                .last()
                .map(|last: &Vector3<f32>| (*last - *point).magnitude2() > 0.0000001)
                .unwrap_or(true);
            if different {
                fit_points.push(*point);
            }
        }
        if fit_points.len() < 2 {
            return fit_points;
        }
        let closed = fit_points.len() >= 4
            && (fit_points[0] - fit_points[fit_points.len() - 1]).magnitude2() < 0.000001;
        if closed {
            fit_points.pop();
        }
        if fit_points.len() < 2 {
            return fit_points;
        }
        if fit_points.len() == 2 {
            return fit_points;
        }
        let steps = subdivisions.max(24);
        let mut sampled = Vec::new();

        if closed {
            let count = fit_points.len();
            for index in 0..count {
                let p0 = fit_points[(index + count - 1) % count];
                let p1 = fit_points[index];
                let p2 = fit_points[(index + 1) % count];
                let p3 = fit_points[(index + 2) % count];
                for step in 0..steps {
                    sampled.push(Self::centripetal_curve_point(
                        p0,
                        p1,
                        p2,
                        p3,
                        step as f32 / steps as f32,
                    ));
                }
            }
            if let Some(first) = sampled.first().copied() {
                sampled.push(first);
            }
        } else {
            for index in 0..fit_points.len() - 1 {
                let p1 = fit_points[index];
                let p2 = fit_points[index + 1];
                let p0 = if index == 0 {
                    p1 + (p1 - p2)
                } else {
                    fit_points[index - 1]
                };
                let p3 = if index + 2 < fit_points.len() {
                    fit_points[index + 2]
                } else {
                    p2 + (p2 - p1)
                };
                for step in 0..steps {
                    sampled.push(Self::centripetal_curve_point(
                        p0,
                        p1,
                        p2,
                        p3,
                        step as f32 / steps as f32,
                    ));
                }
            }
            sampled.push(*fit_points.last().unwrap());
        }
        sampled
    }

    fn sample_curve_preview(
        confirmed: &[Vector3<f32>],
        preview: Vector3<f32>,
        subdivisions: usize,
    ) -> Vec<Vector3<f32>> {
        if confirmed.is_empty() {
            return vec![preview];
        }
        let stable = Self::sample_curve(confirmed, subdivisions);
        let mut with_preview = confirmed.to_vec();
        if with_preview
            .last()
            .map(|last| (*last - preview).magnitude2() > 0.0000001)
            .unwrap_or(true)
        {
            with_preview.push(preview);
        }
        let mut full = Self::sample_curve(&with_preview, subdivisions);
        let shared = stable.len().min(full.len());
        for index in 0..shared {
            full[index] = stable[index] * 0.9 + full[index] * 0.1;
        }
        full
    }
    pub fn draw_curve(&mut self, subdivisions: usize) {
        self.holding_left = false;
        self.polyline_active = false;
        self.curve_active = false;
        self.preview_point = None;
        if self.active_polyline.len() < 2 {
            self.active_polyline.clear();
            self.rebuild_vertices();
            return;
        }
        let control_points = mem::take(&mut self.active_polyline);
        let sampled_points = Self::sample_curve(&control_points, subdivisions);
        if sampled_points.len() >= 2 {
            self.add_polyline(sampled_points, self.polyline_color, POLYLINE_WIDTH_PIXELS);
            if let Some(curve) = self.entities.last_mut() {
                curve.show_markers = false;
            }
        }
        self.rebuild_vertices();
    }
    pub fn add_line(&mut self, point1: Vector3<f32>, point2: Vector3<f32>, step_size: f32) {
        let dir = point2 - point1;
        let distance = dir.magnitude();
        if distance == 0.0 {
            self.add_point(point1);
            return;
        }
        let steps = (distance / step_size).ceil() as usize;
        for i in 0..=steps {
            let t = i as f32 / steps as f32;
            let point = point1 + dir * t;
            self.add_point(point);
        }
    }
    pub fn add_point(&mut self, point: Vector3<f32>) {
        let size = 0.005;
        let color = [0.4, 0.4, 0.4, 1.0];
        let forward = (self.camera.target - self.camera.eye).normalize();
        let right = forward.cross(self.camera.up).normalize();
        let up = right.cross(forward).normalize();
        let p0 = point - right * size - up * size;
        let p1 = point + right * size - up * size;
        let p2 = point - right * size + up * size;
        let p3 = point + right * size + up * size;
        self.point_vertices.extend_from_slice(&[
            Vertex {
                position: p0.into(),
                coords: [0.0, 0.0, 0.0],
                color,
            },
            Vertex {
                position: p1.into(),
                coords: [0.0, 0.0, 0.0],
                color,
            },
            Vertex {
                position: p2.into(),
                coords: [0.0, 0.0, 0.0],
                color,
            },
            Vertex {
                position: p2.into(),
                coords: [0.0, 0.0, 0.0],
                color,
            },
            Vertex {
                position: p1.into(),
                coords: [0.0, 0.0, 0.0],
                color,
            },
            Vertex {
                position: p3.into(),
                coords: [0.0, 0.0, 0.0],
                color,
            },
        ]);
        self.redraw = true;
    }

    pub fn mouse_move(&mut self, x: f64, y: f64) -> Option<String> {
        self.cursor_pos = PhysicalPosition::new(x, y);
        if self.grab_mode {
            let current = self.fetch_point(self.cursor_pos)?;
            let origin = self.grab_origin?;
            let offset = self.grid_snap_offset(current - origin);
            if let Some(selected_id) = self.selected_entity {
                if let Some(entity) = self
                    .entities
                    .iter_mut()
                    .find(|entity| entity.id == selected_id)
                {
                    entity.vertices = self
                        .grab_snapshot
                        .iter()
                        .map(|point| *point + offset)
                        .collect();
                }
            }
            self.rebuild_vertices();
            return None;
        }
        if !self.polyline_active {
            return None;
        }

        let point = self.fetch_point(self.cursor_pos)?;
        let (position, _, snap_kind) = self.get_snap_pos(point);
        if !self.active_polyline.is_empty() {
            let position_changed = self
                .preview_point
                .map(|previous| (previous - position).magnitude2() > 0.00000001)
                .unwrap_or(true);

            if position_changed {
                self.preview_point = Some(position);
                self.rebuild_vertices();
            }
        }
        snap_kind.map(str::to_owned)
    }

    pub fn mouse_button(&mut self, pressed: bool) -> bool {
        self.holding_left = pressed;
        if pressed {
            let screen_pos = Vector2::new(self.cursor_pos.x as f32, self.cursor_pos.y as f32);
            let screen_size = Vector2::new(self.width as f32, self.height as f32);
            if let Some(ray) = self.camera.screen_to_ray(screen_pos, screen_size) {
                if let Some(gumball) = &mut self.gumball {
                    if let Some(handle) = gumball.hit_test(&ray) {
                        gumball.active = Some(handle);
                        gumball.drag_start_origin = gumball.origin;
                        return true;
                    }
                }
            }
        } else {
            if let Some(gumball) = &mut self.gumball {
                if gumball.active.is_some() {
                    gumball.active = None;
                    gumball.drag_start_point = None;
                    self.save_undo_state();
                    return true;
                }
            }
        }
        if pressed && self.polyline_active {
            return self.polyline(self.cursor_pos);
        }
        false
    }

    pub fn handle_key(&mut self, _event_loop: &ActiveEventLoop, code: KeyCode, is_pressed: bool) {
        if self.graph_handle_key(code, is_pressed) {
            return;
        }
        if is_pressed && self.move_shape_key(code, 0.1) {
            return;
        }
        self.camera_controller
            .handle_key(&mut self.camera, code, is_pressed);
    }
    pub fn linear_array(&mut self, count: u32, offset: Vector3<f32>) -> usize {
        if count == 0 {
            return 0;
        }
        let Some(id) = self.selected_entity else {
            return 0;
        };
        let Some(source) = self.entities.iter().find(|e| e.id == id).cloned() else {
            return 0;
        };
        self.save_undo_state();
        for copy_number in 1..=count {
            let copy_offset = offset * copy_number as f32;
            let id = self.next_entity;
            self.next_entity += 1;
            let vertices = source.vertices.iter().map(|v| *v + copy_offset).collect();
            self.entities.push(PolyLine {
                id,
                vertices,
                color: source.color,
                thickness: source.thickness,
                selected: false,
                height: source.height,
                    extrude_axis: source.extrude_axis,
                show_markers: source.show_markers,
                hatched: source.hatched,
            });
        }
        self.rebuild_vertices();
        count as usize
    }
    pub fn polar_array(&mut self, item_count: u32, center: Vector3<f32>) -> usize {
        if item_count < 2 {
            return 0;
        }
        let sources: Vec<PolyLine> = self
            .entities
            .iter()
            .filter(|entity| entity.selected)
            .cloned()
            .collect();
        if sources.is_empty() {
            return 0;
        } 
        self.save_undo_state();
        let angle_step = std::f32::consts::TAU / item_count as f32;
        let mut created = 0;
        for copy_number in 1..item_count {
            let angle = angle_step * copy_number as f32;
            let cos_angle = angle.cos();
            let sin_angle = angle.sin();
            for source in &sources {
                let vertices = source
                    .vertices
                    .iter()
                    .map(|point| {
                        let local = *point - center;
                        Vector3::new(
                            local.x * cos_angle - local.z * sin_angle,
                            local.y,
                            local.x * sin_angle + local.z * cos_angle,
                        ) + center
                    })
                    .collect();
                let id = self.next_entity;
                self.next_entity += 1;
                self.entities.push(PolyLine {
                    id,
                    vertices,
                    color: source.color,
                    thickness: source.thickness,
                    selected: true,
                    height: source.height,
                    extrude_axis: source.extrude_axis,
                    show_markers: source.show_markers,
                    hatched: source.hatched,
                });
                created += 1;
            }
        }
        self.selected_entity = self
            .entities
            .iter()
            .find(|entity| entity.selected)
            .map(|entity| entity.id);
        self.rebuild_vertices();
        created
    }
    pub fn cancel_active_command(&mut self, code: KeyCode, is_pressed: bool) -> bool {
        let circle_active = self.circle_center.is_some();
        let copy_active = !self.copy_sources.is_empty();
        let move_active = self.move_anchor.is_some();
        let grab_active = self.grab_mode;
        let curve_active = self.curve_active;
        let polyline_active = self.polyline_active && !self.curve_active;
        if code != KeyCode::Escape || !is_pressed {
            return false;
        }
        if circle_active {
            self.cancel_circle();
            return true;
        } else if copy_active {
            self.cancel_copy();
            return true;
        } else if move_active {
            for (id, vertices) in &self.move_snapshots {
                if let Some(entity) = self.entities.iter_mut().find(|entity| entity.id == *id) {
                    entity.vertices = vertices.clone();
                }
            }
            for (id, original_surface) in &self.move_surface_snapshots {
                if let Some(surface_entity) = self
                    .nurbs_surfaces
                    .iter_mut()
                    .find(|surface| surface.id == *id)
                {
                    surface_entity.surface = original_surface.clone();
                }
            }
            for (id, original_origin) in &self.move_solid_snapshots {
                if let Some(solid) = self.solids.iter_mut().find(|s| s.id == *id) {
                    solid.origin = *original_origin;
                }
            }
            self.move_anchor = None;
            self.move_snapshots.clear();
            self.move_solid_snapshots.clear();
            self.move_surface_snapshots.clear();
            self.rebuild_vertices();
            return true;
        } else if grab_active {
            if let Some(id) = self.selected_entity {
                if let Some(entity) = self.entities.iter_mut().find(|e| e.id == id) {
                    entity.vertices.clone_from(&self.grab_snapshot);
                }
            }
            self.grab_mode = false;
            self.grab_origin = None;
            self.grab_snapshot.clear();
            self.holding_left = false;
            self.rebuild_vertices();
            return true;
        } else if curve_active {
            self.cancel_curve();
            return true;
        } else if polyline_active {
            self.cancel_polyline();
            return true;
        } else {
            return false;
        }
    }
    pub fn solid_hit_test(&self, solid: &SolidEntity, mouse: Vector2<f32>) -> Option<f32> {
        let ray = self.camera.screen_to_ray(
            mouse,
            Vector2::new(self.width as f32, self.height as f32)
        )?;
        let tris = solid.to_tris();
        let bvh_tris: Vec<BvhTriangle> = tris
            .into_iter()
            .map(|tri| {
                BvhTriangle::new(
                    Point3::new(tri.v0[0], tri.v0[1], tri.v0[2]),
                    Point3::new(tri.v1[0], tri.v1[1], tri.v1[2]),
                    Point3::new(tri.v2[0], tri.v2[1], tri.v2[2]),
                )
            }).collect();
        let bvh = Bvh::build(bvh_tris);
        bvh.ray_intersect(&ray)
    }
    pub fn extract_triangles(&self) -> Vec<Triangle> {
        let mut triangles =Vec::new();
        let push_vert_tri = |verts: &[Vertex], tris: &mut Vec<Triangle>| {
            for chunk in verts.chunks_exact(3) {
                let p0 = chunk[0].position;
                let p1 = chunk[1].position;
                let p2 = chunk[2].position;
                let v0 = Vector3::new(p0[0], p0[1], p0[2]);
                let v1 = Vector3::new(p1[0], p1[1], p1[2]);
                let v2 = Vector3::new(p2[0], p2[1], p2[2]);
                let norm_vec =(v1 - v0).cross(v2 - v0);
                let normal = if norm_vec.magnitude2() > f32::EPSILON {
                    let n = norm_vec.normalize();
                    [n.x, n.y, n.z]
                } else {
                    chunk[0].coords
                };
                tris.push(Triangle {
                    v0: p0,
                    v1: p1,
                    v2: p2,
                    normal,
                });
            }
        };
        for entity in &self.entities {
            let closed =Self::is_closed_shape(&entity.vertices);
            if closed && entity.height.abs() > f32::EPSILON {
                let wall_verts = Self::extrude(
                    &entity.vertices,
                    [1.0, 1.0, 1.0, 1.0],
                    entity.height,
                    entity.extrude_axis,
                );
                push_vert_tri(&wall_verts, &mut triangles);
                let bottom_cap = Self::tessellate_fill(&entity.vertices, [1.0, 1.0, 1.0, 1.0]);
                push_vert_tri(&bottom_cap, &mut triangles);
                let offset = entity.extrude_axis.normalize() * entity.height;
                let top_verts: Vec<Vector3<f32>> = entity.vertices.iter().map(|p| *p + offset).collect();
                let mut top_cap = Self::tessellate_fill(&top_verts, [1.0, 1.0, 1.0, 1.0]);
                for chunk in top_cap.chunks_exact_mut(3) {
                    chunk.swap(1, 2);
                }
                push_vert_tri(&top_cap, &mut triangles);
            }
        }
        for entity in &self.nurbs_surfaces {
            let mesh_verts = entity.surface.sample_mesh(
                entity.u_subdivisions.max(16),
                entity.v_subdivisions.max(16),
                [1.0, 1.0, 1.0, 1.0],
            );
            push_vert_tri(&mesh_verts, &mut triangles);
        }
        for mesh in &self.imported_meshes {
            for triangle in mesh.triangles.iter() {
                let v0 = [triangle[0].x, triangle[0].y, triangle[0].z];
                let v1 = [triangle[1].x, triangle[1].y, triangle[1].z];
                let v2 = [triangle[2].x, triangle[2].y, triangle[2].z];
                let cross = (triangle[1] - triangle[0]).cross(triangle[2] - triangle[0]);
                let normal = if cross.magnitude2() > f32::EPSILON {
                    let n = cross.normalize();
                    [n.x, n.y, n.z]
                } else {
                    [0.0, 1.0, 0.0]
                };
                triangles.push(Triangle { v0, v1, v2, normal });
            }
        }
        for text in &self.text_entities {
            if text.depth > 0.0 {
                push_vert_tri(&text.cached_vertices, &mut triangles);
            }
        }
        for solid in &self.solids {
            triangles.extend(solid.to_tris());
        }
        triangles
    }
    pub fn add_solid(&mut self, origin: [f32; 3], shape: BaseShape, color: [f32; 4]) -> usize {
        let id = self.next_entity;
        self.next_entity += 1;
        self.solids.push(SolidEntity {
            id,
            origin,
            shape,
            color,
            selected: true,
        });
        self.selected_entity = Some(id);
        self.rebuild_vertices();
        id
    }
    pub fn add_box(&mut self, origin: [f32; 3], width: f32, height: f32, depth: f32) -> usize {
        self.add_solid(origin, BaseShape::Box {
            width,
            height,
            depth,
        }, [0.2, 0.6, 0.9, 1.0])
    }
    pub fn add_cylinder(&mut self, origin: [f32; 3], radius: f32, height: f32, subdivisions: usize) -> usize {
        self.add_solid(origin, BaseShape::Cylinder {
            radius,
            height,
            subdivisions,
        }, [0.3, 0.8, 0.4, 1.0])
    }
    pub fn add_sphere(&mut self, origin: [f32; 3], radius: f32, rings: usize, sectors: usize) ->  usize {
        self.add_solid(origin, BaseShape::Sphere {
            radius,
            rings,
            sectors,
        }, [0.9, 0.4, 0.3, 1.0])
    }
    pub fn add_cone(&mut self, origin: [f32; 3], radius: f32, height: f32, subdivisions: usize) -> usize {
        self.add_solid(origin, BaseShape::Cone {
            radius,
            height,
            subdivisions,
        }, [0.9, 0.7, 0.2, 1.0])
    }
    pub fn add_torus(&mut self, origin: [f32; 3], outer_radius: f32, inner_radius: f32, outer_subdivisions: usize, inner_subdivisions: usize) -> usize {
        self.add_solid(origin, BaseShape::Torus {
            outer_radius,
            inner_radius,
            outer_subdivisions,
            inner_subdivisions,
        }, [0.7, 0.3, 0.8, 1.0])
    }
    pub fn import_solid_mesh(&mut self, tris: Vec<crate::exporter::Triangle>, color: [f32; 4]) -> usize {
        let id = self.next_entity;
        self.next_entity += 1;
        self.save_undo_state();
        let solid = SolidEntity {
            id,
            origin: [0.0, 0.0, 0.0],
            shape: BaseShape::CustomMesh { triangles: tris },
            color,
            selected: true,
        };
        for s in &mut self.solids {
            s.selected = false;
        }
        for e in &mut self.entities {
            e.selected = false;
        }
        self.solids.push(solid);
        self.selected_entity = Some(id);
        self.rebuild_vertices();
        id
    }
    pub fn bool_union(&mut self, id_a: usize, id_b: usize) -> Option<usize> {
        let s_a = self.solids.iter().find(|s| s.id == id_a)?.clone();
        let s_b = self.solids.iter().find(|s| s.id == id_b)?.clone();
        self.save_undo_state();
        let csg_a = s_a.to_csg();
        let csg_b = s_b.to_csg();
        let result_csg = csg_a.union(&csg_b);
        self.solids.retain(|s| s.id != id_a && s.id != id_b);
        let id = self.next_entity;
        self.next_entity += 1;
        let mut new_solid = SolidEntity::from_csg(id, &result_csg, s_a.color);
        new_solid.selected = true;
        self.solids.push(new_solid);
        self.selected_entity = Some(id);
        self.rebuild_vertices();
        Some(id)
    }
    pub fn bool_difference(&mut self, id_a: usize, id_b: usize) -> Option<usize> {
        let s_a = self.solids.iter().find(|s| s.id == id_a)?.clone();
        let s_b = self.solids.iter().find(|s| s.id == id_b)?.clone();
        self.save_undo_state();
        let csg_a = s_a.to_csg();
        let csg_b = s_b.to_csg();
        let result_csg = csg_a.difference(&csg_b);
        self.solids.retain(|s| s.id != id_a && s.id != id_b);
        let id = self.next_entity;
        self.next_entity += 1;
        let mut new_solid = SolidEntity::from_csg(id, &result_csg, s_a.color);
        new_solid.selected = true;
        self.solids.push(new_solid);
        self.selected_entity = Some(id);
        self.rebuild_vertices();
        Some(id)
    }
    pub fn bool_intersection(&mut self, id_a: usize, id_b: usize) -> Option<usize> {
        let s_a = self.solids.iter().find(|s| s.id == id_a)?.clone();
        let s_b = self.solids.iter().find(|s| s.id == id_b)?.clone();
        self.save_undo_state();
        let csg_a = s_a.to_csg();
        let csg_b = s_b.to_csg();
        let result_csg = csg_a.intersect(&csg_b);
        self.solids.retain(|s| s.id != id_a && s.id != id_b);
        let id = self.next_entity;
        self.next_entity += 1;
        let mut new_solid = SolidEntity::from_csg(id, &result_csg, s_a.color);
        new_solid.selected = true;
        self.solids.push(new_solid);
        self.selected_entity = Some(id);
        self.rebuild_vertices();
        Some(id)
    }
    pub fn bool_union_selected(&mut self) -> Option<usize> {
        let selected: Vec<usize> = self.solids.iter().filter(|s| s.selected).map(|s| s.id).collect();
        if selected.len() >= 2 {
            self.bool_union(selected[0], selected[1])
        } else {
            None
        }
    }
    pub fn bool_difference_selected(&mut self) -> Option<usize> {
        let selected: Vec<usize> = self.solids.iter().filter(|s| s.selected).map(|s| s.id).collect();
        if selected.len() >= 2 {
            self.bool_difference(selected[0], selected[1])
        } else {
            None
        }
    }
    pub fn bool_intersection_selected(&mut self) -> Option<usize> {
        let selected: Vec<usize> = self.solids.iter().filter(|s| s.selected).map(|s| s.id).collect();
        if selected.len() >= 2 {
            self.bool_intersection(selected[0], selected[1])
        } else {
            None
        }
    }
    pub fn revolve_selected_curve_to_solid(
        &mut self,
        degrees: f32,
        axis: &str,
    ) -> Option<usize> {
        let sel_id = self.selected_entity?;
        let curve_entity = self.entities.iter().find(|e| e.id == sel_id)?.clone();
        if curve_entity.vertices.len() < 2 {
            return None;
        }
        let (axis_origin, axis_dir) = match axis {
            "x" => (Vector3::new(0.0, 0.0, 0.0), Vector3::unit_x()),
            "y" => (Vector3::new(0.0, 0.0, 0.0), Vector3::unit_y()),
            _ => (Vector3::new(0.0, 0.0, 0.0), Vector3::unit_z()),
        };
        let id = self.next_entity;
        self.next_entity += 1;
        let solid = SolidEntity::from_revolve(
            id,
            &curve_entity.vertices,
            axis_origin,
            axis_dir,
            degrees,
            32,
            [0.3, 0.7, 0.5, 1.0],
        )?;
        self.save_undo_state();
        self.solids.push(solid);
        self.selected_entity = Some(id);
        self.rebuild_vertices();
        Some(id)
    }
    pub fn loft_selected_curves(&mut self, closed_caps: bool) -> Option<usize> {
        let mut selected_curves: Vec<Vec<Vector3<f32>>> = self
            .entities
            .iter()
            .filter(|e| e.selected && e.vertices.len() >= 2)
            .map(|e| e.vertices.clone())
            .collect();
        if selected_curves.len() < 2 {
            return None;
        }
        selected_curves.sort_by(|a, b| {
            let avg_a: f32 = a.iter().map(|p| p.z).sum::<f32>() / a.len() as f32;
            let avg_b: f32 = b.iter().map(|p| p.z).sum::<f32>() / b.len() as f32;
            avg_a.partial_cmp(&avg_b).unwrap_or(std::cmp::Ordering::Equal)
        });
        let id = self.next_entity;
        self.next_entity += 1;
        let solid = SolidEntity::from_loft(
            id,
            &selected_curves,
            closed_caps,
            [0.2, 0.7, 0.85, 1.0],
        )?;
        self.save_undo_state();
        self.solids.push(solid);
        self.selected_entity = Some(id);
        self.rebuild_vertices();
        Some(id)
    }
    pub fn extrude_selected_curve_to_solid(&mut self, height: f32) -> Option<usize> {
        let sel_id = self.selected_entity?;
        let curve_entity = self.entities.iter().find(|e| e.id == sel_id)?.clone();
        if curve_entity.vertices.len() < 3 {
            return None;
        }
        let view_direction = self.camera.eye - self.camera.target;
        let fallback_axis = if self.right_view {
            Vector3::unit_x()
        } else if self.front_view {
            Vector3::unit_z()
        } else {
            Vector3::unit_y()
        };
        let axis = Self::extrusion_axis_for_vertices(&curve_entity.vertices, view_direction, fallback_axis);
        let id = self.next_entity;
        self.next_entity += 1;
        let solid = SolidEntity::from_extrusion(
            id,
            &curve_entity.vertices,
            height,
            Some(axis),
            [0.2, 0.6, 0.9, 1.0],
        )?;
        self.save_undo_state();
        self.solids.push(solid);
        self.selected_entity = Some(id);
        self.rebuild_vertices();
        Some(id)
    }
    pub fn hollow_selected_solid(&mut self, wall_thickness: f32) -> Option<usize> {
        let sel_id = self.selected_entity?;
        let solid = self.solids.iter().find(|s| s.id == sel_id)?.clone();
        self.save_undo_state();
        let new_solid = solid.hollow(sel_id, wall_thickness)?;
        if let Some(s) = self.solids.iter_mut().find(|s| s.id == sel_id) {
            *s = new_solid;
        }
        self.rebuild_vertices();
        Some(sel_id)
    }
    pub fn scale_selected_solid(&mut self, fx: f32, fy: f32, fz: f32) -> bool {
        let Some(sel_id) = self.selected_entity else {
            return false;
        };
        self.save_undo_state();
        if let Some(solid) = self.solids.iter_mut().find(|s| s.id == sel_id) {
            solid.scale([fx, fy, fz]);
            self.rebuild_vertices();
            true
        } else {
            false
        }
    }
    pub fn mirror_selected_solid(&mut self, axis: &str) -> bool {
        let Some(sel_id) = self.selected_entity else {
            return false;
        };
        self.save_undo_state();
        if let Some(solid) = self.solids.iter_mut().find(|s| s.id == sel_id) {
            solid.mirror(axis);
            self.rebuild_vertices();
            true
        } else {
            false
        }
    }
    pub fn section_selected_solid(
        &mut self,
        plane_origin: Vector3<f32>,
        plane_norm: Vector3<f32>,
    ) -> Vec<usize> {
        let Some(sel_id) = self.selected_entity else {
            return Vec::new()
        };
        let Some(solid) = self.solids.iter().find(|s| s.id == sel_id) else {
            return Vec::new()
        };
        let tris = solid.to_tris();
        let polys = SolidEntity::slice_with_plane(&tris, plane_origin, plane_norm);
        let mut created_ids = Vec::new();
        self.save_undo_state();
        for p in polys {
            if p.len() >= 2 {
                let id = self.next_entity;
                self.next_entity += 1;
                self.entities.push(crate::viewport::PolyLine {
                    id,
                    vertices: p,
                    color: [0.0, 1.0, 0.6, 1.0],
                    selected: false,
                    thickness: 2.0,
                    height: 0.0,
                    hatched: false,
                    extrude_axis: Vector3::unit_z(),
                    show_markers: false,
                });
                created_ids.push(id);
            }
        }
        self.rebuild_vertices();
        created_ids
    }
    pub fn contour_selected_solid(
        &mut self,
        start: Vector3<f32>,
        dir: Vector3<f32>,
        interval: f32,
        count: usize,
    ) -> Vec<usize> {
        let dir = if dir.magnitude2() > 1e-6 {
            dir.normalize()
        } else {
            Vector3::unit_z()
        };
        let mut all_created = Vec::new();
        for k in 0..count {
            let origin = start + dir * (k as f32 * interval);
            let ids = self.section_selected_solid(origin, dir);
            all_created.extend(ids);
        }
        all_created
    }
    pub fn array_linear_selected(&mut self, count: usize, delta: Vector3<f32>) -> Vec<usize> {
        let Some(sel_id) = self.selected_entity else {
            return Vec::new();
        };
        if count < 2 {
            return Vec::new();
        }
        self.save_undo_state();
        let mut created_ids = Vec::new();
        if let Some(s) = self.solids.iter().find(|s| s.id == sel_id).cloned() {
            for i in 1..count {
                let id = self.next_entity;
                self.next_entity += 1;
                let offset = delta * (i as f32);
                let mut n_s = s.clone();
                n_s.id = id;
                n_s.origin[0] += offset.x;
                n_s.origin[1] += offset.y;
                n_s.origin[2] += offset.z;
                n_s.selected = false;
                self.solids.push(n_s);
                created_ids.push(id);
            }
        }
        self.rebuild_vertices();
        created_ids
    }
    pub fn array_polar_selected(
        &mut self,
        count: usize,
        center: Vector3<f32>,
        axis: Vector3<f32>,
        total_angle_rad: f32,
    ) -> Vec<usize> {
        let Some(sel_id) = self.selected_entity else {
            return Vec::new();
        };
        if count < 2 {
            return Vec::new();
        };
        let axis = if axis.magnitude2() > 1e-6 {
            axis.normalize()
        } else {
            Vector3::unit_z()
        };
        self.save_undo_state();
        let mut created_ids = Vec::new();
        if let Some(s) = self.solids.iter().find(|s| s.id == sel_id).cloned() {
            let tris = s.to_tris();
            for i in 1..count {
                let angle = total_angle_rad * (i as f32 / count as f32);
                let cos_a = angle.cos();
                let sin_a = angle.sin();
                let rot_pt = |p: [f32; 3]| -> [f32; 3] {
                    let v = Vector3::new(p[0], p[1], p[2]);
                    let rot = v * cos_a + axis.cross(v) * sin_a + axis * axis.dot(v) * (1.0 - cos_a);
                    let final_p = center + rot;
                    [final_p.x, final_p.y, final_p.z]
                };
                let rot_dir = |d: [f32; 3]| -> [f32; 3] {
                    let v = Vector3::new(d[0], d[1], d[2]);
                    let rot = v * cos_a + axis.cross(v) * sin_a + axis * axis.dot(v) * (1.0 -  cos_a);
                    [rot.x, rot.y, rot.z]
                };
                let rot_tris: Vec<crate::exporter::Triangle> = tris
                    .iter()
                    .map(|t| {
                        let v0 = rot_pt(t.v0);
                        let v1 = rot_pt(t.v1);
                        let v2 = rot_pt(t.v2);
                        let norm = rot_dir(t.normal);
                        crate::exporter::Triangle {
                            v0,
                            v1,
                            v2,
                            normal: norm,
                        }
                    }).collect();
                let id = self.next_entity;
                self.next_entity += 1;
                self.solids.push(SolidEntity {
                    id,
                    origin: [0.0, 0.0, 0.0],
                    shape: BaseShape::CustomMesh { triangles: rot_tris },
                    color: s.color,
                    selected: false,
                });
                created_ids.push(id);
            }
        }
        self.rebuild_vertices();
        created_ids
    }
    pub fn array_grid_selected(
        &mut self,
        c_x: usize,
        c_y: usize,
        c_z: usize,
        spacing: Vector3<f32>,
    ) -> Vec<usize> {
        let Some(sel_id) = self.selected_entity else {
            return Vec::new();
        };
        self.save_undo_state();
        let mut created_ids = Vec::new();
        if let Some(solid) = self.solids.iter().find(|s| s.id == sel_id).cloned() {
            for ix in 0..c_x {
                for iy in 0..c_y {
                    for iz in 0..c_z {
                        if ix == 0 && iy == 0 && iz == 0 {
                            continue;
                        }
                        let id = self.next_entity;
                        self.next_entity += 1;
                        let mut n_solid = solid.clone();
                        n_solid.id = id;
                        n_solid.origin[0] += ix as f32 * spacing.x;
                        n_solid.origin[1] += iy as f32 * spacing.y;
                        n_solid.origin[2] += iz as f32 * spacing.z;
                        n_solid.selected = false;
                        self.solids.push(n_solid);
                        created_ids.push(id);
                    }
                }
            }
        }
        self.rebuild_vertices();
        created_ids
    }
    pub fn sweep1_selected_curves(&mut self, closed_caps: bool) -> Option<usize> {
        let selected: Vec<Vec<Vector3<f32>>> = self
            .entities
            .iter()
            .filter(|e| e.selected && e.vertices.len() >= 2)
            .map(|e| e.vertices.clone())
            .collect();
        if selected.len() < 2 {
            return None;
        }
        let rail = &selected[0];
        let profile = &selected[1];
        let id = self.next_entity;
        self.next_entity += 1;
        let solid = SolidEntity::from_sweep1(
            id,
            rail,
            profile,
            closed_caps,
            [0.3, 0.6, 0.95, 1.0],
        )?;
        self.save_undo_state();
        self.solids.push(solid);
        self.selected_entity = Some(id);
        self.rebuild_vertices();
        Some(id)
    }
    pub fn seg_intersect_2d(
        p1: Vector3<f32>,
        p2: Vector3<f32>,
        q1: Vector3<f32>,
        q2: Vector3<f32>,
    ) -> Option<(f32, Vector3<f32>)> {
        let r = p2 - p1;
        let s = q2 - q1;
        let det = r.x * s.y - r.y * s.x;
        if det.abs() < 1e-6 {
            return None;
        }
        let qp = q1 - p1;
        let t = (qp.x * s.y - qp.y * s.x) / det;
        let u = (qp.x * r.y - qp.y * r.x) / det;
        if t > 1e-4 && t < (1.0 - 1e-4) && u > 1e-4 && u < (1.0 - 1e-4) {
            let hit = p1 + r * t;
            Some((t, hit))
        } else {
            None
        }
    }
    pub fn split_select_curve(&mut self, cutter_id: usize) -> Vec<usize> {
        let Some(target_id) = self.selected_entity else {
            return Vec::new();
        };
        let Some(target_curve) = self.entities.iter().find(|e| e.id == target_id).cloned() else {
            return Vec::new();
        };
        let Some(cutter_curve) = self.entities.iter().find(|e| e.id == cutter_id).cloned() else {
            return Vec::new();
        };
        let mut sub_curves: Vec<Vec<Vector3<f32>>> = Vec::new();
        let mut current_sub = Vec::new();
        let n = target_curve.vertices.len();
        if n < 2 {
            return Vec::new();
        }
        current_sub.push(target_curve.vertices[0]);
        for i in 0..n - 1 {
            let p1 = target_curve.vertices[i];
            let p2 = target_curve.vertices[i + 1];
            let mut split_point: Option<Vector3<f32>> = None;
            for j in 0..cutter_curve.vertices.len().saturating_sub(1) {
                let q1 = cutter_curve.vertices[j];
                let q2 = cutter_curve.vertices[j + 1];
                if let Some((_, hit)) = Self::seg_intersect_2d(p1, p2, q1, q2) {
                    split_point = Some(hit);
                    break;
                }
            }
            if let Some(hit) = split_point {
                current_sub.push(hit);
                sub_curves.push(current_sub);
                current_sub = vec![hit, p2];
            } else {
                current_sub.push(p2);
            }
        }
        sub_curves.push(current_sub);
        if sub_curves.len() <= 1 {
            return Vec::new();
        }
        self.save_undo_state();
        self.entities.retain(|e| e.id != target_id);
        let mut new_ids = Vec::new();
        for verts in sub_curves {
            if verts.len() >= 2 {
                let id = self.next_entity;
                self.next_entity += 1;
                self.entities.push(crate::viewport::PolyLine {
                    id,
                    vertices: verts,
                    color: target_curve.color,
                    thickness: target_curve.thickness,
                    selected: false,
                    height: target_curve.height,
                    hatched: false,
                    extrude_axis: target_curve.extrude_axis,
                    show_markers: false,
                });
            }
        }
        self.rebuild_vertices();
        new_ids
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shape(id: usize, vertices: Vec<Vector3<f32>>) -> PolyLine {
        PolyLine {
            id,
            vertices,
            color: [1.0; 4],
            thickness: 1.0,
            selected: false,
            height: 0.0,
            extrude_axis: Vector3::unit_y(),
            show_markers: true,
            hatched: true,
        }
    }

    #[test]
    fn grab_key_moves_selected_shape_until_released() {
        let mut viewport = Viewport::new(800, 600);
        let original = vec![Vector3::new(-0.5, 0.0, 0.0), Vector3::new(0.5, 0.0, 0.0)];
        viewport.entities.push(shape(7, original.clone()));
        viewport.selected_entity = Some(7);
        viewport.cursor_pos = PhysicalPosition::new(400.0, 300.0);

        assert!(viewport.graph_handle_key(KeyCode::KeyG, true));
        assert!(viewport.grab_mode);
        viewport.mouse_move(450.0, 300.0);

        let moved = &viewport.entities[0].vertices;
        let offset = moved[0] - original[0];
        assert!(offset.magnitude() > 0.0);
        assert!(((moved[1] - original[1]) - offset).magnitude() < 0.00001);

        assert!(viewport.graph_handle_key(KeyCode::KeyG, false));
        let stopped = viewport.entities[0].vertices.clone();
        viewport.mouse_move(500.0, 300.0);
        assert_eq!(viewport.entities[0].vertices, stopped);
    }

    #[test]
    fn extrude_selected_does_not_change_unselected_shapes() {
        let mut viewport = Viewport::new(800, 600);
        viewport.entities = vec![
            shape(
                1,
                vec![Vector3::new(0.0, 0.0, 0.0), Vector3::new(1.0, 0.0, 0.0)],
            ),
            shape(
                2,
                vec![Vector3::new(2.0, 0.0, 0.0), Vector3::new(3.0, 0.0, 0.0)],
            ),
        ];
        viewport.selected_entity = Some(1);

        viewport.extrude_selected(2.0);

        assert_eq!(viewport.entities[0].height, 2.0);
        assert_eq!(viewport.entities[1].height, 0.0);
    }
    #[test]
    fn mouse_move_previews_end_snap_without_clicking() {
        let mut viewport = Viewport::new(800, 600);
        viewport.entities.push(shape(
            1,
            vec![Vector3::new(0.0, 0.0, 0.0), Vector3::new(0.5, 0.0, 0.0)],
        ));
        viewport.active_polyline.push(Vector3::new(-0.5, 0.0, 0.0));
        viewport.polyline_active = true;
        viewport.osnap = true;
        viewport.end_snap_enabled = true;

        let snap_kind = viewport.mouse_move(400.0, 300.0);

        assert_eq!(snap_kind.as_deref(), Some("End"));
        assert!(viewport.preview_point.unwrap().magnitude() < 0.00001);
        assert_eq!(viewport.active_polyline.len(), 1);
    }

    #[test]
    fn grid_snap_rounds_to_rendered_subgrid_intersections() {
        let mut viewport = Viewport::new(800, 600);
        viewport.set_grid_snap(true);

        let (snapped, closes_shape, snap_kind) =
            viewport.get_snap_pos(Vector3::new(0.14, 0.0, -0.26));

        assert!((snapped.x - 0.1).abs() < 0.00001);
        assert!((snapped.z + 0.3).abs() < 0.00001);
        assert!(!closes_shape);
        assert_eq!(snap_kind, Some("Grid"));

        viewport.set_grid_spacing(0.5);
        let (coarse, _, _) = viewport.get_snap_pos(Vector3::new(0.31, 0.0, -0.74));
        assert!((coarse.x - 0.5).abs() < 0.00001);
        assert!((coarse.z + 0.5).abs() < 0.00001);
    }

    #[test]
    fn near_snap_uses_active_polyline_before_grid_snap() {
        let mut viewport = Viewport::new(800, 600);
        viewport.active_polyline = vec![Vector3::new(-0.5, 0.0, 0.0), Vector3::new(0.5, 0.0, 0.0)];
        viewport.set_osnap_modes(false, true, false, false);
        viewport.set_grid_snap(true);

        let (snapped, closes_shape, snap_kind) =
            viewport.get_snap_pos(Vector3::new(0.23, 0.0, 0.01));

        assert!(snapped.z.abs() < 0.00001);
        assert!(!closes_shape);
        assert_eq!(snap_kind, Some("Near"));
    }

    #[test]
    fn end_snap_to_another_figure_keeps_active_polyline_open() {
        let mut viewport = Viewport::new(800, 600);
        viewport.entities.push(shape(
            1,
            vec![Vector3::new(0.0, 0.0, 0.0), Vector3::new(0.5, 0.0, 0.0)],
        ));
        viewport.active_polyline = vec![Vector3::new(-0.5, 0.0, -0.5)];
        viewport.set_osnap_modes(true, false, false, false);

        let (snapped, finishes_polyline, snap_kind) =
            viewport.get_snap_pos(Vector3::new(0.01, 0.0, 0.01));

        assert!(snapped.magnitude() < 0.00001);
        assert!(!finishes_polyline);
        assert_eq!(snap_kind, Some("End"));
    }

    #[test]
    fn mid_snap_finds_segment_midpoint() {
        let mut viewport = Viewport::new(800, 600);
        viewport.entities.push(shape(
            1,
            vec![Vector3::new(-0.5, 0.0, 0.0), Vector3::new(0.5, 0.0, 0.0)],
        ));
        viewport.set_osnap_modes(false, false, true, false);

        let (snapped, closes_shape, snap_kind) = viewport.get_snap_pos(Vector3::new(0.0, 0.0, 0.0));

        assert!(snapped.magnitude() < 0.00001);
        assert!(!closes_shape);
        assert_eq!(snap_kind, Some("Mid"));
    }

    #[test]
    fn int_snap_finds_crossing_segments() {
        let mut viewport = Viewport::new(800, 600);
        viewport.entities = vec![
            shape(
                1,
                vec![Vector3::new(-0.5, 0.0, -0.5), Vector3::new(0.5, 0.0, 0.5)],
            ),
            shape(
                2,
                vec![Vector3::new(-0.5, 0.0, 0.5), Vector3::new(0.5, 0.0, -0.5)],
            ),
        ];
        viewport.set_osnap_modes(false, false, false, true);

        let (snapped, closes_shape, snap_kind) = viewport.get_snap_pos(Vector3::new(0.0, 0.0, 0.0));

        assert!(snapped.magnitude() < 0.00001);
        assert!(!closes_shape);
        assert_eq!(snap_kind, Some("Int"));
    }

    #[test]
    fn construction_snap_does_not_change_user_osnap_modes() {
        let mut viewport = Viewport::new(800, 600);
        viewport.entities.push(shape(
            1,
            vec![Vector3::new(0.0, 0.0, 0.0), Vector3::new(0.5, 0.0, 0.0)],
        ));
        viewport.set_osnap_modes(false, false, false, false);
        viewport.set_construction_snap(true);

        let (_, _, snap_kind) = viewport.get_snap_pos(Vector3::new(0.0, 0.0, 0.0));

        assert_eq!(snap_kind, Some("End"));
        assert!(!viewport.end_snap_enabled);
        assert!(!viewport.mid_snap_enabled);
        assert!(!viewport.int_snap_enabled);
    }
}
