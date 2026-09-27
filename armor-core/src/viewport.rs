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
use winit::dpi::PhysicalPosition;
use winit::event::MouseButton;
use winit::event_loop::ActiveEventLoop;
use winit::keyboard::KeyCode;

pub const SNAP_RADIUS: f32 = 0.05;

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

impl <T: Clone> ShapeUndoStore<T> {
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
    pub selected_entity: Option<usize>,
    pub osnap: bool,
    pub cursor_pos: PhysicalPosition<f64>,
    pub holding_left: bool,
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
            orthographic: true,
        };
        let camera_controller = CameraController::new(0.02);
        Self {
            hist_entities: ShapeUndoStore::new(Vec::new()),
            entities: Vec::new(),
            active_polyline: Vec::new(),
            selected_entity: None,
            osnap: true,
            cursor_pos: PhysicalPosition::new(0.0, 0.0),
            holding_left: false,
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
    pub fn edit_shape(point: cgmath::Vector3<f32>, color: [f32; 4]) -> Vec<Vertex> {
        Vec::new()
    }
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
        for entity in &self.entities {
            let draw_color = if entity.selected {
                [1.0, 0.8, 0.0, 1.0]
            } else {
                entity.color
            };
            if entity.vertices.len() >= 3 {
                let fill_color = [draw_color[0], draw_color[1], draw_color[2], draw_color[3]];
                if entity.height > 0.0 {
                    let mesh_vertices = Self::extrude(&entity.vertices, fill_color, entity.height);
                    new_vertices.extend_from_slice(&mesh_vertices);
                } else {
                    let fill_vertices = Self::tessellate_fill(&entity.vertices, fill_color);
                    new_vertices.extend_from_slice(&fill_vertices);
                }
            }
            let entity_vertices = self.tessellate_polyline(&entity.vertices, entity.thickness, draw_color);
            new_vertices.extend_from_slice(&entity_vertices);
        }
        self.point_vertices = new_vertices;
        self.redraw = true;
    }
    pub fn extrude_selected(&mut self, height: f32) {
        if let Some(selected_id) = self.selected_entity {
            if let Some(entity) = self.entities.iter_mut().find(|e| e.id == selected_id) {
                entity.height = height;
                self.rebuild_vertices();
            }
        }
        if let Some(last_entity) = self.entities.last_mut() {
            last_entity.height = height;
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
        let half_w = thickness * 0.5;
        for window in points.windows(2) {
            let p1 = window[0];
            let p2 = window[1];
            let dir = (p2 - p1).normalize();
            let normal = cgmath::Vector3::new(-dir.z, 0.0, dir.x) * half_w;

            let v0 = p1 + normal;
            let v1 = p1 - normal;
            let v2 = p2 + normal;
            let v3 = p2 - normal;
            let quad = [
                Vertex {
                    position: [v0.x, v0.y, v0.z],
                    coords: [0.0, 0.0, 0.0],
                    color,
                },
                Vertex {
                    position: [v1.x, v1.y, v1.z],
                    coords: [0.0, 0.0, 0.0],
                    color,
                },
                Vertex {
                    position: [v2.x, v2.y, v2.z],
                    coords: [0.0, 0.0, 0.0],
                    color,
                },
                Vertex {
                    position: [v2.x, v2.y, v2.z],
                    coords: [0.0, 0.0, 0.0],
                    color,
                },
                Vertex {
                    position: [v1.x, v1.y, v1.z],
                    coords: [0.0, 0.0, 0.0],
                    color,
                },
                Vertex {
                    position: [v3.x, v3.y, v3.z],
                    coords: [0.0, 0.0, 0.0],
                    color,
                },
            ];
            vertices.extend_from_slice(&quad);
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
            KeyCode::KeyG => {
                if let Some(selected) = self.selected_entity {
                    if let Some(entity) = self.entities.iter_mut().find(|e| e.id == selected) {
                        self.grab_snapshot = entity.vertices.clone();
                    }
                    self.grab_origin = self.fetch_point(self.cursor_pos);
                    self.grab_mode = true;
                }
                true
            }
            _ => false,
        }
    }
    pub fn clear(&mut self) {
        self.point_vertices.clear();
        self.active_polyline.clear();
        self.redraw = true;
    }
    pub fn get_snap_pos(&mut self, point: Vector3<f32>) -> (Vector3<f32>, bool) {
        if self.active_polyline.len() >= 3 {
            if let Some(&start) = self.active_polyline.first() {
                if (point - start).magnitude() <= SNAP_RADIUS {
                    return (point, true);
                }
            }
        }
        for &vertex in &self.active_polyline {
            if (point - vertex).magnitude() <= SNAP_RADIUS {
                return (point, false);
            }
        }
        (point, false)
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
                self.polyline(mouse_pos);
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
    pub fn polyline(&mut self, mouse_pos: PhysicalPosition<f64>) {
        let hit_pos = match self.fetch_point(mouse_pos) {
            Some(hit_pos) => hit_pos,
            None => return,
        };
        let (snap_pos, shape_close) = if self.osnap {
            self.get_snap_pos(hit_pos)
        } else {
            (hit_pos, false)
        };
        if shape_close {
            self.active_polyline.push(self.active_polyline[0]);
            let points = self.active_polyline.clone();
            self.add_polyline(points, [0.4, 0.7, 0.4, 0.7], 0.05);
            self.active_polyline.clear();
            self.rebuild_vertices();
        } else {
            self.active_polyline.push(snap_pos);
            if self.active_polyline.len() > 1 {
                let last = self.active_polyline[self.active_polyline.len() - 2];
                self.add_line(last, snap_pos, 0.008);
            } else {
                self.add_point(snap_pos);
            }
        }
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

    pub fn mouse_move(&mut self, x: f64, y: f64) {
        self.cursor_pos = PhysicalPosition::new(x, y);
        if self.holding_left {
            self.drawing(self.cursor_pos, MouseButton::Left, true);
        } else if self.grab_mode {
            let mouse_pos = self.fetch_point(self.cursor_pos);
            let delta = mouse_pos.expect("Error unwrapping") - self.grab_origin.expect("Error unwrapping");
            let preview_pos: Vec<_> = self.grab_snapshot.iter().map(|p| p + delta).collect();
            self.rebuild_vertices();
        } else {
            return;
        }
    }

    pub fn mouse_button(&mut self, pressed: bool) {
        self.holding_left = pressed;
        if pressed {
            self.drawing(self.cursor_pos, MouseButton::Left, true);
        }
    }

    pub fn handle_key(&mut self, event_loop: &ActiveEventLoop, code: KeyCode, is_pressed: bool) {
        if self.graph_handle_key(code, is_pressed) {
            return;
        }
        self.camera_controller
            .handle_key(&mut self.camera, code, is_pressed);
    }
}

