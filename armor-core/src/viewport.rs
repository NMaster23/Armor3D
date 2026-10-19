use crate::Vertex;
use crate::camera::{Camera, CameraController};
use crate::nurbs::{NurbsControlPoint, NurbsCurve, NurbsSurface};
use anyhow::Ok;
use cgmath::{InnerSpace, Quaternion, Rotation, SquareMatrix, Vector3, Vector4, Zero};
use glyphon::fontdb::Query;
use lyon::lyon_tessellation::{
    BuffersBuilder, FillOptions, FillRule, FillTessellator, FillVertex, VertexBuffers,
};
use lyon::math::point;
use lyon::path::Path;
use std::mem;
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
pub struct TextEntity {
    pub id: usize,
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
    extrude_drag: Option<(usize, f32)>,
    pub hist_entities: ShapeUndoStore<PolyLine>,
    pub hist_surfaces: ShapeUndoStore<NurbsSurfaceEntity>,
    pub entities: Vec<PolyLine>,
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
    pub move_snapshots: Vec<(usize, Vec<Vector3<f32>>)>,
    pub point_vertices: Vec<Vertex>,
    pub next_entity: usize,
    pub camera: Camera,
    pub camera_controller: CameraController,
    pub width: u32,
    pub height: u32,
    pub redraw: bool,
    pub grab_mode: bool,
    pub grab_origin: Option<Vector3<f32>>,
    pub grab_snapshot: Vec<Vector3<f32>>,
    pub circle_center: Option<Vector3<f32>>,
    pub circle_target: Option<Vector3<f32>>,
    pub rectangle_first: Option<Vector3<f32>>,
    pub rectangle_second: Option<Vector3<f32>>,
    pub copy_sources: Vec<PolyLine>,
    pub copy_base: Option<Vector3<f32>>,
    pub copy_target: Option<Vector3<f32>>,
    pub text_entities: Vec<TextEntity>,
    pub nurbs_curves: Vec<NurbsCurveEntity>,
    pub nurbs_surfaces: Vec<NurbsSurfaceEntity>,
}

impl Viewport {
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
    fn is_closed_shape(vertices: &[Vector3<f32>]) -> bool {
        vertices.len() >= 4 && (vertices[0] - vertices[vertices.len() - 1]).magnitude2() < 0.0001
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
                selected: false,
                u_subdivisions: (profile_rows * 4).clamp(12, 128),
                v_subdivisions: angle_steps
            });
            created += 1;
        }
        if created > 0 {
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
            });
        self.next_entity = self
            .entities
            .iter()
            .map(|entity| entity.id)
            .chain(self.nurbs_surfaces.iter().map(|surface| surface.id))
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

        if !self.hist_entities.undo() {
            return false;
        }
        let _ = self.hist_surfaces.undo();
        self.entities = self.hist_entities.current.clone();
        self.nurbs_surfaces = self.hist_surfaces.current.clone();
        self.restore_history_state();
        true
    }
    pub fn redo_scene(&mut self) -> bool {
        self.hist_entities.current = self.entities.clone();
        self.hist_surfaces.current = self.nurbs_surfaces.clone();

        if !self.hist_entities.redo() {
            return false;
        }
        let _ = self.hist_surfaces.redo();
        self.entities = self.hist_entities.current.clone();
        self.nurbs_surfaces = self.hist_surfaces.current.clone();
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
            entities: Vec::new(),
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
            point_vertices: Vec::new(),
            next_entity: 0,
            camera,
            camera_controller,
            width,
            height,
            redraw: true,
            grab_mode: false,
            grab_origin: None,
            grab_snapshot: Vec::new(),
            circle_center: None,
            circle_target: None,
            rectangle_first: None,
            rectangle_second: None,
            copy_sources: Vec::new(),
            copy_base: None,
            copy_target: None,
            text_entities: Vec::new(),
            nurbs_curves: Vec::new(),
            nurbs_surfaces: Vec::new(),
        }
    }
}

impl Viewport {
    pub fn extrude_step(&mut self, step: f32) {
        let target = if let Some(selected_id) = self.selected_entity {
            self.entities.iter_mut().find(|e| e.id == selected_id)
        } else {
            self.entities.last_mut()
        };
        if let Some(entity) = target {
            entity.height += step;
            self.rebuild_vertices();
        }
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
    ) -> bool {
        let vertices = surface.surface.sample_mesh(
            surface.u_subdivisions,
            surface.v_subdivisions,
            surface.color,
        );
        vertices.chunks_exact(3).any(|triangle| {
            let screen: Option<Vec<cgmath::Vector2<f32>>> = triangle
                .iter()
                .map(|vertex| {
                    self.world_to_screen(Vector3::new(
                        vertex.position[0],
                        vertex.position[1],
                        vertex.position[2],
                    ))
                })
                .collect();
            let Some(screen) = screen else {
                return false;
            };
            Self::point_in_triangle(mouse, screen[0], screen[1], screen[2])
        })
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
        let shape = PolyLine {
            id: self.next_entity,
            vertices: points.to_vec(),
            color,
            thickness: 1.0,
            selected: false,
            height: 0.0,
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
    ) -> Vec<Vertex> {
        let mut vertices = Vec::new();
        if points.len() < 3 || !height.is_finite() || height.abs() <= f32::EPSILON {
            return vertices;
        }
        for window in points.windows(2) {
            let point0 = window[0];
            let point1 = window[1];
            let product =
                (point1 - point0).cross(Vector3::new(0.0, height.signum(), 0.0));
            let wall_normal = if product.magnitude2() > f32::EPSILON {
                let normal = product.normalize();
                [normal.x, normal.y, normal.z]
            } else {
                [0.0, 1.0, 0.0]
            };
            let bottom0 = [point0.x, point0.y, point0.z];
            let bottom1 = [point1.x, point1.y, point1.z];
            let top0 = [point0.x, point0.y + height, point0.z];
            let top1 = [point1.x, point1.y + height, point1.z];
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
        for surface in &self.nurbs_surfaces {
            if self.surface_hit_test(surface, mouse_px) {
                closest = Some(surface.id);
                min_dist_sq = 0.0;
            }
        }

        if additive {
            if let Some(id) = closest {
                if let Some(entity) = self.entities.iter_mut().find(|entity| entity.id == id) {
                    entity.selected = true;
                } else if let Some(surface) =
                    self.nurbs_surfaces.iter_mut().find(|surface| surface.id == id)
                {
                    surface.selected = true;
                }
            }
        } else {
            for entity in &mut self.entities {
                entity.selected = Some(entity.id) == closest;
            }
            for surface in &mut self.nurbs_surfaces {
                surface.selected = Some(surface.id) == closest;
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
            });

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
            });
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
        let deleted = deleted_polylines + deleted_surfaces;
        if deleted == 0 {
            return 0;
        }
        self.save_undo_state();
        self.entities.retain(|entity| !entity.selected);
        self.nurbs_surfaces.retain(|surface| !surface.selected);
        self.selected_entity = None;
        self.rebuild_vertices();
        deleted
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
        let deleted = deleted_polylines + deleted_surfaces;
        if deleted == 0 {
            return 0;
        }
        self.save_undo_state();
        self.entities.retain(|entity| !ids.contains(&entity.id));
        self.nurbs_surfaces.retain(|surface| !ids.contains(&surface.id));
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
            });
        self.rebuild_vertices();
        deleted
    }
    pub fn begin_move_selected(&mut self, x: f32, y: f32) -> bool {
        let mouse = cgmath::Vector2::new(x, y);
        let threshold_sq = 8.0 * 8.0;
        let hit_selected = self
            .entities
            .iter()
            .filter(|entity| entity.selected)
            .any(|entity| {
                let screen_vertices: Vec<cgmath::Vector2<f32>> = entity
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
                if interior_hit {
                    return true;
                }
                if segment_hit {
                    return true;
                }
                if let Some((center, endpoint)) = Self::circle_control_points(&entity.vertices) {
                    return [center, endpoint]
                        .into_iter()
                        .filter_map(|point| self.world_to_screen(point))
                        .any(|screen| (screen - mouse).magnitude2() <= threshold_sq);
                }
                false
            });
        if !hit_selected {
            self.move_anchor = None;
            self.move_snapshots.clear();
            return false;
        }
        self.save_undo_state();
        self.move_anchor = self.fetch_point(PhysicalPosition::new(x as f64, y as f64));
        if self.move_anchor.is_some() {
            self.move_snapshots = self
                .entities
                .iter()
                .filter(|entity| entity.selected)
                .map(|entity| (entity.id, entity.vertices.clone()))
                .collect();
        }
        self.move_anchor.is_some()
    }

    pub fn move_selected(&mut self, x: f32, y: f32) -> bool {
        let Some(anchor) = self.move_anchor else {
            return false;
        };
        self.cursor_pos = PhysicalPosition::new(x as f64, y as f64);
        let Some(current) = self.fetch_point(self.cursor_pos) else {
            return false;
        };
        let offset = self.grid_snap_offset(current - anchor);
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
        self.rebuild_vertices();
        true
    }

    pub fn end_move_selected(&mut self) {
        self.move_anchor = None;
        self.move_snapshots.clear();
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
        let marker_radius = self.world_width_for_pixels(3.5);
        let marker_color = [1.0, 1.0, 1.0, 1.0];
        for entity in &self.nurbs_surfaces {
            let draw_color = if entity.selected { [1.0, 0.8, 0.0, 1.0] } else { entity.color };
            let mesh_verts = entity.surface.sample_mesh(entity.u_subdivisions, entity.v_subdivisions, draw_color);
            new_vertices.extend(mesh_verts);
        }
        for entity in &self.nurbs_curves {
            let draw_color = if entity.selected { [1.0, 0.8, 0.0, 1.0] } else { entity.color };
            let curve_points = entity.curve.sample(64);
            new_vertices.extend(self.tessellate_polyline(&curve_points, entity.thickness, draw_color));
        }
        for entity in &self.entities {
            let draw_color = if entity.selected {
                [1.0, 0.8, 0.0, 1.0]
            } else {
                entity.color
            };
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
                        let fill_color = [draw_color[0], draw_color[1], draw_color[2], 0.45];
                        new_vertices.extend(Self::tessellate_fill_contours(&contours, fill_color));
                    }
                }
            }
            if closed && entity.height.abs() > f32::EPSILON {
                new_vertices.extend(Self::extrude(&entity.vertices, draw_color, entity.height));
            }
            let entity_vertices =
                self.tessellate_polyline(&entity.vertices, entity.thickness, draw_color);
            new_vertices.extend_from_slice(&entity_vertices);
            if entity.show_markers && Self::circle_control_points(&entity.vertices).is_none() {
                let marker_count = if closed {
                    entity.vertices.len() - 1
                } else {
                    entity.vertices.len()
                };
                for point in entity.vertices.iter().take(marker_count) {
                    new_vertices.extend(Self::tessellate_vertex_circle(
                        *point,
                        marker_radius,
                        marker_color,
                    ));
                }
            }
        }
        if let (Some(first), Some(second)) = (self.rectangle_first, self.rectangle_second) {
            let rectangle = Self::rectangle_points(first, second);
            new_vertices.extend(self.tessellate_polyline(
                &rectangle,
                POLYLINE_WIDTH_PIXELS,
                self.polyline_color,
            ));
            for point in rectangle.iter().take(4) {
                new_vertices.extend(Self::tessellate_vertex_circle(
                    *point,
                    marker_radius,
                    marker_color,
                ));
            }
        }
        if let (Some(center), Some(target)) = (self.circle_center, self.circle_target) {
            let preview_points = Self::circle_points_from_target(center, target, 64);
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
                new_vertices.extend(Self::tessellate_vertex_circle(
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
            new_vertices.extend(Self::tessellate_vertex_circle(
                *point,
                marker_radius,
                marker_color,
            ));
        }
        for entity in &self.text_entities {
            new_vertices.extend_from_slice(&entity.cached_vertices);
        }
        for entity in &self.nurbs_surfaces {
            let draw_color = if entity.selected { [1.0, 0.8, 0.0, 1.0] } else { entity.color };
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
                        new_vertices.extend(Self::tessellate_vertex_circle(cp.point, marker_radius, pt_color));
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
                    new_vertices.extend(Self::tessellate_vertex_circle(*pt, marker_radius, [1.0, 0.5, 0.0, 1.0]));
                }
            }
        }
        self.point_vertices = new_vertices;
        self.redraw = true;
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
    pub fn extrude_selected(&mut self, height: f32) -> usize {
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
        self.save_undo_state();
        for entity in &mut self.entities {
            if selected_ids.contains(&entity.id) {
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
        let Some(entity) = self.entities.iter().find(|entity| entity.id == object_id) else{
            return false;
        };
        if !Self::is_closed_shape(&entity.vertices) {
            return false;
        }
        self.save_undo_state();
        let Some(entity) = self.entities.iter_mut().find(|entity| entity.id == object_id) else{
            return false;
        };
        entity.height = height;
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
        let Some(entity) = self.entities.iter().find(|entity| {
            entity.id == id && Self::is_closed_shape(&entity.vertices)
        }) else {
            return false;
        };
        let start_height = entity.height;
        self.save_undo_state();
        self.extrude_drag = Some((id, start_height));
        true
    }
    pub fn preview_extrude_drag(&mut self, delta_y: f32) -> bool {
        if !delta_y.is_finite() {
            return false;
        }
        let Some((id, start_height)) = self.extrude_drag else {
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
        let Some((id, start_height)) = self.extrude_drag.take() else {
            return;
        };
        if !commit {
            if let Some(entity) = self.entities.iter_mut().find(|entity| entity.id == id) {
                entity.height = start_height;
            }
            self.hist_entities.undo.pop();
            self.hist_entities.current = self.entities.clone();
            self.hist_surfaces.undo.pop();
            self.hist_surfaces.current = self.nurbs_surfaces.clone();
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
        let lift = Vector3::new(0.0, POLYLINE_HEIGHT, 0.0);
        let normal_for = |segment: Vector3<f32>| {
            let flat = Vector3::new(segment.x, 0.0, segment.z);
            if flat.magnitude2() <= f32::EPSILON {
                None
            } else {
                let direction = flat.normalize();
                Some(Vector3::new(-direction.z, 0.0, direction.x))
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
    fn tessellate_vertex_circle(point: Vector3<f32>, radius: f32, color: [f32; 4]) -> Vec<Vertex> {
        const SEGMENTS: usize = 16;

        let mut vertices = Vec::with_capacity(SEGMENTS * 3);
        let center = Vector3::new(point.x, point.y + POLYLINE_HEIGHT * 2.0, point.z);

        for index in 0..SEGMENTS {
            let angle1 = index as f32 / SEGMENTS as f32 * std::f32::consts::TAU;
            let angle2 = (index + 1) as f32 / SEGMENTS as f32 * std::f32::consts::TAU;

            let point1 = Vector3::new(
                center.x + angle1.cos() * radius,
                center.y,
                center.z + angle1.sin() * radius,
            );
            let point2 = Vector3::new(
                center.x + angle2.cos() * radius,
                center.y,
                center.z + angle2.sin() * radius,
            );

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
        let difference = point - anchor;
        if difference.x.abs() >= difference.z.abs() {
            Some(Vector3::new(point.x, point.y, anchor.z))
        } else {
            Some(Vector3::new(anchor.x, point.y, point.z))
        }
    }
    fn grid_snap_point(&self, point: Vector3<f32>) -> Vector3<f32> {
        if !self.grid_snap_enabled {
            return point;
        }
        Vector3::new(
            (point.x / self.grid_spacing).round() * self.grid_spacing,
            0.0,
            (point.z / self.grid_spacing).round() * self.grid_spacing,
        )
    }

    fn grid_snap_offset(&self, offset: Vector3<f32>) -> Vector3<f32> {
        if !self.grid_snap_enabled {
            return offset;
        }
        Vector3::new(
            (offset.x / self.grid_spacing).round() * self.grid_spacing,
            offset.y,
            (offset.z / self.grid_spacing).round() * self.grid_spacing,
        )
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

    pub fn set_polyline_color(&mut self, red: f32, green: f32, blue: f32, alpha: f32) {
        let color = [
            red.clamp(0.0, 1.0),
            green.clamp(0.0, 1.0),
            blue.clamp(0.0, 1.0),
            alpha.clamp(0.0, 1.0),
        ];
        self.polyline_color = color;
        for entity in &mut self.entities {
            if entity.selected {
                entity.color = color;
            }
        }
        self.rebuild_vertices();
    }
    fn segment_intersection_xz(
        first_start: Vector3<f32>,
        first_end: Vector3<f32>,
        second_start: Vector3<f32>,
        second_end: Vector3<f32>,
    ) -> Option<Vector3<f32>> {
        let rx = first_end.x - first_start.x;
        let rz = first_end.z - first_start.z;
        let sx = second_end.x - second_start.x;
        let sz = second_end.z - second_start.z;
        let denominator = rx * sz - rz * sx;
        if denominator.abs() <= 0.000001 {
            return None;
        }
        let qx = second_start.x - first_start.x;
        let qz = second_start.z - first_start.z;
        let t = (qx * sz - qz * sx) / denominator;
        let u = (qx * rz - qz * rx) / denominator;
        if !(0.0..=1.0).contains(&t) || !(0.0..=1.0).contains(&u) {
            return None;
        }
        Some(Vector3::new(
            first_start.x + rx * t,
            first_start.y + (first_end.y - first_start.y) * t,
            first_start.z + rz * t,
        ))
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
                        let Some(intersection) = Self::segment_intersection_xz(a, b, c, d) else {
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
        if ray_dir.y.abs() > 1e-6 {
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
    ) -> Vec<Vector3<f32>> {
        let subdivisions = subdivisions.max(16);
        let delta = target - center;
        let radius = (delta.x * delta.x + delta.z * delta.z).sqrt();
        if radius <= f32::EPSILON {
            return Vec::new();
        }
        let start_angle = delta.z.atan2(delta.x);
        let mut points = Vec::with_capacity(subdivisions + 1);
        for index in 0..=subdivisions {
            let angle = start_angle + std::f32::consts::TAU * index as f32 / subdivisions as f32;
            points.push(Vector3::new(
                center.x + radius * angle.cos(),
                center.y,
                center.z + radius * angle.sin(),
            ));
        }
        points
    }

    fn rectangle_points(first: Vector3<f32>, second: Vector3<f32>) -> Vec<Vector3<f32>> {
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
        let points = Self::rectangle_points(first, second);
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
        let radius = (difference.x * difference.x + difference.z * difference.z).sqrt();
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
        let planar_length = (offset.x * offset.x + offset.z * offset.z).sqrt();
        let direction = if planar_length > 0.0001 {
            Vector3::new(offset.x / planar_length, 0.0, offset.z / planar_length)
        } else {
            Vector3::new(1.0, 0.0, 0.0)
        };
        self.circle_target = Some(center + direction * radius);
        self.rebuild_vertices();
        true
    }
    pub fn commit_circle(&mut self, subdivisions: usize) -> bool {
        let (Some(center), Some(target)) = (self.circle_center, self.circle_target) else {
            return false;
        };
        let points = Self::circle_points_from_target(center, target, subdivisions);
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
        let target = center + Vector3::new(radius, 0.0, 0.0);
        let points = Self::circle_points_from_target(center, target, subdivisions);
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
                if let Some(entity) = self.entities.iter_mut().find(|e| e.id == *id) {
                    entity.vertices = vertices.clone();
                }
            }
            self.move_anchor = None;
            self.move_snapshots.clear();
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
