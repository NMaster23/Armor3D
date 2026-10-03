struct Point {
    position: vec2<f32>,
};

struct Uniforms {
    subdivisions: u32,
    total_points: u32,
};

@group(0) @binding(0) var<storage, read> points: array<Point>;
@group(0) @binding(1) var<uniform> config: Uniforms;

fn rom(p0: vec2<f32>, p1: vec2<f32>, p2: vec2<f32>, p3: vec2<f32>, t: f32) -> vec2<f32> {
    let t2 = t * t;
    let t3 = t2 * t;
    return 0.5 * (
        (2.0 * p1) +
        (-p0 + p2) * t +
        (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3) * t2 +
        (-p0 + 3.0 * p1 - 3.0 * p2 + p3) * t3
    );
}

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> @builtin(position) vec4<f32> {
    let subs = config.subdivisions;
    let segment_idx = vertex_index / subs;
    let t = f32(vertex_index % subs) / f32(subs);
    let num_points = config.total_points;
    let i0 = select(segment_idx - 1u, 0u, segment_idx == 0u);
    let i1 = segment_idx;
    let i2 = min(segment_idx + 1u, num_points - 1u);
    let i3 = min(segment_idx + 2u, num_points - 1u);
    let p0 = points[i0].position;
    let p1 = points[i1].position;
    let p2 = points[i2].position;
    let p3 = points[i3].position;
    let curve_pos = rom(p0, p1, p2, p3, t);
    return vec4<f32>(curve_pos, 0.0, 1.0);
}

@fragment
fn fs_main() -> @location(0) vec4<f32> {
    return vec4<f32>(1.0, 1.0, 1.0, 1.0);
}