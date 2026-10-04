use crate::Vertex;
use crate::camera::{Camera, CameraController};
use cgmath::{
    InnerSpace, SquareMatrix, Vector3, Vector4, Zero,
};
use lyon::lyon_tessellation::{
    BuffersBuilder, FillOptions, FillTessellator, FillVertex, VertexBuffers,
};
use lyon::math::point;
use lyon::path::Path;
use std::mem;
use glyphon::{Attrs, Buffer, Cache, Family, FontSystem, Metrics, Resolution, Shaping, SwashCache, TextAtlas, TextRenderer};
use wgpu::{Device, Queue, TextureFormat};
use winit::dpi::PhysicalPosition;
use winit::event::MouseButton;
use winit::event_loop::{self, ActiveEventLoop};
use winit::keyboard::KeyCode;

const END_SNAP_RADIUS_PIXELS: f32 = 12.0;
const NEAR_SNAP_RADIUS_PIXELS: f32 = 10.0;
const POLYLINE_WIDTH_PIXELS: f32 = 2.5;
const POLYLINE_HEIGHT: f32 = 0.002;
const DEFAULT_POLYLINE_COLOR: [f32; 4] = [214.0 / 255.0, 166.0 / 255.0, 64.0 / 255.0, 1.0];

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct PolyLineVertex {
    position: [f32; 3],
}

#[derive(Clone, Debug)]
pub struct PolyLine {
    pub id: usize,
    pub vertices: Vec<Vector3<f32>>,
    pub color: [f32; 4],
    pub thickness: f32,
    pub selected: bool,
    pub height: f32,
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
    pub hist_entities: ShapeUndoStore<PolyLine>,
    pub entities: Vec<PolyLine>,
    pub active_polyline: Vec<Vector3<f32>>,
    pub preview_point: Option<Vector3<f32>>,
    pub selected_entity: Option<usize>,
    pub osnap: bool,
    pub end_snap_enabled: bool,
    pub near_snap_enabled: bool,
    pub grid_snap_enabled: bool,
    pub grid_spacing: f32,
    pub cursor_pos: PhysicalPosition<f64>,
    pub holding_left: bool,
    pub polyline_active: bool,
    pub polyline_color: [f32; 4],
    pub curve_subdivisions: u32,
    move_anchor: Option<Vector3<f32>>,
    move_snapshots: Vec<(usize, Vec<Vector3<f32>>)>,
    pub point_vertices: Vec<Vertex>,
    next_entity: usize,
    pub camera: Camera,
    pub camera_controller: CameraController,
    pub width: u32,
    pub height: u32,
    pub redraw: bool,
    grab_mode: bool,
    grab_origin: Option<Vector3<f32>>,
    grab_snapshot: Vec<Vector3<f32>>,
    circle_center: Option<Vector3<f32>>,
}

impl Viewport {
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
            entities: Vec::new(),
            active_polyline: Vec::new(),
            preview_point: None,
            selected_entity: None,
            osnap: true,
            end_snap_enabled: false,
            near_snap_enabled: false,
            grid_snap_enabled: false,
            grid_spacing: 0.1,
            cursor_pos: PhysicalPosition::new(0.0, 0.0),
            holding_left: false,
            polyline_active: false,
            polyline_color: DEFAULT_POLYLINE_COLOR,
            curve_subdivisions: 16,
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
    // pub fn edit_shape(_point: cgmath::Vector3<f32>, _color: [f32; 4]) -> Vec<Vertex> {
    //     Vec::new()
    // }
    pub fn store_shape(&mut self, points: &[cgmath::Vector3<f32>], color: [f32; 4]) {
        self.hist_entities.save_undo();
        let shape = PolyLine {
            id: self.next_entity,
            vertices: points.to_vec(),
            color,
            thickness: 1.0,
            selected: false,
            height: 0.0,
        };
        self.next_entity += 1;
        self.hist_entities.current.push(shape);
        self.redraw = true;
    }
    pub fn extrude(points: &[cgmath::Vector3<f32>], color: [f32; 4], height: f32) -> Vec<Vertex> {
        let mut vertices = Vec::new();
        if points.len() < 3 {
            return vertices;
        }
        let bottom_vertices = Self::tessellate_fill(points, color);
        for chunk in bottom_vertices.chunks_exact(3) {
            vertices.push(chunk[0]);
            vertices.push(chunk[2]);
            vertices.push(chunk[1]);
        }
        let mut top_vertices = Self::tessellate_fill(points, color);
        for vertex in &mut top_vertices {
            vertex.position[1] += height;
        }
        vertices.extend(top_vertices);
        let side_color = [color[0] * 0.75, color[1] * 0.75, color[2] * 0.75, color[3] * 0.75];
        for window in points.windows(2) {
            let point0 = window[0];
            let point1 = window[1];
            let bottom0 = [point0.x, point0.y, point0.z];
            let bottom1 = [point1.x, point1.y, point1.z];
            let top0 = [point0.x, point0.y + height, point0.z];
            let top1 = [point1.x, point1.y + height, point1.z];
            vertices.push(Vertex {
                position: bottom0,
                coords: [0.0, 0.0, 0.0],
                color: side_color,
            });
            vertices.push(Vertex {
                position: bottom1,
                coords: [0.0, 0.0, 0.0],
                color: side_color,
            });
            vertices.push(Vertex {
                position: top1,
                coords: [0.0, 0.0, 0.0],
                color: side_color,
            });
            vertices.push(Vertex {
                position: bottom0,
                coords: [0.0, 0.0, 0.0],
                color: side_color,
            });
            vertices.push(Vertex {
                position: top1,
                coords: [0.0, 0.0, 0.0],
                color: side_color,
            });
            vertices.push(Vertex {
                position: top0,
                coords: [0.0, 0.0, 0.0],
                color: side_color,
            });
        }
        vertices
    }
    pub fn tessellate_fill(points: &[cgmath::Vector3<f32>], color: [f32; 4]) -> Vec<Vertex> {
        if points.len() < 3 {
            return Vec::new();
        }
        let mut builder = Path::builder();
        builder.begin(point(points[0].x, points[0].z));
        for p in &points[1..] {
            builder.line_to(point(p.x, p.z));
        }
        builder.close();
        let path = builder.build();
        let mut geometry: VertexBuffers<Vertex, u16> = VertexBuffers::new();
        let mut tessellator = FillTessellator::new();
        let result = tessellator.tessellate_path(
            &path,
            &FillOptions::default(),
            &mut BuffersBuilder::new(&mut geometry, |vertex: FillVertex| {
                let pos = vertex.position();
                Vertex {
                    position: [pos.x, 0.0, pos.y],
                    coords: [0.0, 0.0, 0.0],
                    color,
                }
            }),
        );
        if result.is_err() {
            return Vec::new();
        }
        geometry
            .indices
            .iter()
            .map(|&i| geometry.vertices[i as usize])
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
    pub fn move_shape(
        &mut self,
        original_points: &[Vector3<f32>],
        move_pos: &[Vector3<f32>],
    ) {
        let temp_points: Vec<Vector3<f32>> = original_points
            .iter()
            .zip(move_pos.iter())
            .map(|(point, delta)| *point + *delta)
            .collect();
        let points = temp_points.iter().map(|p| self.grid_snap_point(*p)).collect::<Vec<_>>();
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
    pub fn select_shape(
        &mut self,
        mouse_px: cgmath::Vector2<f32>,
        hit_threshold_px: f32,
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
                .filter_map(|p| self.world_to_screen(*p))
                .collect();
            for i in 0..screen_vertices.len().saturating_sub(1) {
                let dist_sq =
                    Self::dist_to_segment(mouse_px, screen_vertices[i], screen_vertices[i + 1]);
                if dist_sq < min_dist_sq {
                    min_dist_sq = dist_sq;
                    closest = Some(entity.id);
                }
            }
        }
        self.selected_entity = closest;
        for entity in &mut self.entities {
            entity.selected = Some(entity.id) == closest;
        }
        self.rebuild_vertices();
        closest
    }

    fn point_in_rect(point: cgmath::Vector2<f32>, min: cgmath::Vector2<f32>, max: cgmath::Vector2<f32>) -> bool {
        point.x >= min.x && point.x <= max.x && point.y >= min.y && point.y <= max.y
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

    pub fn select_box(&mut self, start_x: f32, start_y: f32, end_x: f32, end_y: f32) -> usize {
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
                screen_vertices.windows(2).any(|segment| {
                    Self::segment_intersects_rect(segment[0], segment[1], min, max)
                })
            };
            if selected {
                selected_ids.push(entity.id);
            }
        }

        for entity in &mut self.entities {
            entity.selected = selected_ids.contains(&entity.id);
        }
        self.selected_entity = selected_ids.first().copied();
        self.rebuild_vertices();
        selected_ids.len()
    }

    pub fn delete_selected(&mut self) -> usize {
        let previous_len = self.entities.len();
        self.entities.retain(|entity| !entity.selected);
        let deleted = previous_len - self.entities.len();
        if deleted > 0 {
            self.selected_entity = None;
            self.rebuild_vertices();
        }
        deleted
    }

    pub fn begin_move_selected(&mut self, x: f32, y: f32) -> bool {
        let mouse = cgmath::Vector2::new(x, y);
        let threshold_sq = 8.0 * 8.0;
        let hit_selected = self.entities.iter().filter(|entity| entity.selected).any(|entity| {
            let screen_vertices: Vec<cgmath::Vector2<f32>> = entity
                .vertices
                .iter()
                .filter_map(|point| self.world_to_screen(*point))
                .collect();
            screen_vertices.windows(2).any(|segment| {
                Self::dist_to_segment(mouse, segment[0], segment[1]) <= threshold_sq
            })
        });
        if !hit_selected {
            self.move_anchor = None;
            self.move_snapshots.clear();
            return false;
        }
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
            if let Some(entity) = self.entities.iter_mut().find(|entity| entity.id == *entity_id) {
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
        self.select_shape(mouse_px, hit_threshold);
    }
    pub fn rebuild_vertices(&mut self) {
        let mut new_vertices = Vec::new();
        let marker_radius = self.world_width_for_pixels(3.5);
        let marker_color = [1.0, 1.0, 1.0, 1.0];
        for entity in &self.entities {
            let draw_color = if entity.selected {
                [1.0, 0.8, 0.0, 1.0]
            } else {
                entity.color
            };
            if entity.vertices.len() >= 3 && entity.height > 0.0 {
                let fill_color = [draw_color[0], draw_color[1], draw_color[2], draw_color[3]];
                let mesh_vertices = Self::extrude(&entity.vertices, fill_color, entity.height);
                new_vertices.extend_from_slice(&mesh_vertices);
            }
            let entity_vertices = self.tessellate_polyline(&entity.vertices, entity.thickness, draw_color);
            new_vertices.extend_from_slice(&entity_vertices);
            let closed = entity.vertices.len() > 2
                && (entity.vertices[0] - entity.vertices[entity.vertices.len() - 1])
                .magnitude2()
                <= f32::EPSILON;
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
        let mut preview_polyline = self.active_polyline.clone();
        if let Some(preview_point) = self.preview_point {
            let should_append = preview_polyline
                .last()
                .map(|last| (*last - preview_point).magnitude2() > f32::EPSILON)
                .unwrap_or(false);
            if should_append {
                preview_polyline.push(preview_point);
            }
        }
        if preview_polyline.len() > 1 {
            new_vertices.extend(self.tessellate_polyline(
                &preview_polyline,
                POLYLINE_WIDTH_PIXELS,
                self.polyline_color,
            ));
        }
        for point in &preview_polyline {
            new_vertices.extend(Self::tessellate_vertex_circle(
                *point,
                marker_radius,
                marker_color,
            ));
        }
        self.point_vertices = new_vertices;
        self.redraw = true;
    }
    pub fn extrude_selected(&mut self, height: f32) {
        let entity = match self.selected_entity {
            Some(selected_id) => self.entities.iter_mut().find(|entity| entity.id == selected_id),
            None => self.entities.last_mut(),
        };
        if let Some(entity) = entity {
            entity.height = height;
            self.rebuild_vertices();
        }
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
        let closed = points.len() > 2
            && (points[0] - points[points.len() - 1]).magnitude2() <= f32::EPSILON;
        let point_count = if closed { points.len() - 1 } else { points.len() };
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
                if closed { points[point_count - 1] } else { current }
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
        let camera_distance = (self.camera.eye - self.camera.target).magnitude().max(0.001);
        let visible_height =
            2.0 * camera_distance * (self.camera.fov_y.to_radians() * 0.5).tan();
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
            Vertex { position: p0, coords: [0.0; 3], color },
            Vertex { position: p1, coords: [0.0; 3], color },
            Vertex { position: p2, coords: [0.0; 3], color },
            Vertex { position: p2, coords: [0.0; 3], color },
            Vertex { position: p1, coords: [0.0; 3], color },
            Vertex { position: p3, coords: [0.0; 3], color },
        ]
    }
    fn tessellate_vertex_circle(
    point: Vector3<f32>,
    radius: f32,
    color: [f32; 4],
) -> Vec<Vertex> {
    const SEGMENTS: usize = 16;

    let mut vertices = Vec::with_capacity(SEGMENTS * 3);
    let center = Vector3::new(
        point.x,
        point.y + POLYLINE_HEIGHT * 2.0,
        point.z,
    );

    for index in 0..SEGMENTS {
        let angle1 =
            index as f32 / SEGMENTS as f32 * std::f32::consts::TAU;
        let angle2 =
            (index + 1) as f32 / SEGMENTS as f32 * std::f32::consts::TAU;

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
    pub fn add_polyline(&mut self, vertices: Vec<Vector3<f32>>, color: [f32; 4], thickness: f32) {
        let id = self.next_entity;
        self.next_entity += 1;
        self.entities.push(PolyLine {
            id,
            vertices,
            color,
            thickness,
            selected: false,
            height: 0.0,
        })
    }
    pub fn graph_handle_key(&mut self, code: KeyCode, is_pressed: bool) -> bool {
        if code == KeyCode::KeyG {
            if is_pressed {
                if !self.grab_mode {
                    if let Some(selected_id) = self.selected_entity {
                        if let Some(entity) = self.entities.iter().find(|entity| entity.id == selected_id) {
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
            KeyCode::KeyC => {
                self.clear();
                true
            }
            KeyCode::Tab => {
                self.osnap = !self.osnap;
                self.redraw = true;
                true
            }
            KeyCode::KeyE => {
                self.extrude_step(0.5);
                true
            }
            _ => false,
        }
    }
    pub fn clear(&mut self) {
        self.point_vertices.clear();
        self.active_polyline.clear();
        self.preview_point = None;
        self.redraw = true;
    }

    pub fn start_polyline(&mut self) {
        self.active_polyline.clear();
        self.preview_point = None;
        self.holding_left = false;
        self.polyline_active = true;
        self.rebuild_vertices();
    }

    pub fn finish_polyline(&mut self) {
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
        self.holding_left = false;
        self.polyline_active = false;
        self.active_polyline.clear();
        self.preview_point = None;
        self.rebuild_vertices();
    }
    pub fn set_osnap_modes(&mut self, end_enabled: bool, near_enabled: bool) {
        self.osnap = end_enabled || near_enabled;
        self.end_snap_enabled = end_enabled;
        self.near_snap_enabled = near_enabled;
        if !self.osnap {
            self.preview_point = None;
            self.rebuild_vertices();
        }
    }

    pub fn set_grid_snap(&mut self, enabled: bool) {
        self.grid_snap_enabled = enabled;
    }

    pub fn set_grid_spacing(&mut self, spacing: f32) {
        if spacing.is_finite() && spacing > 0.0 {
            self.grid_spacing = spacing;
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
        Some((point.x, point.z, point.y))
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

    pub fn get_snap_pos(
        &self,
        point: Vector3<f32>,
    ) -> (Vector3<f32>, bool, Option<&'static str>) {
        if self.end_snap_enabled {
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
                if let Some(&first) = entity.vertices.first() {
                    consider_end(first, false);
                }
                if let Some(&last) = entity.vertices.last() {
                    consider_end(last, false);
                }
            }
            if let Some((_, position, closes)) = best_end {
                return (position, closes, Some("End"));
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

        if self.grid_snap_enabled {
            let snapped = self.grid_snap_point(point);
            return (snapped, false, Some("Grid"));
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
        let ndc = Vector4::new(ndc_x as f32, ndc_y as f32, 1.0, 1.0);
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
    pub fn fill_2d(&mut self, points: &[Vector3<f32>]) {
        if points.len() < 3 {
            return;
        }
        let mut builder = Path::builder();
        builder.begin(point(points[0].x, points[0].z));
        for p in &points[1..] {
            builder.line_to(point(p.x, p.z));
        }
        builder.close();
        let path = builder.build();
        let mut geometry: VertexBuffers<Vertex, u16> = VertexBuffers::new();
        let mut tessellator = FillTessellator::new();
        let color = [0.4, 0.7, 0.4, 0.7];
        let result = tessellator.tessellate_path(
            &path,
            &FillOptions::default(),
            &mut BuffersBuilder::new(&mut geometry, |vertex: FillVertex| {
                let vertex_pos = vertex.position();
                Vertex {
                    position: [vertex_pos.x, 0.0, vertex_pos.y],
                    coords: [0.0, 0.0, 0.0],
                    color,
                }
            }),
        );
        if result.is_err() {
            return;
        }
        let mut flat_vertices = Vec::with_capacity(geometry.indices.len());
        for &index in &geometry.indices {
            flat_vertices.push(geometry.vertices[index as usize]);
        }
        self.vertice_append(&flat_vertices);
    }
    pub fn polyline(&mut self, mouse_pos: PhysicalPosition<f64>) -> bool {
        let hit_pos = match self.fetch_point(mouse_pos) {
            Some(hit_pos) => hit_pos,
            None => return false,
        };
        let (snap_pos, shape_close, _) = self.get_snap_pos(hit_pos);
        if shape_close {
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
    pub fn draw_circle_mouse(&mut self, subdivisions: usize, mouse_pos: PhysicalPosition<f64>, mouse_button: MouseButton) {
        if mouse_button != MouseButton::Left {
            return;
        }
        let Some(point) = self.fetch_point(mouse_pos) else {
            return;
        };
        if let Some(center) = self.circle_center.take() {
            let delta = point - center;
            let radius = (delta.x * delta.x + delta.z * delta.z).sqrt();
            self.draw_circle_command(subdivisions, radius, center);
        } else {
            self.circle_center = Some(point)
        }
    }
    pub fn draw_circle_command(&mut self, subdivisions: usize, radius: f32, circle_pos: Vector3<f32>) {
        if subdivisions < 3 || !radius.is_finite() || radius <= 0.0 {
            return;
        }
        let mut points: Vec<_> = (0..subdivisions)
            .map(|i| {
                let theta = i as f32 / subdivisions as f32 * std::f32::consts::TAU;
                Vector3::new(
                    circle_pos.x + radius * theta.cos(),
                    circle_pos.y,
                    circle_pos.z + radius * theta.sin(),
                )
            })
            .collect();
        points.push(points[0]);
        self.add_polyline(points, self.polyline_color, POLYLINE_WIDTH_PIXELS);
        self.rebuild_vertices();
    }
    pub fn draw_text(&mut self, text: &str) {

    }
    pub fn draw_curve(&mut self, subdivisions: usize) {
        if self.active_polyline.len() < 2 || subdivisions == 0 {
            self.active_polyline.clear();
            self.preview_point = None;
            self.polyline_active = false;
            self.holding_left = false;
            self.rebuild_vertices();
            return;
        }
        let points = mem::take(&mut self.active_polyline);
        let mut sampled_points = Vec::new();
        for i in 0..points.len() - 1 {
            let p0 = points[i.saturating_sub(1)];
            let p1 = points[i];
            let p2 = points[i + 1];
            let p3 = points[(i + 2).min(points.len() - 1)];
            for step in 0..subdivisions {
                let t = step as f32 / subdivisions as f32;
                sampled_points.push(Self::catmull_rom(p0, p1, p2, p3, t));
            }
        }
        sampled_points.push(*points.last().expect("Missing last point"));
        self.add_polyline(
            sampled_points,
            self.polyline_color,
            POLYLINE_WIDTH_PIXELS,
        );
        self.preview_point = None;
        self.polyline_active = false;
        self.holding_left = false;
        self.rebuild_vertices();
    }
    pub fn catmull_rom(
        p0: Vector3<f32>,
        p1: Vector3<f32>,
        p2: Vector3<f32>,
        p3: Vector3<f32>,
        t: f32,
    ) -> Vector3<f32> {
        let t2 = t * t;
        let t3 = t2 * t;
        (p1 * 2.0
            + (p2 - p0) * t
            + (p0 * 2.0 - p1 * 5.0 + p2 * 4.0 - p3) * t2
            + (-p0 + p1 * 3.0 - p2 * 3.0 + p3) * t3)
            * 0.5
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
                if let Some(entity) = self.entities.iter_mut().find(|entity| entity.id == selected_id) {
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
                .map(|previous| {
                    (previous - position).magnitude2() > 0.00000001
                })
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

    pub fn handle_key(&mut self, event_loop: &ActiveEventLoop, code: KeyCode, is_pressed: bool) {
        if self.graph_handle_key(code, is_pressed) {
            return;
        }
        self.camera_controller
            .handle_key(&mut self.camera, code, is_pressed);
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
        }
    }

    #[test]
    fn grab_key_moves_selected_shape_until_released() {
        let mut viewport = Viewport::new(800, 600);
        let original = vec![
            Vector3::new(-0.5, 0.0, 0.0),
            Vector3::new(0.5, 0.0, 0.0),
        ];
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
            shape(1, vec![Vector3::new(0.0, 0.0, 0.0), Vector3::new(1.0, 0.0, 0.0)]),
            shape(2, vec![Vector3::new(2.0, 0.0, 0.0), Vector3::new(3.0, 0.0, 0.0)]),
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
        viewport.active_polyline = vec![
            Vector3::new(-0.5, 0.0, 0.0),
            Vector3::new(0.5, 0.0, 0.0),
        ];
        viewport.set_osnap_modes(false, true);
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
        viewport.set_osnap_modes(true, false);

        let (snapped, finishes_polyline, snap_kind) =
            viewport.get_snap_pos(Vector3::new(0.01, 0.0, 0.01));

        assert!(snapped.magnitude() < 0.00001);
        assert!(!finishes_polyline);
        assert_eq!(snap_kind, Some("End"));
    }
}

pub fn egui_extrude_input(ctx: &egui::Context, viewport: &mut Viewport) {
    egui::Window::new("Extrude Selected")
        .resizable(false)
        .collapsible(false)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("Height:");
                let mut height = viewport
                    .selected_entity
                    .and_then(|id| viewport.entities.iter().find(|e| e.id == id))
                    .map(|e| e.height)
                    .unwrap_or(0.0);
                if ui.add(egui::DragValue::new(&mut height).speed(0.1)).changed() {
                    viewport.extrude_selected(height);
                }
            });
        });
}
