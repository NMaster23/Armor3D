use std::f32::consts::PI;

use crate::{Vertex, exporter::Triangle};


#[derive(Clone, Debug)]
pub enum BaseShape {
    Box { width: f32, height: f32, depth: f32 },
    Cylinder { radius: f32, height: f32, subdivisions: usize },
    Sphere { radius: f32, rings: usize, sectors: usize },
    Cone { radius: f32, height: f32, subdivisions: usize },
    Torus { outer_radius: f32, inner_radius: f32, outer_subdivisions: usize, inner_subdivisions: usize },
}

#[derive(Clone, Debug)]
pub struct SolidEntity {
    pub id: usize,
    pub origin: [f32; 3],
    pub shape: BaseShape,
    pub color: [f32; 4],
    pub selected: bool,
}

impl SolidEntity {
    pub fn to_verts(&self, color: [f32; 4]) -> Vec<Vertex> {
        let tris = self.to_tris();
        let mut verts = Vec::with_capacity(tris.len() * 3);
        for tri in tris {
            verts.push(Vertex { position: tri.v0, coords: tri.normal, color });
            verts.push(Vertex { position: tri.v1, coords: tri.normal, color });
            verts.push(Vertex { position: tri.v2, coords: tri.normal, color });
        }
        verts
    }
    pub fn to_tris(&self) -> Vec<Triangle> {
        match &self.shape {
            BaseShape::Box { width, height, depth } => {
                Self::create_box(self.origin, *width, *height, *depth)
            }
            BaseShape::Cylinder { radius, height, subdivisions } => {
                Self::create_cylinder(self.origin, *radius, *height, *subdivisions)
            }
            BaseShape::Sphere { radius, rings, sectors } => {
                Self::create_sphere(self.origin, *radius, *rings, *sectors)
            }
            BaseShape::Cone { radius, height, subdivisions } => {
                Self::create_cone(self.origin, *radius, *height, *subdivisions)
            }
            BaseShape::Torus { outer_radius, inner_radius, outer_subdivisions, inner_subdivisions } => {
                Self::create_torus(self.origin, *outer_radius, *inner_radius, *outer_subdivisions, *inner_subdivisions)
            }
        }
    }
    pub fn create_box(origin: [f32; 3], w: f32, h: f32, d: f32) -> Vec<Triangle> {
        let x0 = origin[0];
        let x1 = origin[0] + w;
        let y0 = origin[1];
        let y1 = origin[1] + h;
        let z0 = origin[2];
        let z1 = origin[2] + d;
        let mut t = Vec::with_capacity(12);
        let quad = |v0, v1, v2, v3, normal: [f32; 3], list: &mut Vec<Triangle>| {
            list.push(Triangle {
                v0,
                v1,
                v2,
                normal,
            });
            list.push(Triangle {
                v0,
                v1,
                v2: v3,
                normal,
            });
        };
        quad([x0, y0, z1], [x1, y0, z1], [x1, y1, z1], [x0, y1, z1], [0.0, 0.0, 1.0], &mut t);
        quad([x1, y0, z0], [x0, y0, z0], [x0, y1, z0], [x1, y1, z0], [0.0, 0.0, -1.0], &mut t);
        quad([x1, y0, z1], [x1, y0, z0], [x1, y1, z0], [x1, y1, z1], [1.0, 0.0, 0.0], &mut t);
        quad([x0, y0, z0], [x0, y0, z1], [x0, y1, z1], [x0, y1, z0], [-1.0, 0.0, 0.0], &mut t);
        quad([x0, y1, z1], [x1, y1, z1], [x1, y1, z0], [x0, y1, z0], [0.0, 1.0, 0.0], &mut t);
        quad([x0, y0, z0], [x1, y0, z0], [x1, y0, z1], [x0, y0, z1], [0.0, -1.0, 0.0], &mut t);
        t
    }
    fn create_cylinder(origin: [f32; 3], r: f32, h: f32, subs: usize) -> Vec<Triangle> {
        let n = subs.max(8);
        let mut t = Vec::new();
        for i in 0..n {
            let theta0 = 2.0 * PI * (i as f32) / (n as f32);
            let theta1 = 2.0 * PI * ((i + 1) as f32) / (n as f32);
            let c0 = theta0.cos();
            let s0 = theta0.sin();
            let c1 = theta1.cos();
            let s1 =theta1.sin();
            let b0 = [origin[0] + r * c0, origin[1], origin[2] + r * s0];
            let b1 = [origin[0] + r * c1, origin[1], origin[2] + r * s1];
            let t0 = [origin[0] + r * c0, origin[1] + h, origin[2] + r * s0];
            let t1 = [origin[0] + r * c1, origin[1] + h, origin[2] + r * s1];
            let wall_norm = [(c0 + c1) * 0.5, 0.0, (s0 + s1) * 0.5];
            t.push(Triangle {
                v0: b0,
                v1: b1,
                v2: t1,
                normal: wall_norm,
            });
            t.push(Triangle {
                v0: b0,
                v1: t1,
                v2: t0,
                normal: wall_norm,
            });
            t.push(Triangle {
                v0: [origin[0], origin[1], origin[2]],
                v1: b1,
                v2: b0,
                normal: [0.0, -1.0, 0.0],
            });
            t.push(Triangle {
                v0: [origin[0], origin[1] + h, origin[2]],
                v1: t0,
                v2: t1,
                normal: [0.0, 1.0, 0.0],
            });
        }
        t
    }
    fn create_sphere(origin: [f32; 3], r: f32, rings: usize, sectors: usize) -> Vec<Triangle> {
        let n_rings = rings.max(6);
        let n_sect = sectors.max(8);
        let mut t = Vec::new();
        for i in 0..n_rings {
            let phi0 = PI * (i as f32) / (n_rings as f32);
            let phi1 = PI * ((i + 1) as f32) / (n_rings as f32);
            for j in 0..n_sect {
                let theta0 = 2.0 * PI * (j as f32) / (n_sect as f32);
                let theta1 = 2.0 * PI * ((j + 1) as f32) / (n_sect as f32);
                let p = |phi: f32, theta: f32| -> ([f32; 3], [f32; 3]) {
                    let nx = phi.sin() * theta.cos();
                    let ny = phi.cos();
                    let nz = phi.sin() * theta.sin();
                    ([origin[0] + r * nx, origin[1] + r * ny, origin[2] + r * nz], [nx , ny, nz])
                };
                let (p00, n00) = p(phi0, theta0);
                let (p10, _) = p(phi1, theta0);
                let (p11, _) = p(phi1, theta1);
                let (p01, _) = p(phi0, theta1);
                if i != 0 {
                    t.push(Triangle {
                        v0: p00,
                        v1: p10,
                        v2: p11,
                        normal: n00
                    });
                }
                if i != n_rings - 1 {
                    t.push(Triangle {
                        v0: p00,
                        v1: p11,
                        v2: p01,
                        normal: n00
                    });
                }
            }
        }
        t
    }
    fn create_cone(origin: [f32; 3], r: f32, h: f32, subs: usize) -> Vec<Triangle> {
        let n = subs.max(8);
        let apex = [origin[0], origin[1] + h, origin[2]];
        let mut t = Vec::new();
        for i in 0..n {
            let theta0 = 2.0 * PI * (i as f32) / (n as f32);
            let theta1 = 2.0 * PI * ((i + 1) as f32) / (n as f32);
            let b0 = [origin[0] + r * theta0.cos(), origin[1], origin[2] + r * theta0.sin()];
            let b1 = [origin[0] + r * theta1.cos(), origin[1], origin[2] + r * theta1.sin()];
            let mid_theta = (theta0 + theta1) * 0.5;
            let normal = [mid_theta.cos() * h / (r*r + h*h).sqrt(), r / (r*r + h*h).sqrt(), mid_theta.sin() * h / (r*r + h*h).sqrt()];
            t.push(Triangle {
                v0: b0,
                v1: b1,
                v2: apex,
                normal,
            });
            t.push(Triangle {
                v0: origin,
                v1: b1,
                v2: b0,
                normal: [0.0, -1.0, 0.0],
            });
        }
        t
    }
    fn create_torus(origin: [f32; 3], outer_r: f32, inner_r: f32, outer_sub: usize, inner_sub: usize) -> Vec<Triangle> {
        let m_subs = outer_sub.max(8);
        let t_subs = inner_sub.max(6);
        let mut t = Vec::new();
        let pt = |u: f32, v: f32| -> ([f32; 3], [f32; 3]) {
            let cu = u.cos();
            let su = u.sin();
            let cv = v.cos();
            let sv = v.sin();
            let x = (outer_r + inner_r * cv) * cu;
            let y = inner_r * sv;
            let z = (outer_r + inner_r * cv) * su;
            let nx = cv * cu;
            let ny = sv;
            let nz = cv * su;
            ([origin[0] + x, origin[1] + y, origin[2] + z], [nx, ny, nz])
        };
        for i in 0..m_subs {
            let u0 = 2.0 * PI * (i as f32) / (m_subs as f32);
            let u1 = 2.0 * PI * ((i + 1) as f32) / (m_subs as f32);
            for j in 0..t_subs {
                let v0 = 2.0 * PI * (j as f32) / (t_subs as f32);
                let v1 = 2.0 * PI * ((j + 1) as f32) / (t_subs as f32);
                let (p00, n00) = pt(u0, v0);
                let (p10, _) = pt(u1, v0);
                let (p11, _) = pt(u1, v1);
                let (p01, _) = pt(u0, v1);
                t.push(Triangle {
                    v0: p00,
                    v1: p10,
                    v2: p11,
                    normal: n00
                });
                t.push(Triangle {
                    v0: p00,
                    v1: p11,
                    v2: p01,
                    normal: n00
                });
            }
        }
        t
    }
}