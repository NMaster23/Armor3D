use crate::Vertex;
use wgpu::util::DeviceExt;
const SHADER: &str = r#"
struct Camera { view_proj: mat4x4<f32> };
@group(0) @binding(0) var<uniform> camera: Camera;
@group(1) @binding(0) var picture: texture_2d<f32>;
@group(1) @binding(1) var picture_sampler: sampler;
struct Input {
    @location(0) position: vec3<f32>,
    @location(1) uv: vec3<f32>,
    @location(2) color: vec4<f32>,
};
struct Output {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
};
@vertex fn vs_main(input: Input) -> Output {
    var output: Output;
    output.position = camera.view_proj * vec4<f32>(input.position, 1.0);
    output.uv = input.uv.xy;
    output.color = input.color;
    return output;
}
@fragment fn fs_main(input: Output) -> @location(0) vec4<f32> {
    let pixel = textureSample(picture, picture_sampler, input.uv);
    return vec4<f32>(pixel.rgb * input.color.rgb, pixel.a * input.color.a);
}
"#;
struct Picture {
    bind_group: wgpu::BindGroup,
    vertices: wgpu::Buffer,
    count: u32,
    opacity: f32,
    corners: Option<((f32, f32), (f32, f32))>,
    selected: bool,
}
pub struct ReferenceImageLayer {
    pipeline: wgpu::RenderPipeline,
    texture_layout: wgpu::BindGroupLayout,
    picture: Option<Picture>,
    deleted_picture: Option<Picture>,
    deleted_with_shapes: bool,
    redo_delete_available: bool,
}
impl ReferenceImageLayer {
    fn write_picture_vertices(queue: &wgpu::Queue, picture: &Picture) -> bool {
        let Some((first, second)) = picture.corners else {
            return false;
        };
        let (x1, z1) = first;
        let (x2, z2) = second;
        let tint = if picture.selected {
            [1.0, 0.72, 0.35]
        } else {
            [1.0, 1.0, 1.0]
        };
        let v = |x, z, u, v| Vertex {
            position: [x, -0.002, z],
            coords: [u, v, 0.0],
            color: [tint[0], tint[1], tint[2], picture.opacity],
        };
        let data = [
            v(x1, z1, 0.0, 0.0), v(x2, z1, 1.0, 0.0),
            v(x1, z2, 0.0, 1.0), v(x1, z2, 0.0, 1.0),
            v(x2, z1, 1.0, 0.0), v(x2, z2, 1.0, 1.0),
        ];
        queue.write_buffer(&picture.vertices, 0, bytemuck::cast_slice(&data));
        true
    }

    pub fn new(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        camera_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let texture_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Picture texture layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Picture shader"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Picture pipeline layout"),
            bind_group_layouts: &[Some(camera_layout), Some(&texture_layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Picture pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[Some(Vertex::desc())],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: Default::default(),
            multiview_mask: None,
            cache: None,
        });
        Self {
            pipeline,
            texture_layout,
            picture: None,
            deleted_picture: None,
            deleted_with_shapes: false,
            redo_delete_available: false,
        }
    }
    pub fn load(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        path: &str,
        opacity: f32,
    ) -> anyhow::Result<()> {
        let rgba = image::open(path)?.to_rgba8();
        let (width, height) = rgba.dimensions();
        anyhow::ensure!(width > 0 && height > 0, "Image is empty");
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Picture texture"),
            size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            wgpu::TexelCopyTextureInfoBase {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            rgba.as_raw(),
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(width * 4),
                rows_per_image: Some(height),
            },
            wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
        );
        let view = texture.create_view(&Default::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Picture bind group"),
            layout: &self.texture_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&view) },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&sampler) },
            ],
        });
        let data = [Vertex {
            position: [0.0; 3],
            coords: [0.0; 3],
            color: [1.0, 1.0, 1.0, 0.0],
        }; 6];
        let vertices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Picture vertices"),
            contents: bytemuck::cast_slice(&data),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        });
        self.picture = Some(Picture {
            bind_group,
            vertices,
            count: data.len() as u32,
            opacity: opacity.clamp(0.0, 1.0),
            corners: None,
            selected: false,
        });
        self.deleted_picture = None;
        self.deleted_with_shapes = false;
        self.redo_delete_available = false;
        Ok(())
    }
    pub fn set_corners(
        &mut self,
        queue: &wgpu::Queue,
        first: (f32, f32),
        second: (f32, f32),
    ) -> bool {
        let Some(picture) = &mut self.picture else {
            return false;
        };
        picture.corners = Some((first, second));
        Self::write_picture_vertices(queue, picture)
    }
    pub fn clear_selection(&mut self, queue: &wgpu::Queue) {
        if let Some(picture) = &mut self.picture {
            if picture.selected {
                picture.selected = false;
                Self::write_picture_vertices(queue, picture);
            }
        }
    }
    pub fn select_at(&mut self, queue: &wgpu::Queue, point: (f32, f32), additive: bool) -> bool {
        let Some(picture) = &mut self.picture else {
            return false;
        };
        let Some((first, second)) = picture.corners else {
            return false;
        };
        let min_x = first.0.min(second.0);
        let max_x = first.0.max(second.0);
        let min_z = first.1.min(second.1);
        let max_z = first.1.max(second.1);
        let hit = point.0 >= min_x
            && point.0 <= max_x
            && point.1 >= min_z
            && point.1 <= max_z;
        let selected = if additive {
            picture.selected || hit
        } else {
            hit
        };
        if selected != picture.selected {
            picture.selected = selected;
            Self::write_picture_vertices(queue, picture);
        }
        hit
    }
    pub fn select_box(
        &mut self,
        queue: &wgpu::Queue,
        first: (f32, f32),
        second: (f32, f32),
        additive: bool,
    ) -> bool {
        let Some(picture) = &mut self.picture else {
            return false;
        };
        let Some((image_first, image_second)) = picture.corners else {
            return false;
        };
        let box_min_x = first.0.min(second.0);
        let box_max_x  = first.0.max(second.0);
        let box_min_z = first.1.min(second.1);
        let box_max_z = first.1.max(second.1);
        let image_min_x = image_first.0.min(image_second.0);
        let image_max_x = image_first.0.max(image_second.0);
        let image_min_z = image_first.1.min(image_second.1);
        let image_max_z = image_first.1.max(image_second.1);
        let hit = image_max_x >= box_min_x
            && image_min_x <= box_max_x
            && image_max_z >= box_min_z
            && image_min_z <= box_max_z;
        let selected = if additive {
            picture.selected || hit
        } else {
            hit
        };
        if selected != picture.selected {
            picture.selected = selected;
            Self::write_picture_vertices(queue, picture);
        }
        hit
    }
    pub fn delete_selected(&mut self, deleted_with_shapes: bool) -> bool {
        let selected = self
            .picture
            .as_ref()
            .map(|picture| picture.selected)
            .unwrap_or(false);
        if selected {
            self.deleted_picture = self.picture.take();
            self.deleted_with_shapes = deleted_with_shapes;
            self.redo_delete_available = false;
        } else if deleted_with_shapes {
            self.deleted_picture = None;
            self.deleted_with_shapes = false;
            self.redo_delete_available = false;
        }
        selected
    }
    pub fn undo_delete(&mut self) -> Option<bool> {
        if self.picture.is_some() {
            return None;
        }
        let picture = self.deleted_picture.take()?;
        let deleted_with_shapes = self.deleted_with_shapes;
        self.picture = Some(picture);
        self.redo_delete_available = true;
        Some(deleted_with_shapes)
    }
    pub fn redo_delete(&mut self) -> Option<bool> {
        if !self.redo_delete_available {
            return None;
        }
        let picture = self.picture.take()?;
        let deleted_with_shapes = self.deleted_with_shapes;
        self.deleted_picture = Some(picture);
        self.redo_delete_available = false;
        Some(deleted_with_shapes)
    }
    pub fn clear(&mut self) {
        self.picture = None;
        self.deleted_picture = None;
        self.deleted_with_shapes = false;
        self.redo_delete_available = false;
    }
    pub fn draw<'a>(&'a self, pass: &mut wgpu::RenderPass<'a>) {
        if let Some(picture) = &self.picture {
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(1, &picture.bind_group, &[]);
            pass.set_vertex_buffer(0, picture.vertices.slice(..));
            pass.draw(0..picture.count, 0..1);
        }
    }
}
