use cgmath::{InnerSpace, Vector3};

use crate::exporter::Triangle;

pub const EPSILON: f32 = 1e-5;

const COPLANAR: i32 = 0;
const FRONT: i32 = 1;
const BACK: i32 = 2;
const SPANNING: i32 = 3;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Vertex {
    pub pos: Vector3<f32>,
    pub normal: Vector3<f32>,
}

impl Vertex {
    pub fn new(pos: [f32; 3], normal: [f32; 3]) -> Self {
        Self {
            pos: Vector3::new(pos[0], pos[1], pos[2]),
            normal: Vector3::new(normal[0], normal[1], normal[2]),
        }
    }
    pub fn interpolate(&self, other: &Self, t: f32) -> Self {
        Self {
            pos: self.pos + (other.pos - self.pos) * t,
            normal: {
                let n= self.normal + (other.normal - self.normal) * t;
                if n.magnitude() > EPSILON {
                    n.normalize()
                } else {
                    self.normal
                }
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct Plane {
    pub normal: Vector3<f32>,
    pub w: f32,
}

impl Plane {
    pub fn from_points(a: Vector3<f32>, b: Vector3<f32>, c: Vector3<f32>) -> Option<Self> {
        let normal = (b - a).cross(c - a);
        if normal.magnitude() <= EPSILON {
            return None;
        }
        let normal = normal.normalize();
        let w = normal.dot(a);
        Some(Self {
            normal,
            w,
        })
    }
    pub fn flip(&mut self) {
        self.normal = -self.normal;
        self.w = -self.w;
    }
    pub fn split_poly(
        &self,
        polygon: &Polygon,
        coplanar_f: &mut Vec<Polygon>,
        coplanar_b: &mut Vec<Polygon>,
        front: &mut Vec<Polygon>,
        back: &mut Vec<Polygon>,
    ) {
        let mut poly_type = 0;
        let mut types = Vec::with_capacity(polygon.verts.len());
        for v in &polygon.verts {
            let t = self.normal.dot(v.pos) - self.w;
            let v_type = if t < -EPSILON {
                BACK
            } else if t > EPSILON {
                FRONT
            } else {
                COPLANAR
            };
            poly_type |= v_type;
            types.push(v_type);
        }
        match poly_type {
            COPLANAR => {
                if self.normal.dot(polygon.plane.normal) > 0.0 {
                    coplanar_f.push(polygon.clone());
                } else {
                    coplanar_b.push(polygon.clone());
                }
            }
            FRONT => {
                front.push(polygon.clone());
            }
            BACK => {
                back.push(polygon.clone());
            }
            SPANNING => {
                let mut f_verts = Vec::new();
                let mut b_verts = Vec::new();
                for i in 0..polygon.verts.len() {
                    let j = (i + 1) % polygon.verts.len();
                    let ti = types[i];
                    let tj = types[j];
                    let vi = &polygon.verts[i];
                    let vj = &polygon.verts[j];
                    if ti != BACK {
                        f_verts.push(*vi);
                    }
                    if ti != FRONT {
                        b_verts.push(*vi);
                    }
                    if (ti | tj) == SPANNING {
                        let denom = self.normal.dot(vj.pos - vi.pos);
                        let t = if denom.abs() > EPSILON {
                            ((self.w - self.normal.dot(vi.pos)) / denom).clamp(0.0, 1.0)
                        } else {
                            0.5
                        };
                        let v = vi.interpolate(vj, t.clamp(0.0, 1.0));
                        f_verts.push(v);
                        b_verts.push(v);
                    }
                }
                if f_verts.len() >= 3 {
                    front.push(Polygon::new(f_verts, polygon.plane.clone()));
                }
                if b_verts.len() >= 3 {
                    back.push(Polygon::new(b_verts, polygon.plane.clone()));
                }
            }
            _ => {}
        }
    }
}

#[derive(Clone, Debug)]
pub struct Polygon {
    pub verts: Vec<Vertex>,
    pub plane: Plane,
}

impl Polygon {
    pub fn new(verts: Vec<Vertex>, plane: Plane) -> Self {
        Self {
            verts,
            plane,
        }
    }
    pub fn from_tris(tri: &Triangle) -> Option<Self> {
        let v0 = Vertex::new(tri.v0, tri.normal);
        let v1 = Vertex::new(tri.v1, tri.normal);
        let v2 = Vertex::new(tri.v2, tri.normal);
        let plane = Plane::from_points(v0.pos, v1.pos, v2.pos)?;
        Some(Self::new(vec![v0, v1, v2], plane))
    }
    pub fn flip(&mut self) {
        self.verts.reverse();
        for v in &mut self.verts {
            v.normal = -v.normal;
        }
        self.plane.flip();
    }
    pub fn to_tris(&self) -> Vec<Triangle> {
        let mut tris = Vec::new();
        if self.verts.len() < 3 {
            return tris;
        }
        let v0 = self.verts[0];
        for i in 1..self.verts.len() - 1 {
            let v1 = self.verts[i];
            let v2 = self.verts[i + 1];
            tris.push(Triangle {
                v0: [v0.pos.x, v0.pos.y, v0.pos.z],
                v1: [v1.pos.x, v1.pos.y, v1.pos.z],
                v2: [v2.pos.x, v2.pos.y, v2.pos.z],
                normal: [self.plane.normal.x, self.plane.normal.y, self.plane.normal.z],
            });
        }
        tris
    }
}

#[derive(Clone, Debug)]
pub struct Node {
    pub plane: Option<Plane>,
    pub front: Option<Box<Node>>,
    pub back: Option<Box<Node>>,
    pub polygons: Vec<Polygon>,
}

impl Node {
    pub fn new() -> Self {
        Self {
            plane: None,
            front: None,
            back: None,
            polygons: Vec::new(),
        }
    }
    pub fn from_poly(poly: &[Polygon]) -> Self {
        let mut node = Self::new();
        node.build(poly);
        node
    }
    pub fn invert(&mut self) {
        for p in &mut self.polygons {
            p.flip();
        }
        if let Some(plane) = &mut self.plane {
            plane.flip();
        }
        if let Some(f) = &mut self.front {
            f.invert();
        }
        if let Some(b) = &mut self.back {
            b.invert();
        }
        std::mem::swap(&mut self.front, &mut self.back);
    }
    pub fn clip_polys(&self, poly: &[Polygon]) -> Vec<Polygon> {
        if self.plane.is_none() {
            return poly.to_vec();
        }
        let plane = self.plane.as_ref().unwrap();
        let mut c_f = Vec::new();
        let mut c_b = Vec::new();
        let mut front = Vec::new();
        let mut back = Vec::new();
        for p in poly {
            plane.split_poly(p, &mut c_f, &mut c_b, &mut front, &mut back);
        }
        front.extend(c_f);
        back.extend(c_b);
        if let Some(f) = &self.front {
            front = f.clip_polys(&front);
        }
        if let Some(b) = &self.back {
            back = b.clip_polys(&back);
        } else {
            back.clear();
        }
        front.extend(back);
        front
    }
    pub fn clip_to(&mut self, other: &Node) {
        self.polygons = other.clip_polys(&self.polygons);
        if let Some(f) = self.front.as_mut() {
            f.clip_to(other);
        }
        if let Some(b) = self.back.as_mut() {
            b.clip_to(other);
        }
    }
    pub fn all_polys(&self) -> Vec<Polygon> {
        let mut list = self.polygons.clone();
        if let Some(f) = &self.front {
            list.extend(f.all_polys());
        }
        if let Some(b) = &self.back {
            list.extend(b.all_polys());
        }
        list
    }
    pub fn build(&mut self, poly: &[Polygon]) {
        if poly.is_empty() {
            return;
        }
        if self.plane.is_none() {
            self.plane = Some(poly[0].plane.clone());
        }
        let plane = self.plane.as_ref().unwrap().clone();
        let mut c_f = Vec::new();
        let mut c_b = Vec::new();
        let mut front = Vec::new();
        let mut back = Vec::new();
        for p in poly {
            plane.split_poly(p, &mut c_f, &mut c_b, &mut front, &mut back);
        }
        self.polygons.extend(c_f);
        self.polygons.extend(c_b);
        if !front.is_empty() {
            if self.front.is_none() {
                self.front = Some(Box::new(Node::new()))
            }
            self.front.as_mut().unwrap().build(&front);
        }
        if !back.is_empty() {
            if self.back.is_none() {
                self.back = Some(Box::new(Node::new()));
            }
            self.back.as_mut().unwrap().build(&back);
        }
    }
}

#[derive(Clone, Debug)]
pub struct CSG {
    pub polygons: Vec<Polygon>,
}

impl CSG {
    pub fn new() -> Self {
        Self {
            polygons: Vec::new(),
        }
    }
    pub fn from_poly(polys: Vec<Polygon>) -> Self {
        Self {
            polygons: polys,
         }
    }
    pub fn from_tris(tris: &[Triangle]) -> Self {
        let mut polys = Vec::new();
        for t in tris {
            if let Some(p) = Polygon::from_tris(t) {
                polys.push(p);
            }
        }
        Self {
            polygons: polys,
        }
    }
    pub fn to_tris(&self) -> Vec<Triangle> {
        let mut tris = Vec::new();
        for p in &self.polygons {
            tris.extend(p.to_tris());
        }
        tris
    }
    pub fn union(&self, other: &CSG) -> CSG {
        let mut a = Node::from_poly(&self.polygons);
        let mut b = Node::from_poly(&other.polygons);
        a.clip_to(&b);
        b.clip_to(&a);
        b.invert();
        b.clip_to(&a);
        b.invert();
        a.build(&b.all_polys());
        CSG::from_poly(a.all_polys())
    }
    pub fn difference(&self, other: &CSG) -> CSG {
        let mut a = Node::from_poly(&self.polygons);
        let mut b = Node::from_poly(&other.polygons);
        a.invert();
        a.clip_to(&b);
        b.clip_to(&a);
        b.invert();
        b.clip_to(&a);
        b.invert();
        a.build(&b.all_polys());
        a.invert();
        CSG::from_poly(a.all_polys())
    }
    pub fn intersect(&self, other: &CSG) -> CSG {
        let mut a = Node::from_poly(&self.polygons);
        let mut b = Node::from_poly(&other.polygons);
        a.invert();
        b.clip_to(&a);
        b.invert();
        a.clip_to(&b);
        b.clip_to(&a);
        a.build(&b.all_polys());
        a.invert();
        CSG::from_poly(a.all_polys())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::objects::SolidEntity;
    #[test]
    fn test_cube_union() {
        let box_a = SolidEntity::create_box([0.0, 0.0, 0.0], 2.0, 2.0, 2.0);
        let box_b = SolidEntity::create_box([1.0, 0.0, 0.0], 2.0, 2.0, 2.0);
        let csg_a = CSG::from_tris(&box_a);
        let csg_b = CSG::from_tris(&box_b);
        let result = csg_a.union(&csg_b);
        let tris = result.to_tris();
        assert!(!tris.is_empty());
        assert!(tris.len() > 12);
    }
    #[test]
    fn test_cube_diff() {
        let box_a = SolidEntity::create_box([0.0, 0.0, 0.0], 2.0, 2.0, 2.0);
        let box_b = SolidEntity::create_box([1.0, 0.0, 0.0], 2.0, 2.0, 2.0);
        let csg_a = CSG::from_tris(&box_a);
        let csg_b = CSG::from_tris(&box_b);
        let result = csg_a.difference(&csg_b);
        let tris = result.to_tris();
        assert!(!tris.is_empty());
        assert!(tris.len() > 12);
    }
    #[test]
    fn test_cube_intersect() {
        let box_a = SolidEntity::create_box([0.0, 0.0, 0.0],2.0, 2.0, 2.0);
        let box_b = SolidEntity::create_box([1.0, 0.0, 0.0], 2.0, 2.0, 2.0);
        let csg_a = CSG::from_tris(&box_a);
        let csg_b = CSG::from_tris(&box_b);
        let result = csg_a.intersect(&csg_b);
        let tris = result.to_tris();
        assert!(!tris.is_empty());
        assert!(tris.len() > 12);
    }
}