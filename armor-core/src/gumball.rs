use cgmath::{Point3, Vector3};
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
        let red = [0.9, 0.2, 0.2, 1.0];
        let green = [0.2, 0.8, 0.2, 1.0];
        let blue = [0.2, 0.4, 0.9, 1.0];
        let tiles = [0.9, 0.8, 0.2, 0.8];
        let axis_x = self.origin + Vector3::new(self.size * 0.85, 0.0, 0.0);
        let axis_y = self.origin + Vector3::new(0.0, self.size * 0.85, 0.0);
        let axis_z = self.origin + Vector3::new(0.0, 0.0, self.size * 0.85);
        let tip_x = self.origin + Vector3::new(self.size, 0.0, 0.0);
        let x0 = axis_x + Vector3::new(0.0, arrow_r, arrow_r);
        let x1 = axis_x + Vector3::new(0.0, -arrow_r, arrow_r);
        let x2 = axis_x + Vector3::new(0.0, -arrow_r, -arrow_r);
        let x3 = axis_x + Vector3::new(0.0, arrow_r, -arrow_r);
        let shaft_r = self.size * 0.015;
        let c_x_a = self.origin + Vector3::new(0.0, shaft_r, 0.0);
        let c_x_b = axis_x + Vector3::new(0.0, shaft_r, 0.0);
        let c_x_c = axis_x + Vector3::new(0.0, -shaft_r, 0.0);
        let c_x_d = self.origin + Vector3::new(0.0, -shaft_r, 0.0);
        let tip_y = self.origin + Vector3::new(0.0, self.size, 0.0);
        let y0 = axis_y + Vector3::new(arrow_r, 0.0, arrow_r);
        let y1 = axis_y + Vector3::new(-arrow_r, 0.0, arrow_r);
        let y2 = axis_y + Vector3::new(-arrow_r, 0.0, -arrow_r);
        let y3 = axis_y + Vector3::new(arrow_r, 0.0, -arrow_r);
        let c_y_a = self.origin + Vector3::new(shaft_r, 0.0, 0.0);
        let c_y_b = axis_y + Vector3::new(shaft_r, 0.0, 0.0);
        let c_y_c = axis_y + Vector3::new(-shaft_r, 0.0, 0.0);
        let c_y_d = self.origin + Vector3::new(-shaft_r, 0.0, 0.0);
        let tip_z = self.origin + Vector3::new(0.0, 0.0, self.size);
        let z0 = axis_z + Vector3::new(arrow_r, arrow_r, 0.0);
        let z1 = axis_z + Vector3::new(-arrow_r,  arrow_r, 0.0);
        let z2 = axis_z + Vector3::new(-arrow_r, -arrow_r, 0.0);
        let z3 = axis_z + Vector3::new(arrow_r,  -arrow_r, 0.0);
        let c_z_a = self.origin + Vector3::new(shaft_r, 0.0, 0.0);
        let c_z_b = axis_z + Vector3::new(shaft_r, 0.0, 0.0);
        let c_z_c = axis_z + Vector3::new(-shaft_r, 0.0, 0.0);
        let c_z_d = self.origin + Vector3::new(-shaft_r, 0.0, 0.0);
        add_three_p(x0, x1, tip_x, [1.0, 0.0, 0.0], red);
        add_three_p(x1, x2, tip_x, [1.0, 0.0, 0.0], red);
        add_three_p(x2, x3, tip_x, [1.0, 0.0, 0.0], red);
        add_three_p(x3, x0, tip_x, [1.0, 0.0, 0.0], red);
        add_three_p(c_x_a, c_x_b, c_x_c, [0.0, 0.0, 1.0], red);
        add_three_p(c_x_a, c_x_c, c_x_d, [0.0, 0.0, 1.0], red);
        add_three_p(y0, y1, tip_y, [0.0, 1.0, 0.0], green);
        add_three_p(y1, y2, tip_y, [0.0, 1.0, 0.0], green);
        add_three_p(y2, y3, tip_y, [0.0, 1.0, 0.0], green);
        add_three_p(y3, y0, tip_y, [0.0, 1.0, 0.0], green);
        add_three_p(c_y_a, c_y_b, c_y_c, [0.0, 0.0, 1.0], green);
        add_three_p(c_y_a, c_y_c, c_y_d, [0.0, 0.0, 1.0], green);
        add_three_p(z0, z1, tip_z, [0.0, 0.0, 1.0], blue);
        add_three_p(z1, z2, tip_z, [0.0, 0.0, 1.0], blue);
        add_three_p(z2, z3, tip_z, [0.0, 0.0, 1.0], blue);
        add_three_p(z3, z0, tip_z, [0.0, 0.0, 1.0], blue);
        add_three_p(c_z_a, c_z_b, c_z_c, [0.0, 0.0, 1.0], blue);
        add_three_p(c_z_a, c_z_c, c_z_d, [0.0, 0.0, 1.0], blue);
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
        let axis_x = self.origin + Vector3::new(self.size * 0.85, 0.0, 0.0);
        let axis_y = self.origin + Vector3::new(0.0, self.size * 0.85, 0.0);
        let axis_z = self.origin + Vector3::new(0.0, 0.0, self.size * 0.85);
        let tip_x = self.origin + Vector3::new(self.size, 0.0, 0.0);
        let x0 = axis_x + Vector3::new(0.0, arrow_r, arrow_r);
        let x1 = axis_x + Vector3::new(0.0, -arrow_r, arrow_r);
        let x2 = axis_x + Vector3::new(0.0, -arrow_r, -arrow_r);
        let x3 = axis_x + Vector3::new(0.0, arrow_r, -arrow_r);
        let shaft_r = self.size * 0.015;
        let c_x_a = self.origin + Vector3::new(0.0, shaft_r, 0.0);
        let c_x_b = axis_x + Vector3::new(0.0, shaft_r, 0.0);
        let c_x_c = axis_x + Vector3::new(0.0, -shaft_r, 0.0);
        let c_x_d = self.origin + Vector3::new(0.0, -shaft_r, 0.0);
        let tip_y = self.origin + Vector3::new(0.0, self.size, 0.0);
        let y0 = axis_y + Vector3::new(arrow_r, 0.0, arrow_r);
        let y1 = axis_y + Vector3::new(-arrow_r, 0.0, arrow_r);
        let y2 = axis_y + Vector3::new(-arrow_r, 0.0, -arrow_r);
        let y3 = axis_y + Vector3::new(arrow_r, 0.0, -arrow_r);
        let c_y_a = self.origin + Vector3::new(shaft_r, 0.0, 0.0);
        let c_y_b = axis_y + Vector3::new(shaft_r, 0.0, 0.0);
        let c_y_c = axis_y + Vector3::new(-shaft_r, 0.0, 0.0);
        let c_y_d = self.origin + Vector3::new(-shaft_r, 0.0, 0.0);
        let tip_z = self.origin + Vector3::new(0.0, 0.0, self.size);
        let z0 = axis_z + Vector3::new(arrow_r, arrow_r, 0.0);
        let z1 = axis_z + Vector3::new(-arrow_r,  arrow_r, 0.0);
        let z2 = axis_z + Vector3::new(-arrow_r, -arrow_r, 0.0);
        let z3 = axis_z + Vector3::new(arrow_r,  -arrow_r, 0.0);
        let c_z_a = self.origin + Vector3::new(shaft_r, 0.0, 0.0);
        let c_z_b = axis_z + Vector3::new(shaft_r, 0.0, 0.0);
        let c_z_c = axis_z + Vector3::new(-shaft_r, 0.0, 0.0);
        let c_z_d = self.origin + Vector3::new(-shaft_r, 0.0, 0.0);
        test_three_p(x0, x1, tip_x, GumballHandle::translate_x);
        test_three_p(x1, x2, tip_x, GumballHandle::translate_x);
        test_three_p(x2, x3, tip_x, GumballHandle::translate_x);
        test_three_p(x3, x0, tip_x, GumballHandle::translate_x);
        test_three_p(c_x_a, c_x_b, c_x_c, GumballHandle::translate_x);
        test_three_p(c_x_a, c_x_c, c_x_d, GumballHandle::translate_x);
        test_three_p(y0, y1, tip_y, GumballHandle::translate_y);
        test_three_p(y1, y2, tip_y, GumballHandle::translate_y);
        test_three_p(y2, y3, tip_y, GumballHandle::translate_y);
        test_three_p(y3, y0, tip_y, GumballHandle::translate_y);
        test_three_p(c_y_a, c_y_b, c_y_c, GumballHandle::translate_y);
        test_three_p(c_y_a, c_y_c, c_y_d, GumballHandle::translate_y);
        test_three_p(z0, z1, tip_z, GumballHandle::translate_z);
        test_three_p(z1, z2, tip_z, GumballHandle::translate_z);
        test_three_p(z2, z3, tip_z, GumballHandle::translate_z);
        test_three_p(z3, z0, tip_z, GumballHandle::translate_z);
        test_three_p(c_z_a, c_z_b, c_z_c, GumballHandle::translate_z);
        test_three_p(c_z_a, c_z_c, c_z_d, GumballHandle::translate_z);
        closest_hit.map(|(_, handle)| handle)
    }
    pub fn compute_drag_delta(&self, handle: GumballHandle, ray: &Ray) -> Option<Vector3<f32>> {
        let normal = Vector3::new(0.0, 0.0, 0.0);
        let translate_x = self.origin + Vector3::new(0.0, 0.0, 1.0);
        let translate_z = self.origin + Vector3::new(0.0, 1.0, 0.0);
        let t = (self.origin - ray.origin) / ray.direction;
        let p = ray.origin + (t * ray.direction);
    }
}