use crate::camera::{Camera, CameraUniform};
use crate::{GRAPH_INDICES, GRAPH_VERTICES, Vertex};
use cgmath::Vector3;
use glyphon::{Attrs, Buffer, Color, FontSystem, Metrics, Resolution, SwashCache, TextArea, TextAtlas, TextBounds, TextRenderer};
use std::sync::Arc;
use wgpu::util::DeviceExt;
use winit::dpi::PhysicalPosition;
use winit::window::Window;
use crate::viewport::{PolyLine, Viewport};

const INITIAL_POINT_SIZE: usize = 16;

const MESH_SHADER: &str = r#"
struct CameraLightUniform {
    view_proj: mat4x4<f32>,
    light_pos: vec4<f32>,
};

@group(0) @binding(0)
var<uniform> camera_light: CameraLightUniform;

@group(1) @binding(0)
var material_texture: texture_2d<f32>;

@group(1) @binding(1)
var material_sampler: sampler;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) texture_coordinates: vec2<f32>,
    @location(3) color: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) texture_coordinates: vec2<f32>,
    @location(1) color: vec4<f32>,
};

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;
    output.clip_position =
        camera_light.view_proj * vec4<f32>(input.position, 1.0);
    output.texture_coordinates = input.texture_coordinates;
    output.color = input.color;
    return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let texel = textureSample(
        material_texture,
        material_sampler,
        input.texture_coordinates
    );
    return texel * input.color;
}
"#;

pub struct State {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    font_system: FontSystem,
    swash_cache: SwashCache,
    text_viewport: glyphon::Viewport,
    text_atlas: TextAtlas,
    text_renderer: TextRenderer,
    text_buffer: Buffer,
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
    camera_uniform: CameraUniform,
    camera_buffer: wgpu::Buffer,
    camera_bind_group: wgpu::BindGroup,
    cursor_pos: PhysicalPosition<f64>,
    holding_left: bool,
    pub entities: Vec<PolyLine>,
    pub selected_entity: Option<usize>,
    next_entity: usize,
    pub viewport: Viewport,
    text_pos: Vector3<f32>,
}

impl State {
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
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
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
                module: &graph_shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
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
        let viewport = Viewport::new(
            size.width,
            size.height,
        );
        let mut font_system = FontSystem::new();
        let cache = glyphon::Cache::new(&device);
        let text_viewport = glyphon::Viewport::new(&device, &cache);
        let swash_cache = SwashCache::new();
        let cache_state = wgpu::MultisampleState::default();
        let mut text_atlas = TextAtlas::new(&device, &queue, &cache, config.format);
        let text_renderer = TextRenderer::new(&mut text_atlas, &device, cache_state, None);
        let mut text_buffer = Buffer::new(&mut font_system, Metrics::new(16.0, 20.0));
        text_buffer.set_size(Some(size.width as f32), Some(size.height as f32));
        text_buffer.set_text(
            "",
            &Attrs::new().family(glyphon::Family::SansSerif),
            glyphon::Shaping::Advanced,
            None,
        );
        Ok(Self {
            surface,
            device,
            queue,
            config,
            font_system,
            swash_cache,
            text_viewport,
            text_atlas,
            text_renderer,
            text_buffer,
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
            camera_uniform,
            camera_buffer,
            camera_bind_group,
            cursor_pos: PhysicalPosition::new(0.0, 0.0),
            holding_left: false,
            entities: Vec::new(),
            selected_entity: None,
            next_entity: 0,
            viewport,
            text_pos: Vector3::new(0.0, 0.0, 0.0),
        })
    }
    pub fn draw_text(&mut self, text: &str, pos: Vector3<f32>) {
        self.text_buffer.set_text(
            text,
            &Attrs::new().family(glyphon::Family::SansSerif),
            glyphon::Shaping::Advanced,
            None
        );
        self.text_buffer.shape_until_scroll(&mut self.font_system, false);
        self.text_pos = pos;
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
    pub async fn new_with_surface(
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
        let viewport = Viewport::new(
            width,
            height
        );
        let mut font_system = FontSystem::new();
        let cache = glyphon::Cache::new(&device);
        let _text_viewport = glyphon::Viewport::new(&device, &cache);
        let text_viewport = glyphon::Viewport::new(&device, &cache);
        let swash_cache = SwashCache::new();
        let _cache_state = wgpu::MultisampleState::default();
        let mut text_atlas = TextAtlas::new(&device, &queue, &cache, config.format);
        let text_renderer = TextRenderer::new(&mut text_atlas, &device, wgpu::MultisampleState::default(), None);
        let mut text_buffer = Buffer::new(&mut font_system, Metrics::new(16.0, 20.0));
        text_buffer.set_size(Some(width as f32), Some(height as f32));
        // Temporary viewport text disabled while the text tool is being built.
        // text_buffer.set_text(
        //     "Hello, World!",
        //     &Attrs::new().family(glyphon::Family::SansSerif),
        //     glyphon::Shaping::Advanced,
        //     None,
        // );
        text_buffer.shape_until_scroll(&mut font_system, false);
        Ok(Self {
            surface,
            device,
            queue,
            config,
            font_system,
            swash_cache,
            text_viewport,
            text_atlas,
            text_renderer,
            text_buffer,
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
            camera_uniform,
            camera_buffer,
            camera_bind_group,
            cursor_pos: PhysicalPosition::new(0.0, 0.0),
            holding_left: false,
            entities: Vec::new(),
            selected_entity: None,
            next_entity: 0,
            viewport,
            text_pos: Vector3::new(0.0, 0.0, 0.0),
        })
    }
    pub fn resize(&mut self, width: u32, height: u32) {
        if width > 0 && height > 0 {
            self.config.width = width;
            self.config.height = height;
            self.viewport.width = width;
            self.viewport.height = height;
            self.viewport.camera.aspect = width as f32 / height as f32;
            self.viewport.rebuild_vertices();
            self.surface.configure(&self.device, &self.config);
            self.is_surface_configured = true;
            self.text_buffer.set_size(Some(width as f32), Some(height as f32));
        }
    }

    pub fn render(&mut self) -> anyhow::Result<()> {
        if self.viewport.redraw {
            let vertices = &self.viewport.point_vertices;
            if vertices.len() > self.point_buffer_capacity {
                self.point_buffer_capacity = (vertices.len() * 2).max(16);
                self.point_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("Point Buffer"),
                    size: (self.point_buffer_capacity * std::mem::size_of::<Vertex>()) as u64,
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                });
            }
            if !vertices.is_empty() {
                self.queue.write_buffer(&self.point_buffer, 0, bytemuck::cast_slice(vertices));
            }
            self.viewport.redraw = false;
        }
        self.viewport.camera_controller.update_camera(&mut self.viewport.camera);
        self.camera_uniform.update_view_proj(&self.viewport.camera);
        self.queue.write_buffer(
            &self.camera_buffer,
            0,
            bytemuck::cast_slice(&[self.camera_uniform]),
        );
        if let Some(window) = &self.window {
            window.request_redraw();
        }
        self.text_viewport.update(&self.queue, Resolution { width: self.config.width, height: self.config.height });
        let text_areas = match self.viewport.world_to_screen(self.text_pos) {
            Some(screen_pos) => vec![TextArea {
                buffer: &self.text_buffer,
                left: screen_pos.x as f32,
                top: screen_pos.y as f32,
                scale: 1.0,
                bounds: TextBounds::default(),
                default_color: Color::rgb(255, 255, 255),
                custom_glyphs: &[]
            }],
            None => Vec::new(),
        };
        self.text_renderer
            .prepare(
                &self.device,
                &self.queue,
                &mut self.font_system,
                &mut self.text_atlas,
                &self.text_viewport,
                text_areas,
                &mut self.swash_cache,
            ).map_err(|e| anyhow::anyhow!("Preparing text failed with error: {e}"))?;
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
                            r: 0.025,
                            g: 0.03,
                            b: 0.02,
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
            if !self.viewport.point_vertices.is_empty() {
                render_pass.set_pipeline(&self.render_pipeline);
                render_pass.set_vertex_buffer(0, self.point_buffer.slice(..));
                render_pass.draw(0..self.viewport.point_vertices.len() as u32, 0..1);
            }
            self.text_renderer
                .render(
                    &mut self.text_atlas,
                    &self.text_viewport,
                    &mut render_pass,
                )
                .map_err(|error| anyhow::anyhow!("Rendering text failed: {error} "))?;

        }
        self.queue.submit(std::iter::once(encoder.finish()));
        self.queue.present(output);

        Ok(())
    }
}

pub struct Renderer {
    pipeline: wgpu::RenderPipeline,
    camera_light: wgpu::BindGroupLayout,
    material: wgpu::BindGroupLayout,
    camera_light_buffer: wgpu::Buffer,
    camera_light_bind: wgpu::BindGroup,
    shapes: Vec<ShapeRenderer>,
    color_texture: wgpu::Texture,
    color_view: wgpu::TextureView,
    depth_texture: wgpu::Texture,
    depth_view: wgpu::TextureView,
    output_size: (u32, u32),
    output_format: wgpu::TextureFormat,
}

pub struct ShapeRenderer {
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    index_count: u32,
    material: Material,
}

pub struct Material {
    texture: wgpu::Texture,
    texture_view: wgpu::TextureView,
    sampler: wgpu::Sampler,
    bind_group: wgpu::BindGroup,
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct VertexRenderer {
    position: [f32; 3],
    normal: [f32; 3],
    texture_coordinates: [f32; 2],
    color: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct CameraLightUniform {
    view_proj: [[f32; 4]; 4],
    light_pos: [f32; 4],
}

impl VertexRenderer {
    fn desc() -> wgpu::VertexBufferLayout<'static> {
        const ATTRIBUTES: [wgpu::VertexAttribute; 4] = [
            wgpu::VertexAttribute {
                offset: 0,
                shader_location: 0,
                format: wgpu::VertexFormat::Float32x3,
            },
            wgpu::VertexAttribute {
                offset: 12,
                shader_location: 1,
                format: wgpu::VertexFormat::Float32x3,
            },
            wgpu::VertexAttribute {
                offset: 24,
                shader_location: 2,
                format: wgpu::VertexFormat::Float32x2,
            },
            wgpu::VertexAttribute {
                offset: 32,
                shader_location: 3,
                format: wgpu::VertexFormat::Float32x4,
            },
        ];
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Self>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &ATTRIBUTES,
        }
    }
}

impl Renderer {
    pub fn new(
        device: &wgpu::Device,
        width: u32,
        height: u32,
        output_format: wgpu::TextureFormat,
    ) -> anyhow::Result<Self> {
        anyhow::ensure!(width > 0 && height > 0, "Renderer Dimensions Must Be NonZero");
        let camera_light = device.create_bind_group_layout(
            &wgpu::BindGroupLayoutDescriptor {
                label: Some("Camera and Light Layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            },
        );
        let material = device.create_bind_group_layout(
            &wgpu::BindGroupLayoutDescriptor {
                label: Some("Material Layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float {
                                filterable: true,
                            },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(
                            wgpu::SamplerBindingType::Filtering,
                        ),
                        count: None,
                    },
                ],
            },
        );
        let uniform = CameraLightUniform {
            view_proj: [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
            light_pos: [0.0, 5.0, 0.0, 1.0],
        };
        let camera_light_buffer =
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Camera and Light Buffer"),
                contents: bytemuck::bytes_of(&uniform),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            });
        let camera_light_bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Camera and Light Bind Group"),
            layout: &camera_light,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera_light_buffer.as_entire_binding(),
            }],
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Mesh Shader"),
            source: wgpu::ShaderSource::Wgsl(MESH_SHADER.into()),
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Mesh Pipeline Layout"),
            bind_group_layouts: &[Some(&camera_light), Some(&material)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Mesh Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[Some(VertexRenderer::desc())],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: output_format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::Less),
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        let target_size = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };
        let color_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Renderer Color Target"),
            size: target_size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: output_format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let color_view = color_texture.create_view(&wgpu::TextureViewDescriptor::default());

        let depth_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Renderer Depth Target"),
            size: target_size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let depth_view = depth_texture.create_view(&wgpu::TextureViewDescriptor::default());

        Ok(Self {
            pipeline,
            camera_light,
            material,
            camera_light_buffer,
            camera_light_bind,
            shapes: Vec::new(),
            color_texture,
            color_view,
            depth_texture,
            depth_view,
            output_size: (width, height),
            output_format,
        })
    }
}