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
    @location(1) world_pos: vec3<f32>,
};
@vertex
fn vs_main(model: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.world_pos = model.position;
    out.clip_position = camera.view_proj * vec4<f32>(model.position, 1.0);
    return out;
}
fn calculate_grid(pos: vec2<f32>, scale: f32) -> f32 {
    let coord = pos * scale;
    let derivative = fwidth(coord);
    let grid_lines = abs(fract(coord - 0.5) - 0.5) / derivative;
    let line = min(grid_lines.x, grid_lines.y);
    return 1.0 - min(line, 1.0);
}
@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    if camera.camera_position_mode.w > 0.5 {
        let uv = in.world_pos.xz * 0.5;
        let fabric = textureSample(material_textures, material_sampler, uv, 8).rgb;
        let light_position = vec3<f32>(0.0, 12.0, 0.0);
        let floor_position = in.world_pos;
        let light_to_floor = floor_position - light_position;
        let light_distance = length(light_to_floor);
        let light_dir = normalize(light_position - floor_position);
        let spot_alignment = dot(normalize(light_to_floor), vec3<f32>(0.0, -1.0, 0.0));
        let spot_cone = smoothstep(0.78, 0.91, spot_alignment);
        let attenuation = spot_cone / (1.0 + 0.012 * light_distance * light_distance);
        let floor_light = 0.08 + attenuation * max(dot(vec3<f32>(0.0, 1.0, 0.0), light_dir), 0.0) * 1.8;
        return vec4<f32>(fabric * floor_light, 1.0); }
    let pos = select(in.world_pos.xy, in.world_pos.xz, abs(in.world_pos.y) < 0.001);
    let main_grid = calculate_grid(pos, 1.0);
    let sub_grid = calculate_grid(pos, 10.0);
    let bg_color = vec4<f32>(0.025, 0.025, 0.025, 0.3);
    let sub_color = vec4<f32>(0.20, 0.20, 0.20, 0.35);
    let main_color = vec4<f32>(0.38, 0.38, 0.38, 0.65);
    var color = mix(bg_color, sub_color, sub_grid);
    color = mix(color, main_color, main_grid);
    let axis_width = fwidth(pos) * 1.5;
    if abs(pos.y) < axis_width.y {
        color = vec4<f32>(0.72, 0.72, 0.72, 0.9);}
    if abs(pos.x) < axis_width.x {
        color = vec4<f32>(0.95, 0.95, 0.95, 0.95); }
    return color;
}
