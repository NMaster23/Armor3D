use crate::camera::{Camera, CameraController, CameraUniform, OPENGL_TO_WGPU_MATRIX};
use crate::{GRAPH_INDICES, GRAPH_VERTICES, Vertex};
use cgmath::{Deg, EuclideanSpace, InnerSpace, Matrix4, SquareMatrix, Vector3, Vector4, Zero, perspective};
use lyon::math::point;
use lyon::path::Path;
use lyon::tessellation::{BuffersBuilder, FillOptions, FillTessellator, FillVertex, VertexBuffers};
use std::sync::Arc;
use pyo3::impl_::wrap::SomeWrap;
use wgpu::util::DeviceExt;
use wgpu::wgt::BufferDescriptor;
use winit::dpi::PhysicalPosition;
use winit::{
    event::*,
    event_loop::ActiveEventLoop,
    keyboard::KeyCode,
    window::Window,
};

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
}

const INITIAL_POINT_SIZE: usize = 16;
const SNAP_RADIUS: f32 = 0.05;

pub struct State {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    is_surface_configured: bool,
    pub(crate) window: Option<Arc<Window>>,
    render_pipeline: wgpu::RenderPipeline,
    graph_pipeline: wgpu::RenderPipeline,
    graph_index_buffer: wgpu::Buffer,
    graph_vertex_buffer: wgpu::Buffer,
    point_buffer: wgpu::Buffer,
    point_buffer_capacity: usize,
    num_indices: u32,
    point_vertices: Vec<Vertex>,
    pub osnap: bool,
    pub active_polyline: Vec<Vector3<f32>>,
    pub camera: Camera,
    camera_uniform: CameraUniform,
    camera_buffer: wgpu::Buffer,
    camera_bind_group: wgpu::BindGroup,
    pub camera_controller: CameraController,
    cursor_pos: PhysicalPosition<f64>,
    holding_left: bool,
    pub entities: Vec<PolyLine>,
    pub selected_entity: Option<usize>,
    next_entity: usize,
}

pub struct Shape {
    pub vertices: Vec<Vector3<f32>>,
    pub is_3d: bool,
}

impl State {
    pub fn tessellate_fill(points: &[cgmath::Vector3<f32>], color: [f32; 4]) -> Vec<Vertex> {
        if points.len() < 3 {
            return Vec::new();
        }
        let mut builder = Path::builder();
        builder.begin(point(points[0].x, points[0].y));
        for p in &points[1..] {
            builder.line_to(point(p.x, p.y));
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
        geometry.indices.iter().map(|&i| geometry.vertices[i as usize]).collect()
    }
    pub fn dist_to_segment(p: cgmath::Vector2<f32>, a: cgmath::Vector2<f32>, b: cgmath::Vector2<f32>) -> f32 {
        let ab = b - a;
        let ap = p - a;
        let len_sq = ab.magnitude2();
        if len_sq == 0.0 {
            return ap.magnitude2();
        }
        let t = (ap.dot(ab) / len_sq).clamp(0.0, 1.0);
        let projection = a + ab * t;
        (p-projection).magnitude2()
    }
    pub fn select_shape(&mut self, mouse_px: cgmath::Vector2<f32>, hit_threshold_px: f32) -> Option<usize> {
        let threshold_sq = hit_threshold_px * hit_threshold_px;
        let mut closest = None;
        let mut min_dist_sq = threshold_sq;
        for entity in &self.entities {
            if entity.vertices.len() < 2 {
                continue;
            }
            let screen_vertices: Vec<cgmath::Vector2<f32>> = entity.vertices.iter().filter_map(|p| self.world_to_screen(*p)).collect();
            for i in 0..screen_vertices.len().saturating_sub(1) {
                let dist_sq = Self::dist_to_segment(
                    mouse_px,
                    screen_vertices[i],
                    screen_vertices[i + 1],
                );
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
        self.rebuild_gpu_buffers();
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
        let screen_x = (ndc.x + 1.0) * 0.5 * self.config.width as f32;
        let screen_y = (1.0 - ndc.y) * 0.5 * self.config.height as f32;

        Some(cgmath::Vector2::new(screen_x, screen_y))
    }
    pub fn click_sel_handle(&mut self, mouse_pos: PhysicalPosition<f64>) {
        let mouse_px = cgmath::Vector2::new(mouse_pos.x as f32, mouse_pos.y as f32);
        let hit_threshold = 10.0;
        self.select_shape(mouse_px, hit_threshold);
    }
    pub fn rebuild_gpu_buffers(&mut self) {
        let mut new_vertices = Vec::new();

        for entity in &self.entities {
            let draw_color = if entity.selected {
                [1.0, 0.8, 0.0, 1.0]
            } else {
                entity.color
            };
            if entity.vertices.len() >= 3 {
                let fill_color = [draw_color[0], draw_color[1], draw_color[2], draw_color[3]];
                let fill_verts = Self::tessellate_fill(&entity.vertices, fill_color);
                new_vertices.extend(fill_verts);
            }
            let entity_verts = self.tessellate_polyline(&entity.vertices, entity.thickness, draw_color);
            new_vertices.extend_from_slice(&entity_verts);
        }
        self.point_vertices = new_vertices;
        if self.point_vertices.len() > self.point_buffer_capacity {
            self.point_buffer_capacity = (self.point_vertices.len() * 2).max(64);
            self.point_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Dynamic Point Buffer"),
                size: (self.point_buffer_capacity * std::mem::size_of::<Vertex>()) as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
        }
        if !self.point_vertices.is_empty() {
            self.queue.write_buffer(
                &self.point_buffer,
                0,
                bytemuck::cast_slice(&self.point_vertices),
            );
        }

        if let Some(window) = &self.window {
            window.request_redraw();
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
                Vertex { position: [v0.x, v0.y, v0.z], coords: [0.0, 0.0, 0.0], color },
                Vertex { position: [v1.x, v1.y, v1.z], coords: [0.0, 0.0, 0.0], color },
                Vertex { position: [v2.x, v2.y, v2.z], coords: [0.0, 0.0, 0.0], color },
                Vertex { position: [v2.x, v2.y, v2.z], coords: [0.0, 0.0, 0.0], color },
                Vertex { position: [v1.x, v1.y, v1.z], coords: [0.0, 0.0, 0.0], color },
                Vertex { position: [v3.x, v3.y, v3.z], coords: [0.0, 0.0, 0.0], color },
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
        })
    }
    pub fn graph_handle_key(
        &mut self,
        code: KeyCode,
        is_pressed: bool
    ) -> bool {
        if !is_pressed {
            return false;
        }
        match code {
            KeyCode::KeyC => {
                self.clear();
                true
            },
            KeyCode::Tab => {
                self.osnap = !self.osnap;
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
                true
            }
            _ => false,
        }
    }
    pub fn clear(&mut self) {
        self.point_vertices.clear();
        self.active_polyline.clear();
        self.queue.write_buffer(
            &self.point_buffer,
            0,
            bytemuck::cast_slice(&self.point_vertices),
        );
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }
    pub fn get_snap_pos(
        &mut self,
        point: Vector3<f32>,
    ) -> (Vector3<f32>, bool) {
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
        let (width, height) = (self.config.width, self.config.height);
        if width == 0 || height == 0 {
            return Some(Vector3::zero());
        }
        let ndc_x = (2.0 * mouse_x / width as f64) - 1.0;
        let ndc_y = 1.0 - (2.0 * mouse_y / height as f64);
        let ndc = Vector4::new(ndc_x as f32, ndc_y as f32, 1.0, 1.0);
        let view = Matrix4::look_at_rh(
            self.camera.eye,
            self.camera.target,
            Vector3::unit_y(),
        );
        let projection = perspective(
            Deg(45.0),
            self.camera.aspect,
            0.1,
            100.0,
        );
        let view_projection = OPENGL_TO_WGPU_MATRIX * projection * view;
        let inverse = view_projection.invert().expect("inverse projection");
        let world = inverse * ndc;
        let world_point = world.truncate() / world.w;
        let ray_origin = self.camera.eye.to_vec();
        let ray_dir = (world_point - ray_origin).normalize();
        let horizontal = if ray_dir.y.abs() > f32::EPSILON {
            -ray_origin.y / ray_dir.y
        } else {
            -1.0
        };
        let vertical = if ray_dir.z.abs() > f32::EPSILON {
            -ray_origin.z / ray_dir.z
        } else {
            -1.0
        };
        let hit_pos = match (horizontal >= 0.0, vertical >= 0.0) {
            (true, true) => {
                if horizontal < vertical {
                    ray_origin + ray_dir * horizontal
                } else {
                    ray_origin + ray_dir * vertical
                }
            }
            (true, false) => ray_origin + ray_dir * horizontal,
            (false, true) => ray_origin + ray_dir * vertical,
            (false, false) => return Some(Vector3::zero()),
        };
        Some(hit_pos)
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
        if self.point_vertices.len() > self.point_buffer_capacity {
            self.point_buffer_capacity = (self.point_vertices.len() * 2).max(64);
            self.point_buffer = self.device.create_buffer(&BufferDescriptor {
                label:  Some("Dynamic Vertex Buffer"),
                size: (self.point_buffer_capacity * std::mem::size_of::<Vertex>()) as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
        }
        self.queue.write_buffer(&self.point_buffer, 0, bytemuck::cast_slice(&self.point_vertices));
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }
    pub fn fill_2d(
        &mut self,
        points: &[Vector3<f32>],
    ) {
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
    pub fn polyline(
        &mut self,
        mouse_pos: PhysicalPosition<f64>,
    ) {
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
            self.rebuild_gpu_buffers();
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
            let point = point1 + dir *t;
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
            Vertex { position: p0.into(), coords: [0.0, 0.0, 0.0], color },
            Vertex { position: p1.into(), coords: [0.0, 0.0, 0.0], color },
            Vertex { position: p2.into(), coords: [0.0, 0.0, 0.0], color },
            Vertex { position: p2.into(), coords: [0.0, 0.0, 0.0], color },
            Vertex { position: p1.into(), coords: [0.0, 0.0, 0.0], color },
            Vertex { position: p3.into(), coords: [0.0, 0.0, 0.0], color },
        ]);
        if self.point_vertices.len() > self.point_buffer_capacity {
            self.point_buffer_capacity = (self.point_vertices.len() * 2).max(16);
            self.point_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Point Buffer"),
                size: (self.point_buffer_capacity * std::mem::size_of::<Vertex>()) as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
        }
        self.queue.write_buffer(&self.point_buffer, 0, bytemuck::cast_slice(&self.point_vertices));
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }
    pub async fn new(window: Arc<Window>) -> anyhow::Result<State> {
        let size = window.inner_size();
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            #[cfg(not(target_arch = "wasm32"))]
            backends: wgpu::Backends::PRIMARY,
            #[cfg(target_arch = "wasm32")]
            backends: wgpu::Backends::GL,
            flags: Default::default(),
            memory_budget_thresholds: Default::default(),
            backend_options: Default::default(),
            display: None,
        });
        let surface = instance
            .create_surface(window.clone())
            .expect("Can't create surface");
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
                apply_limit_buckets: true,
            })
            .await?;
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: None,
                required_features: wgpu::Features::empty(),
                experimental_features: wgpu::ExperimentalFeatures::disabled(),
                required_limits: if cfg!(target_arch = "wasm32") {
                    wgpu::Limits::downlevel_webgl2_defaults()
                } else {
                    wgpu::Limits::default()
                },
                memory_hints: Default::default(),
                trace: wgpu::Trace::Off,
            })
            .await?;
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
        });
        let graph_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("graph_shader.wgsl").into()),
        });
        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps
            .formats
            .iter()
            .find(|f| f.is_srgb())
            .copied()
            .unwrap_or(surface_caps.formats[0]);
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width,
            height: size.height,
            present_mode: surface_caps.present_modes[0],
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
            color_space: wgpu::SurfaceColorSpace::Auto,
        };
        let graph_vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Graph Vertex Buffer"),
            contents: bytemuck::cast_slice(GRAPH_VERTICES),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        });
        let graph_index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Graph Index Buffer"),
            contents: bytemuck::cast_slice(GRAPH_INDICES),
            usage: wgpu::BufferUsages::INDEX,
        });
        let point_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Point Buffer"),
            size: (INITIAL_POINT_SIZE * std::mem::size_of::<Vertex>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let num_indices = GRAPH_INDICES.len() as u32;
        let camera = Camera {
            eye: (0.0, 1.0, 2.0).into(),
            target: (0.0, 0.0, 0.0).into(),
            up: cgmath::Vector3::unit_y(),
            aspect: config.width as f32 / config.height as f32,
            fov_y: 45.0,
            z_near: 0.1,
            z_far: 100.0,
            orthographic: false,
        };
        let camera_controller = CameraController::new(0.02);
        let mut camera_uniform = CameraUniform::new();
        camera_uniform.update_view_proj(&camera);
        let camera_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Camera Buffer"),
            contents: bytemuck::cast_slice(&[camera_uniform]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let camera_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
                label: Some("camera_bind_group_layout"),
            });
        let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &camera_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera_buffer.as_entire_binding(),
            }],
            label: Some("camera_bind_group"),
        });
        let render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Render Pipeline Layout"),
                bind_group_layouts: &[Some(&camera_bind_group_layout)],
                immediate_size: 0,
            });
        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Render Pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[Some(Vertex::desc())],
            },
            fragment: Some(wgpu::FragmentState {
                // 3.
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    // 4.
                    format: config.format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview_mask: None,
            cache: None,
        });
        let graph_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Render Pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &graph_shader,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[Some(Vertex::desc())],
            },
            fragment: Some(wgpu::FragmentState {
                // 3.
                module: &graph_shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    // 4.
                    format: config.format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview_mask: None,
            cache: None,
        });
        Ok(Self {
            surface,
            device,
            queue,
            config,
            is_surface_configured: true,
            render_pipeline,
            graph_pipeline,
            window: Some(window),
            graph_vertex_buffer,
            graph_index_buffer,
            num_indices,
            point_vertices: Vec::new(),
            osnap: true,
            active_polyline: Vec::new(),
            point_buffer,
            point_buffer_capacity: INITIAL_POINT_SIZE,
            camera,
            camera_uniform,
            camera_buffer,
            camera_bind_group,
            camera_controller,
            cursor_pos: PhysicalPosition::new(0.0, 0.0),
            holding_left: false,
            entities: Vec::new(),
            selected_entity: None,
            next_entity: 0,
        })
    }

    pub async fn new_embedded(
        hwnd: isize,
        width: u32,
        height: u32,
    ) -> anyhow::Result<State> {
        use raw_window_handle::{
            RawDisplayHandle, RawWindowHandle, Win32WindowHandle, WindowsDisplayHandle,
        };
        use std::num::NonZeroIsize;

        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            flags: Default::default(),
            memory_budget_thresholds: Default::default(),
            backend_options: Default::default(),
            display: None,
        });
        let window_handle = Win32WindowHandle::new(
            NonZeroIsize::new(hwnd).ok_or_else(|| anyhow::anyhow!("invalid HWND"))?,
        );
        let surface = unsafe {
            instance.create_surface_unsafe(wgpu::SurfaceTargetUnsafe::RawHandle {
                raw_display_handle: Some(RawDisplayHandle::Windows(WindowsDisplayHandle::new())),
                raw_window_handle: RawWindowHandle::Win32(window_handle),
            })?
        };
        Self::new_with_surface(instance, surface, width, height).await
    }

    async fn new_with_surface(
        instance: wgpu::Instance,
        surface: wgpu::Surface<'static>,
        width: u32,
        height: u32,
    ) -> anyhow::Result<State> {
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
                apply_limit_buckets: true,
            })
            .await?;
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: None,
                required_features: wgpu::Features::empty(),
                experimental_features: wgpu::ExperimentalFeatures::disabled(),
                required_limits: wgpu::Limits::default(),
                memory_hints: Default::default(),
                trace: wgpu::Trace::Off,
            })
            .await?;
        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps
            .formats
            .iter()
            .find(|format| format.is_srgb())
            .copied()
            .unwrap_or(surface_caps.formats[0]);
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width,
            height,
            present_mode: surface_caps.present_modes[0],
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
            color_space: wgpu::SurfaceColorSpace::Auto,
        };
        surface.configure(&device, &config);

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
        });
        let graph_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Graph Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("graph_shader.wgsl").into()),
        });
        let graph_vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Graph Vertex Buffer"),
            contents: bytemuck::cast_slice(GRAPH_VERTICES),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        });
        let graph_index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Graph Index Buffer"),
            contents: bytemuck::cast_slice(GRAPH_INDICES),
            usage: wgpu::BufferUsages::INDEX,
        });
        let point_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Point Buffer"),
            size: (INITIAL_POINT_SIZE * std::mem::size_of::<Vertex>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
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
        let mut camera_uniform = CameraUniform::new();
        camera_uniform.update_view_proj(&camera);
        let camera_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Camera Buffer"),
            contents: bytemuck::cast_slice(&[camera_uniform]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let camera_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
                label: Some("camera_bind_group_layout"),
            });
        let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &camera_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera_buffer.as_entire_binding(),
            }],
            label: Some("camera_bind_group"),
        });
        let render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Render Pipeline Layout"),
                bind_group_layouts: &[Some(&camera_bind_group_layout)],
                immediate_size: 0,
            });
        let create_pipeline = |module: &wgpu::ShaderModule, blend| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Render Pipeline"),
                layout: Some(&render_pipeline_layout),
                vertex: wgpu::VertexState {
                    module,
                    entry_point: Some("vs_main"),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    buffers: &[Some(Vertex::desc())],
                },
                fragment: Some(wgpu::FragmentState {
                    module,
                    entry_point: Some("fs_main"),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: config.format,
                        blend,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                }),
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                multiview_mask: None,
                cache: None,
            })
        };

        let render_pipeline = create_pipeline(&shader, Some(wgpu::BlendState::REPLACE));
        let graph_pipeline = create_pipeline(&graph_shader, Some(wgpu::BlendState::ALPHA_BLENDING));

        Ok(Self {
            surface,
            device,
            queue,
            config,
            is_surface_configured: width > 0 && height > 0,
            window: None,
            render_pipeline,
            graph_pipeline,
            graph_index_buffer,
            graph_vertex_buffer,
            point_buffer,
            point_buffer_capacity: INITIAL_POINT_SIZE,
            num_indices: GRAPH_INDICES.len() as u32,
            point_vertices: Vec::new(),
            osnap: true,
            active_polyline: Vec::new(),
            camera,
            camera_uniform,
            camera_buffer,
            camera_bind_group,
            camera_controller,
            cursor_pos: PhysicalPosition::new(0.0, 0.0),
            holding_left: false,
            entities: Vec::new(),
            selected_entity: None,
            next_entity: 0,
        })
    }

    pub fn mouse_move(&mut self, x: f64, y: f64) {
        self.cursor_pos = PhysicalPosition::new(x, y);
        if self.holding_left {
            self.drawing(
                self.cursor_pos,
                MouseButton::Left,
                true,
            );
        }
    }

    pub fn mouse_button(&mut self, pressed: bool) {
        self.holding_left = pressed;
        if pressed {
            self.drawing(
                self.cursor_pos,
                MouseButton::Left,
                true,
            );
        }
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width > 0 && height > 0 {
            self.config.width = width;
            self.config.height = height;
            self.camera.aspect = width as f32 / height as f32;
            self.surface.configure(&self.device, &self.config);
            self.is_surface_configured = true;
        }
    }

    pub fn handle_key(&mut self, event_loop: &ActiveEventLoop, code: KeyCode, is_pressed: bool) {
        if self.graph_handle_key(code, is_pressed) {
            return;
        }
        self.camera_controller.handle_key(&mut self.camera, code, is_pressed);
    }
    pub fn render(&mut self) -> anyhow::Result<()> {
        self.camera_controller.update_camera(&mut self.camera);
        self.camera_uniform.update_view_proj(&self.camera);
        self.queue.write_buffer(
            &self.camera_buffer,
            0,
            bytemuck::cast_slice(&[self.camera_uniform]),
        );
        if let Some(window) = &self.window {
            window.request_redraw();
        }
        if !self.is_surface_configured {
            return Ok(());
        }
        let output = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(surface_texture) => surface_texture,
            wgpu::CurrentSurfaceTexture::Suboptimal(surface_texture) => surface_texture,
            wgpu::CurrentSurfaceTexture::Timeout
            | wgpu::CurrentSurfaceTexture::Occluded
            | wgpu::CurrentSurfaceTexture::Validation => {
                return Ok(());
            }
            wgpu::CurrentSurfaceTexture::Outdated => {
                self.surface.configure(&self.device, &self.config);
                return Ok(());
            }
            wgpu::CurrentSurfaceTexture::Lost => {
                anyhow::bail!("Lost device");
            }
        };
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });
        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.1,
                            g: 0.2,
                            b: 0.3,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
                multiview_mask: None,
            });
            render_pass.set_bind_group(0, &self.camera_bind_group, &[]);
            render_pass.set_pipeline(&self.graph_pipeline);
            render_pass.set_vertex_buffer(0, self.graph_vertex_buffer.slice(..));
            render_pass.set_index_buffer(self.graph_index_buffer.slice(..), wgpu::IndexFormat::Uint16);
            render_pass.draw_indexed(0..self.num_indices, 0, 0..1);
            if !self.point_vertices.is_empty() {
                render_pass.set_pipeline(&self.render_pipeline);
                render_pass.set_vertex_buffer(0, self.point_buffer.slice(..));
                render_pass.draw(0..self.point_vertices.len() as u32, 0..1);
            }

        }
        self.queue.submit(std::iter::once(encoder.finish()));
        self.queue.present(output);

        Ok(())
    }
}