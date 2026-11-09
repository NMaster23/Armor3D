struct CameraUniform {
    view_proj: mat4x4<f32>,
    camera_position_mode: vec4<f32>,
};

@group(0) @binding(0)
var<uniform> camera: CameraUniform;

@group(1) @binding(0)
var material_textures: texture_2d_array<f32>;
@group(1) @binding(1)
var material_sampler: sampler;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) tex_coords: vec3<f32>,
    @location(2) color: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) world_position: vec3<f32>,
};

@vertex
fn vs_main(model: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.color = model.color;
    out.world_position = model.position;
    out.clip_position = camera.view_proj * vec4<f32>(model.position, 1.0);
    out.normal = model.tex_coords;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let has_material = camera.camera_position_mode.w > 0.5 && in.color.a < -0.01;
    if length(in.normal) <= 0.01 && !has_material {
        return vec4<f32>(in.color.rgb, 1.0);
    }
    var n = vec3<f32>(0.0, 1.0, 0.0);
    if length(in.normal) > 0.01 {
        n = normalize(in.normal);
    }
    let light_position = vec3<f32>(0.0, 12.0, 0.0);
    let light_to_surface = in.world_position - light_position;
    let light_distance = length(light_to_surface);
    let light_dir = normalize(light_position - in.world_position);
    let spot_alignment = dot(normalize(light_to_surface), vec3<f32>(0.0, -1.0, 0.0));
    let spot_cone = smoothstep(0.78, 0.91, spot_alignment);
    let attenuation = spot_cone / (1.0 + 0.012 * light_distance * light_distance);
    let diffuse = 0.18 + attenuation * abs(dot(n, light_dir));

    if has_material {
        let layer = i32(round(abs(in.color.a) * 8.0)) - 1;
        let weights = pow(abs(n), vec3<f32>(4.0));
        let blend = weights / max(weights.x + weights.y + weights.z, 0.0001);
        let scale = 0.5;
        let uv_x = in.world_position.zy * scale;
        let uv_y = in.world_position.xz * scale;
        let uv_z = in.world_position.xy * scale;
        let color_x = textureSample(material_textures, material_sampler, uv_x, layer).rgb;
        let color_y = textureSample(material_textures, material_sampler, uv_y, layer).rgb;
        let color_z = textureSample(material_textures, material_sampler, uv_z, layer).rgb;
        let albedo = color_x * blend.x + color_y * blend.y + color_z * blend.z;
        let view_dir = normalize(camera.camera_position_mode.xyz - in.world_position);
        let half_dir = normalize(light_dir + view_dir);
        let specular = pow(max(dot(n, half_dir), 0.0), 48.0) * attenuation * 0.55;
        return vec4<f32>(albedo * diffuse + vec3<f32>(specular), 1.0);
    }

    let base = in.color.rgb * diffuse;
    if camera.camera_position_mode.w < 0.5 {
        return vec4<f32>(base, in.color.a); }
    let view_dir = normalize(camera.camera_position_mode.xyz - in.world_position);
    let fresnel = pow(1.0 - abs(dot(n, view_dir)), 3.0);
    let half_dir = normalize(light_dir + view_dir);
    let highlight = pow(max(dot(n, half_dir), 0.0), 40.0) * attenuation;
    let environment = mix(
        vec3<f32>(0.18, 0.22, 0.30),
        vec3<f32>(0.85, 0.92, 1.0),
        max(n.y, 0.0));
    let glossy = mix(base, environment, fresnel * 0.35);
    return vec4<f32>(glossy + vec3<f32>(highlight * 0.35), in.color.a);
}
