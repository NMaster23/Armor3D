struct CameraUniform {
    view_proj: mat4x4<f32>,
    camera_position_mode: vec4<f32>,
};

@group(0) @binding(0)
var<uniform> camera: CameraUniform;

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
    if length(in.normal) <= 0.01 {
        return in.color;
    }
    let n = normalize(in.normal);
    let light_dir = normalize(vec3<f32>(0.45, 0.8, 0.35));
    let diffuse = abs(dot(n, light_dir));
    let base = in.color.rgb * (0.35+0.65 * diffuse);
    if camera.camera_position_mode.w < 0.5 {
        return vec4<f32>(base, in.color.a); }
    let view_dir = normalize(camera.camera_position_mode.xyz - in.world_position);
    let fresnel = pow(1.0 - abs(dot(n, view_dir)), 3.0);
    let half_dir = normalize(light_dir + view_dir);
    let highlight = pow(max(dot(n, half_dir), 0.0), 40.0);
    let environment = mix(
        vec3<f32>(0.18, 0.22, 0.30),
        vec3<f32>(0.85, 0.92, 1.0),
        max(n.y, 0.0));
    let glossy = mix(base, environment, fresnel * 0.35);
    return vec4<f32>(glossy + vec3<f32>(highlight * 0.35), in.color.a);
}
