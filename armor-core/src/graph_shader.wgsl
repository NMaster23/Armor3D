struct CameraUniform {
    view_proj: mat4x4<f32>,
};
@group(0) @binding(0)
var<uniform> camera: CameraUniform;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) tex_coords: vec2<f32>,
    @location(2) color: vec4<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
    @location(1) world_pos: vec3<f32>,
}

@vertex
fn vs_main(model: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.tex_coords = model.tex_coords;
    out.world_pos = model.position; // <--- MUST ADD THIS LINE
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
    let pos = in.world_pos.xy;
    let main_grid = calculate_grid(pos, 1.0);
    let sub_grid = calculate_grid(pos, 10.0);
    let bg_color = vec4<f32>(0.05, 0.07, 0.1, 0.3);
    let sub_color = vec4<f32>(0.2, 0.4, 0.7, 0.25);
    let main_color = vec4<f32>(0.4, 0.7, 1.0, 0.7);
    var color = mix(bg_color, sub_color, sub_grid);
    color = mix(color, main_color, main_grid);
    let axis_width = fwidth(pos) * 1.5;
    if abs(pos.y) < axis_width.y {
        color = vec4<f32>(1.0, 0.25, 0.25, 0.95); // Red X-axis
    }
    if abs(pos.x) < axis_width.x {
        color = vec4<f32>(0.25, 0.85, 0.25, 0.95); // Green Z-axis
    }
    return color;
}