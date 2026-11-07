use std::f32::consts::PI;
use cgmath::{InnerSpace, Vector3};

use crate::csg::CSG;

use crate::{Vertex, exporter::Triangle};


#[derive(Clone, Debug)]
pub enum BaseShape {
    Box { width: f32, height: f32, depth: f32 },
    Cylinder { radius: f32, height: f32, subdivisions: usize },
    Sphere { radius: f32, rings: usize, sectors: usize },
    Cone { radius: f32, height: f32, subdivisions: usize },
    Torus { outer_radius: f32, inner_radius: f32, outer_subdivisions: usize, inner_subdivisions: usize },
    CustomMesh { triangles: Vec<Triangle> },
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
    pub fn from_revolve(
        id: usize,
        points: &[Vector3<f32>],
        axis_origin: Vector3<f32>,
        axis_dir: Vector3<f32>,
        angle_deg: f32,
        subdivisions: u32,
        color: [f32; 4],
    ) -> Option<Self> {
        let tris = Self::create_revolve(points, axis_origin, axis_dir, angle_deg, subdivisions)?;
        Some(Self {
            id,
            origin: [0.0, 0.0, 0.0],
            shape: BaseShape::CustomMesh { triangles: tris },
            color,
            selected: true,
        })
    }
    pub fn create_revolve(
        points: &[Vector3<f32>],
        axis_origin: Vector3<f32>,
        axis_dir: Vector3<f32>,
        angle_deg: f32,
        subdivisions: u32,
    ) -> Option<Vec<Triangle>> {
        if points.len() < 2 || subdivisions < 3 {
            return None;
        }
        let dir = if axis_dir.magnitude2() > 1e-6 {
            axis_dir.normalize()
        } else {
            Vector3::unit_z()
        };
        let total_angle = angle_deg.to_radians();
        let is_full_circle = (angle_deg.abs() - 360.0).abs() < 1e-3;
        let n_steps = (subdivisions as usize).max(8);
        let n_pts = points.len();
        let rotate_pt = |p: Vector3<f32>, theta: f32| -> Vector3<f32> {
            let v = p - axis_origin;
            let (s, c) = theta.sin_cos();
            axis_origin + v * c + dir.cross(v) * s + dir * dir.dot(v) * (1.0 - c)
        };
        let num_rings = if is_full_circle {
            n_steps
        } else {
            n_steps + 1
        };
        let mut rings: Vec<Vec<Vector3<f32>>> = Vec::with_capacity(num_rings);
        for s in 0..num_rings {
            let theta = total_angle * (s as f32 / n_steps as f32);
            let mut ring = Vec::with_capacity(n_pts);
            for pt in points {
                ring.push(rotate_pt(*pt, theta));
            }
            rings.push(ring);
        }
        let mut tris = Vec::new();
        for s in 0..n_steps {
            let next_s = (s + 1) % rings.len();
            if !is_full_circle && s + 1 >= rings.len() {
                break;
            }
            let ring_a = &rings[s];
            let ring_b = &rings[next_s];
            for k in 0..n_pts - 1 {
                let v00 = ring_a[k];
                let v01 = ring_a[k + 1];
                let v10 = ring_b[k];
                let v11 = ring_b[k + 1];
                let d0 = (v10 - v00).magnitude2();
                let d1 = (v11 - v01).magnitude2();
                if d0 > 1e-6 && d1 > 1e-6 {
                    let norm1 = (v10 - v00).cross(v11 - v00);
                    let n1 = if norm1.magnitude2() > 1e-6 {
                        norm1.normalize()
                    } else {
                        dir
                    };
                    tris.push(Triangle {
                        v0: [v00.x, v00.y, v00.z],
                        v1: [v10.x, v10.y, v10.z],
                        v2: [v11.x, v11.y, v11.z],
                        normal: [n1.x, n1.y, n1.z],
                    });
                    let norm2 = (v11 - v00).cross(v01 - v00);
                    let n2 = if norm2.magnitude2() > 1e-6 {
                        norm2.normalize()
                    } else {
                        dir
                    };
                    tris.push(Triangle {
                        v0: [v00.x, v00.y, v00.z],
                        v1: [v11.x, v11.y, v11.z],
                        v2: [v01.x, v01.y, v01.z],
                        normal: [n2.x, n2.y, n2.z],
                    });
                } else if d0 > 1e-6 {
                    let norm = (v11 - v00).cross(v01 - v00);
                    let n = if norm.magnitude2() > 1e-6 {
                        norm.normalize()
                    } else {
                        dir
                    };
                    tris.push(Triangle {
                        v0: [v00.x,v00.y, v00.z],
                        v1: [v11.x, v11.y, v11.z],
                        v2: [v01.x, v01.y, v01.z],
                        normal: [n.x, n.y, n.z],
                    });
                } else if d1 > 1e-6 {
                    let norm = (v11 - v00).cross(v01 - v00);
                    let n = if norm.magnitude2() > 1e-6 {
                        norm.normalize()
                    } else {
                        dir
                    };
                    tris.push(Triangle {
                        v0: [v00.x, v00.y, v00.z],
                        v1: [v11.x, v11.y, v11.z],
                        v2: [v01.x, v01.y, v01.z],
                        normal: [n.x, n.y, n.z],
                    });
                }
            }
        }
        if !is_full_circle && rings.len() > 1 && n_pts >= 3 {
            let start_ring = &rings[0];
            let end_ring = &rings[rings.len() - 1];
            let v_ref = start_ring[0];
            for i in 1..n_pts - 1 {
                let v1 = start_ring[i];
                let v2 = start_ring[i + 1];
                let n = (v2 - v_ref).cross(v1 - v_ref);
                let norm = if n.magnitude2() > 1e-6 {
                    n.normalize()
                } else {
                    -dir
                };
                tris.push(Triangle {
                    v0: [v_ref.x, v_ref.y, v_ref.z],
                    v1: [v2.x, v2.y, v2.z],
                    v2: [v1.x, v1.y, v1.z],
                    normal: [norm.x, norm.y, norm.z],
                });
            }
            let end_ref = end_ring[0];
            for i in 1..n_pts - 1 {
                let v1 = end_ring[i];
                let v2 = end_ring[i + 1];
                let n = (v1 - end_ref).cross(v2 - end_ref);
                let norm = if n.magnitude2() > 1e-6 {
                    n.normalize()
                } else {
                    dir
                };
                tris.push(Triangle {
                    v0: [end_ref.x, end_ref.y, end_ref.z],
                    v1: [v1.x, v1.y, v1.z],
                    v2: [v2.x, v2.y, v2.z],
                    normal: [norm.x, norm.y, norm.z],
                });
            }
        }
        Some(tris)
    }
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
            BaseShape::CustomMesh { triangles } => {
                triangles.iter().map(|tri| {
                    Triangle {
                        v0: [tri.v0[0] + self.origin[0], tri.v0[1] + self.origin[1], tri.v0[2] + self.origin[2]],
                        v1: [tri.v1[0] + self.origin[0], tri.v1[1] + self.origin[1], tri.v1[2] + self.origin[2]],
                        v2: [tri.v2[0] + self.origin[0], tri.v2[1] + self.origin[1], tri.v2[2] + self.origin[2]],
                        normal: tri.normal,
                    }
                }).collect()
            }
        }
    }
    pub fn to_csg(&self) -> CSG {
        let tris = self.to_tris();
        CSG::from_tris(&tris)
    }
    pub fn from_csg(id: usize, csg: &CSG, color: [f32; 4]) -> Self {
        let tris = csg.to_tris();
        Self {
            id,
            origin: [0.0, 0.0, 0.0],
            shape: BaseShape::CustomMesh { triangles: tris },
            color,
            selected: false,
        }
    }
    pub fn from_extrusion(
        id: usize,
        points: &[Vector3<f32>],
        h: f32,
        axis: Option<Vector3<f32>>,
        color: [f32; 4]
    ) -> Option<Self> {
        let tris = Self::create_extrusion(points, h, axis)?;
        Some(Self {
            id,
            origin: [0.0, 0.0, 0.0],
            shape: BaseShape::CustomMesh { triangles: tris },
            color,
            selected: true,
        })
    }
    pub fn create_extrusion(
        points: &[Vector3<f32>],
        h: f32,
        axis: Option<Vector3<f32>>,
    ) -> Option<Vec<Triangle>> {
        let pts = if points.len() > 1 && (points.first()? - points.last()?).magnitude2() < 1e-6 {
            &points[..points.len() - 1]
        } else {
            points
        };
        if pts.len() < 3 {
            return None;
        }
        let dir = match axis {
            Some(a) if a.magnitude2() > 1e-6 => a.normalize(),
            _ => Vector3::unit_z(),
        };
        let h_vec = dir * h;
        let mut tris = Vec::new();
        let n = pts.len();
        for i in 0..n {
            let j = (i + 1) % n;
            let b0 = pts[i];
            let b1 = pts[j];
            let t0 = b0 + h_vec;
            let t1 = b1 + h_vec;
            let wall_cross = (b1 - b0).cross(h_vec);
            let wall_norm = if wall_cross.magnitude2() > 1e-6 {
                wall_cross.normalize()
            } else {
                Vector3::unit_y()
            };
            let norm = [wall_norm.x, wall_norm.y, wall_norm.z];
            tris.push(Triangle {
                v0: [b0.x, b0.y, b0.z],
                v1: [b1.x, b1.y, b1.z],
                v2: [t1.x, t1.y, t1.z],
                normal: norm,
            });
            tris.push(Triangle {
                v0: [b0.x, b0.y, b0.z],
                v1: [t1.x, t1.y, t1.z],
                v2: [t0.x, t0.y, t0.z],
                normal: norm,
            });
        }
        let b_norm = [(-dir).x, (-dir).y, (-dir).z];
        for i in 1..n - 1 {
            let b0 = pts[0];
            let bi = pts[i];
            let bj = pts[i + 1];
            tris.push(Triangle {
                v0: [b0.x, b0.y, b0.z],
                v1: [bi.x, bi.y, bi.z],
                v2: [bj.x, bj.y, bj.z],
                normal: b_norm,
            });
        }
        let t_norm = [dir.x, dir.y, dir.z];
        for i in 1..n - 1 {
            let t0 = pts[0] + h_vec;
            let ti = pts[i] + h_vec;
            let tj = pts[i + 1] + h_vec;
            tris.push(Triangle {
                v0: [t0.x, t0.y, t0.z],
                v1: [ti.x, ti.y, ti.z],
                v2: [tj.x, tj.y, tj.z],
                normal: t_norm,
            });
        }
        Some(tris)
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
                v1: v2,
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
    pub fn bounding_box(&self) -> ([f32; 3], [f32; 3]) {
        let tris = self.to_tris();
        if tris.is_empty() {
            return (self.origin, self.origin);
        }
        let mut min = [f32::INFINITY; 3];
        let mut max = [f32::NEG_INFINITY; 3];
        for tri in &tris {
            for v in &[tri.v0, tri.v1, tri.v2] {
                for i in 0..3 {
                    if v[i] < min[i] {
                        min[i] = v[i];
                    }
                    if v[i] > max[i] {
                        max[i] = v[i];
                    }
                }
            }
        }
        (min, max)
    }
    pub fn centroid(&self) -> [f32; 3] {
        let (min, max) = self.bounding_box();
        return [(min[0] + max[0]) * 0.5, (min[1] + max[1]) * 0.5, (min[2] + max[2]) * 0.5];
    }
}