use cgmath::{InnerSpace, Vector3, Vector4};

use crate::Vertex;

#[derive(Clone, Debug)]
pub struct NurbsControlPoint {
    pub point: Vector3<f32>,
    pub weight: f32,
}

impl NurbsControlPoint {
    pub fn to_homogeneous(&self) -> Vector4<f32> {
        Vector4::new(self.point.x * self.weight, self.point.y * self.weight, self.point.z * self.weight, self.weight)
    }
}

#[derive(Clone, Debug)]
pub struct NurbsEntity {
    pub curve: NurbsCurve,
    pub color: [f32; 4],
    pub thickness: u32,
    pub selected: bool,
}

#[derive(Clone, Debug)]
pub struct NurbsSurface {
    pub(crate) degree_u: usize,
    pub(crate) degree_v: usize,
    pub(crate) control_points: Vec<Vec<NurbsControlPoint>>,
    pub(crate) knots_u: Vec<f32>,
    pub(crate) knots_v: Vec<f32>,
}

impl NurbsSurface {
    pub fn de_boor_1d(degree: usize, knots: &[f32], points: &[Vector4<f32>], u: f32) -> Vector4<f32> {
        let n = points.len();
        if n == 0 || degree >= n || knots.len() <= n {
            return Vector4::new(0.0, 0.0, 0.0, 0.0);
        }
        let mut k = if u >= knots[n] {
            n - 1
        } else {
            degree
        };
        if u < knots[n] {
            for idx in degree..n {
                if u >= knots[idx] && u < knots[idx + 1] {
                    k = idx;
                    break;
                }
            }
        }
        let mut d: Vec<Vector4<f32>> = (0..=degree).map(|j| points[j + k - degree]).collect();
        for level in 1..=degree {
            for j in (level..=degree).rev() {
                let left_knot = knots[j + k - degree];
                let right_knot = knots[j + 1 + k - level];
                let denominator = right_knot - left_knot;
                let alpha = if denominator.abs() > 1.0e-7 {
                    (u - left_knot) / denominator
                } else {
                    0.0
                };
                d[j] = d[j - 1] * (1.0 - alpha) + d[j] * alpha;
            }
        }
        d[degree]
    }
    pub fn evaluate(&self, u: f32, v: f32) -> Vector3<f32> {
        if self.control_points.is_empty() || self.control_points[0].is_empty() {
            return Vector3::new(0.0, 0.0, 0.0);
        }
        let u = u.clamp(self.knots_u[self.degree_u], self.knots_u[self.control_points.len()]);
        let v = v.clamp(self.knots_v[self.degree_v], self.knots_v[self.control_points[0].len()]);
        let row_points: Vec<Vector4<f32>> = self
            .control_points
            .iter()
            .map(|row| {
                let row_4d: Vec<Vector4<f32>> =
                    row.iter().map(|cp| cp.to_homogeneous()).collect();
                Self::de_boor_1d(
                    self.degree_v,
                    &self.knots_v,
                    &row_4d,
                    v,
                )
            })
            .collect();
        let pt4 = Self::de_boor_1d(
            self.degree_u,
            &self.knots_u,
            &row_points,
            u,
        );
        if pt4.w.abs() > 1.0e-7 {
            Vector3::new(pt4.x / pt4.w, pt4.y / pt4.w, pt4.z / pt4.w)
        } else {
            Vector3::new(pt4.x, pt4.y, pt4.z)
        }
    }
    pub fn sample_mesh(&self, u_steps: usize, v_steps: usize, color: [f32; 4]) -> Vec<Vertex> {
        if self.control_points.is_empty() || self.control_points[0].is_empty() || u_steps < 2 || v_steps < 2 {
            return Vec::new()
        } else {
            let u_min = self.knots_u[self.degree_u];
            let u_max = self.knots_u[self.control_points.len()];
            let v_min = self.knots_v[self.degree_v];
            let v_max = self.knots_v[self.control_points[0].len()];
            let mut grid: Vec<Vec<Vertex>> = Vec::with_capacity(u_steps + 1);
            for i in 0..=u_steps {
                let tu = (i as f32) / (u_steps as f32);
                let u = u_min * (1.0 - tu) + u_max * tu;
                let mut row = Vec::with_capacity(v_steps + 1);
                for j in 0..=v_steps {
                    let tv = (j as f32) / (v_steps as f32);
                    let v = v_min * (1.0 - tv) + v_max * tv;
                    let pos = self.evaluate(u, v);
                    let eps = 0.001;
                    let tu_vec = self.evaluate(u + eps, v) - self.evaluate(u - eps, v);
                    let tv_vec = self.evaluate(u, v + eps) - self.evaluate(u, v - eps);
                    let n_vec = tu_vec.cross(tv_vec);
                    let normal = if n_vec.magnitude2() > 1.0e-7 {
                        n_vec.normalize()
                    } else {
                        Vector3::new(0.0, 1.0, 0.0)
                    };
                    row.push(Vertex {
                        position: [pos.x, pos.y, pos.z],
                        coords: [normal.x, normal.y, normal.z],
                        color,
                    });
                }
                grid.push(row);
            }
            let mut verts = Vec::with_capacity(u_steps * v_steps * 6);
            for i in 0..u_steps {
                for j in 0..v_steps {
                    let p00 = grid[i][j];
                    let p10 = grid[i + 1][j];
                    let p01 = grid[i][j + 1];
                    let p11 = grid[i + 1][j + 1];
                    verts.push(p00);
                    verts.push(p01);
                    verts.push(p10);
                    verts.push(p10);
                    verts.push(p01);
                    verts.push(p11);
                }
            }
            verts
        }
    }
}

#[derive(Clone, Debug)]
pub struct NurbsCurve {
    pub degree: usize,
    pub(crate) control_points: Vec<NurbsControlPoint>,
    pub knots: Vec<f32>,
}

impl NurbsCurve {
    pub fn is_valid(&self) -> bool {
        if self.control_points.len() > self.degree && self.knots.len() == self.control_points.len() + self.degree + 1 {
            self.knots.windows(2).all(|w| w[0] <= w[1])
        } else {
            false
        }
    }
    pub fn clamped_knot_vector(num_control_points: usize, degree: usize) -> Vec<f32> {
        let t_k = num_control_points + degree + 1;
        let order = degree + 1;
        if num_control_points <= degree {
            return Vec::new()
        }
        let mut knots = Vec::with_capacity(t_k);
        for _ in 0..order {
            knots.push(0.0);
        }
        let num_seg = num_control_points - degree;
        for i in 1..num_seg {
            let to_push = (i as f32) / (num_seg as f32);
            knots.push(to_push);
        }
        for _ in 0..order {
            knots.push(1.0);
        }
        knots
    }
    pub fn find_span(&self, u: f32) -> usize {
        let n = self.control_points.len();
        if u >= self.knots[n] {
            return (n - 1)
        }
        for k in self.degree..n {
            if u >= self.knots[k] && u < self.knots[k + 1] {
                return k;
            }
        }
        self.degree
    }
    pub fn evaluate(&self, u: f32) -> Vector3<f32> {
        if !self.is_valid() {
            return Vector3::new(0.0, 0.0, 0.0)
        }
        let u = u.clamp(self.knots[self.degree], self.knots[self.control_points.len()]);
        let k = self.find_span(u);
        let mut d: Vec<Vector4<f32>> = (0..=self.degree).map(|j| self.control_points[j + k - self.degree].to_homogeneous()).collect();
        for level in 1..=self.degree {
            for j in (level..=self.degree).rev() {
                let left_knot = self.knots[j + k - self.degree];
                let right_knot = self.knots[j + 1 + k - level];
                let denominator = right_knot - left_knot;
                let alpha = if denominator.abs() > 1.0e-7 {
                    (u - left_knot) / denominator
                } else {
                    0.0
                };
                d[j] = d[j - 1] * (1.0 - alpha) + d[j] * alpha;
            }
        }
        let pt4 = d[self.degree];
        if pt4.w.abs() > 1.0e-7 {
            return Vector3::new(pt4.x / pt4.w, pt4.y / pt4.w, pt4.z / pt4.w);
        } else {
            return Vector3::new(pt4.x, pt4.y, pt4.z)
        }
    }
    pub fn sample(&self, subdivisions: usize) -> Vec<Vector3<f32>> {
        if !self.is_valid() || subdivisions < 2 {
            return Vec::new()
        }
        let u_min = self.knots[self.degree];
        let u_max = self.knots[self.control_points.len()];
        let mut result = Vec::new();
        for i in 0..=subdivisions {
            let t = (i as f32) / (subdivisions as f32);
            let u = u_min * (1.0 - t) + u_max * t;
            result.push(self.evaluate(u));
        }
        result
    }
}