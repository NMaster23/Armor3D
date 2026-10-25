use cgmath::{InnerSpace, Point3, Vector3};
use crate::camera::Ray;

use crate::Vertex;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GumballHandle {
    translate_x,
    translate_y,
    translate_z,
    translate_xy,
    translate_xz,
    translate_yz,
    rotate_x,
    rotate_y,
    rotate_z,
}

pub struct Gumball {
    pub origin: Point3<f32>,
    pub size: f32,
    pub active: Option<GumballHandle>,
    pub drag_start_origin: Point3<f32>,
    pub drag_start_point: Option<Point3<f32>>,
}

impl Gumball {
    pub fn new(origin: Point3<f32>, size: f32) -> Self {
        Self {
            origin,
            size,
            active: None,
            drag_start_origin: origin,
            drag_start_point: None,
        }
    }
    pub fn build_vertices(&self) -> Vec<Vertex> {
        let mut verts = Vec::new();
        let mut add_three_p = |p0: Point3<f32>, p1: Point3<f32>, p2: Point3<f32>, normal: [f32; 3], color: [f32; 4]| {
            verts.push(Vertex {
                position: [p0.x, p0.y, p0.z],
                coords: normal,
                color,
            });
            verts.push(Vertex {
                position: [p1.x, p1.y, p1.z],
                coords: normal,
                color,
            });
            verts.push(Vertex {
                position: [p2.x, p2.y, p2.z],
                coords: normal,
                color,
            });
        };
        let arrow_r = self.size * 0.05;
        let shaft_r = self.size * 0.015;
        let red = [0.9, 0.2, 0.2, 1.0];
        let green = [0.2, 0.8, 0.2, 1.0];
        let blue = [0.2, 0.4, 0.9, 1.0];
        let tiles = [0.9, 0.8, 0.2, 0.8];
        let axes = [
            (Vector3::unit_x(), Vector3::unit_y(), Vector3::unit_z(), red, GumballHandle::translate_x),
            (Vector3::unit_y(), Vector3::unit_z(), Vector3::unit_x(), green, GumballHandle::translate_y),
            (Vector3::unit_z(), Vector3::unit_x(), Vector3::unit_y(), blue, GumballHandle::translate_z),
        ];
        for (dir, u, v, color, _handle) in axes {
            let shaft_end = self.origin + dir * (self.size * 0.85);
            let tip = self.origin + dir * self.size;
            let p0 = shaft_end + (u + v) * arrow_r;
            let p1 = shaft_end + (-u + v) * arrow_r;
            let p2 = shaft_end + (-u - v) * arrow_r;
            let p3 = shaft_end + (u - v) * arrow_r;
            add_three_p(p0, p1, tip, [dir.x, dir.y, dir.z], color);
            add_three_p(p1, p2, tip, [dir.x, dir.y, dir.z], color);
            add_three_p(p2, p3, tip, [dir.x, dir.y, dir.z], color);
            add_three_p(p3, p0, tip, [dir.x, dir.y, dir.z], color);
            let c0 = self.origin + u * shaft_r;
            let c1 = shaft_end + u * shaft_r;
            let c2 = shaft_end - u * shaft_r;
            let c3 = self.origin - u *shaft_r;
            add_three_p(c0, c1, c2, [v.x, v.y, v.z], color);
            add_three_p(c0, c2, c3, [v.x, v.y, v.z], color);
        }
        let arc_r = self.size * 0.6;
        let arc_thick = self.size * 0.015;
        let seg = 16;
        let half_pi = std::f32::consts::FRAC_PI_2;
        for i in 0..seg {
            let t0 = (i as f32 / seg as f32) * half_pi;
            let t1 = ((i + 1) as f32 / seg as f32) * half_pi;
            let (sin0, cos0) = (t0.sin(), t0.cos());
            let (sin1, cos1) = (t1.sin(), t1.cos());
            let g_in0 = self.origin + Vector3::new((arc_r - arc_thick) * cos0, 0.0, (arc_r - arc_thick) * sin0);
            let g_out0 = self.origin + Vector3::new((arc_r + arc_thick) * cos0, 0.0, (arc_r + arc_thick) * sin0);
            let g_in1 = self.origin + Vector3::new((arc_r - arc_thick) * cos1, 0.0, (arc_r - arc_thick) * sin1);
            let g_out1 = self.origin + Vector3::new((arc_r + arc_thick) * cos1, 0.0, (arc_r + arc_thick) * sin1);
            add_three_p(g_in0, g_out0, g_out1, [0.0, 1.0, 0.0], green);
            add_three_p(g_in0, g_out1, g_in1, [0.0, 1.0, 0.0], green);
            let r_in0 = self.origin + Vector3::new(0.0, (arc_r - arc_thick) * cos0, (arc_r - arc_thick) * sin0);
            let r_out0 = self.origin + Vector3::new(0.0, (arc_r + arc_thick) * cos0, (arc_r + arc_thick) * sin0);
            let r_in1 = self.origin + Vector3::new(0.0, (arc_r - arc_thick) * cos1, (arc_r - arc_thick) * sin1);
            let r_out1 = self.origin + Vector3::new(0.0, (arc_r + arc_thick) * cos1, (arc_r + arc_thick) * sin1);
            add_three_p(r_in0, r_out0, r_out1, [1.0, 0.0, 0.0], red);
            add_three_p(r_in0, r_out1, r_in1, [1.0, 0.0, 0.0], red);
            let b_in0 = self.origin + Vector3::new((arc_r - arc_thick) * cos0, (arc_r - arc_thick) * sin0, 0.0);
            let b_out0 = self.origin + Vector3::new((arc_r + arc_thick) * cos0, (arc_r + arc_thick) * sin0, 0.0);
            let b_in1 = self.origin + Vector3::new((arc_r - arc_thick) * cos1, (arc_r - arc_thick) * sin1, 0.0);
            let b_out1 = self.origin + Vector3::new((arc_r + arc_thick) * cos1, (arc_r + arc_thick) * sin1, 0.0);
            add_three_p(b_in0, b_out0, b_out1, [0.0, 0.0, 1.0], blue);
            add_three_p(b_in0, b_out1, b_in1, [0.0, 0.0, 1.0], blue);
        }
        verts
    }
    pub fn hit_test(&self, ray: &Ray) -> Option<GumballHandle> {
        let mut closest_hit: Option<(f32, GumballHandle)> = None;
        let mut test_three_p = |p0: Point3<f32>, p1: Point3<f32>, p2: Point3<f32>, handle: GumballHandle| {
            if let Some(t) = ray.intersect_triangle(p0, p1, p2) {
                if closest_hit.map_or(true, |(cur_t, _)| t < cur_t) {
                    closest_hit = Some((t, handle))
                }
            }
        };
        let arrow_r = self.size * 0.05;
        let shaft_r = self.size * 0.015;
        let axes = [
            (Vector3::unit_x(), Vector3::unit_y(), Vector3::unit_z(), GumballHandle::translate_x),
            (Vector3::unit_y(), Vector3::unit_z(), Vector3::unit_x(), GumballHandle::translate_y),
            (Vector3::unit_z(), Vector3::unit_x(), Vector3::unit_y(), GumballHandle::translate_z),
        ];
        for (dir, u, v, handle) in axes {
            let shaft_end = self.origin + dir * (self.size * 0.85);
            let tip = self.origin + dir * self.size;
            let p0 = shaft_end + (u + v) * arrow_r;
            let p1 = shaft_end + (-u + v) * arrow_r;
            let p2 = shaft_end + (-u - v) * arrow_r;
            let p3 = shaft_end + (u - v) * arrow_r;
            test_three_p(p0, p1, tip, handle);
            test_three_p(p1, p2, tip, handle);
            test_three_p(p2, p3, tip, handle);
            test_three_p(p3, p0, tip, handle);
            let c0 = self.origin + u * shaft_r;
            let c1 = shaft_end + u * shaft_r;
            let c2 = shaft_end - u * shaft_r;
            let c3 = self.origin - u * shaft_r;
            test_three_p(c0, c1, c2, handle);
            test_three_p(c0, c2, c3, handle);
        }
        let rings: [(Vector3<f32>, Vector3<f32>, GumballHandle); 3] = [
            (Vector3::unit_x(), Vector3::unit_z(), GumballHandle::rotate_x),
            (Vector3::unit_y(), Vector3::unit_z(), GumballHandle::rotate_y),
            (Vector3::unit_z(), Vector3::unit_y(), GumballHandle::rotate_z),
        ];
        let arc_r = self.size * 0.6;
        let arc_thick = self.size * 0.02;
        let seg = 16;
        let half_pi = std::f32::consts::FRAC_PI_2;
        for (u, v, handle) in rings {
            for i in 0..seg {
                let t0 = (i as f32 / seg as f32) * half_pi;
                let t1 = ((i + 1) as f32 / seg as f32) * half_pi;
                let (s0, c0) = (t0.sin(), t0.cos());
                let (s1, c1) = (t1.sin(), t1.cos());
                let p0 = self.origin + (u * c0 + v * s0) * (arc_r - arc_thick);
                let p1 = self.origin + (u * c0 + v * s0) * (arc_r + arc_thick);
                let p2 = self.origin + (u * c1 + v * s1) * (arc_r + arc_thick);
                let p3 = self.origin + (u * c1 + v * s1) * (arc_r - arc_thick);
                test_three_p(p0, p1, p2, handle);
                test_three_p(p0, p2, p3, handle);
            }
        }
        closest_hit.map(|(_, handle)| handle)
    }
    pub fn compute_drag_delta(
        &self,
        handle: GumballHandle,
        ray: &Ray,
        camera_dir: Vector3<f32>
    ) -> Option<Vector3<f32>> {
        let start_pt = self.drag_start_point?;
        let axis = match handle {
            GumballHandle::translate_x => Vector3::new(1.0, 0.0, 0.0),
            GumballHandle::translate_y => Vector3::new(0.0, 1.0, 0.0),
            GumballHandle::translate_z => Vector3::new(0.0, 0.0, 1.0),
            _ => return None,
        };
        let plane_normal = (camera_dir - axis * camera_dir.dot(axis)).normalize();
        let denom = ray.direction.dot(plane_normal);
        if denom.abs() < 1e-6 {
            return None;
        }
        let t = (self.drag_start_origin - ray.origin).dot(plane_normal) / denom;
        if t < 0.0 {
            return None;
        }
        let current_pt = ray.origin + ray.direction * t;
        let total_displacement = current_pt - start_pt;
        let proj_dist = total_displacement.dot(axis);
        Some(axis * proj_dist)
    }
}
