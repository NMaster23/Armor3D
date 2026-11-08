use cgmath::{Point3, Vector3};

use crate::camera::Ray;

#[derive(Clone, Copy, Debug)]
pub struct AABB {
    min: Point3<f32>,
    max: Point3<f32>,
}

impl AABB {
    pub fn empty() -> Self {
        Self {
            min: Point3::new(f32::INFINITY, f32::INFINITY, f32::INFINITY),
            max: Point3::new(f32::NEG_INFINITY, f32::NEG_INFINITY, f32::NEG_INFINITY),
        }
    }
    pub fn from_triangle(v0: Point3<f32>, v1: Point3<f32>, v2: Point3<f32>) -> Self {
        Self {
            min: Point3::new(
                v0.x.min(v1.x).min(v2.x),
                v0.y.min(v1.y).min(v2.y),
                v0.z.min(v1.z).min(v2.z),
            ),
            max: Point3::new(
                v0.x.max(v1.x).max(v2.x),
                v0.y.max(v1.y).max(v2.y),
                v0.z.max(v1.z).max(v2.z),
            ),
        }
    }
    pub fn grow(&mut self, p: Point3<f32>) {
        self.min.x = self.min.x.min(p.x);
        self.min.y = self.min.y.min(p.y);
        self.min.z = self.min.z.min(p.z);
        self.max.x = self.max.x.max(p.x);
        self.max.y = self.max.y.max(p.y);
        self.max.z = self.max.z.max(p.z);
    }
    pub fn union(&self, other: &Self) -> Self {
        Self {
            min: Point3::new(
                self.min.x.min(other.min.x),
                self.min.y.min(other.min.y),
                self.min.z.min(other.min.z),
            ),
            max: Point3::new(
                self.max.x.max(other.max.x),
                self.max.y.max(other.max.y),
                self.max.z.max(other.max.z),
            ),
        }
    }
    pub fn longest_axis(&self) -> usize {
        let delta_x = self.max.x - self.min.x;
        let delta_y = self.max.y - self.min.y;
        let delta_z = self.max.z - self.min.z;
        if delta_x >= delta_y && delta_x >= delta_z {
            return 0;
        } else if delta_y >= delta_z {
            return 1;
        } else {
            return 2;
        }
    }
    pub fn intersect_ray(&self, ray: &Ray) -> Option<f32> {
        let inv_d = Vector3::new(
            1.0 / ray.direction.x,
            1.0 / ray.direction.y,
            1.0 / ray.direction.z,
        );
        let t1 = (self.min.x - ray.origin.x) * inv_d.x;
        let t2 = (self.max.x - ray.origin.x) * inv_d.x;
        let t3 = (self.min.y - ray.origin.y) * inv_d.y;
        let t4 = (self.max.y - ray.origin.y) * inv_d.y;
        let t5 = (self.min.z - ray.origin.z) * inv_d.z;
        let t6 = (self.max.z - ray.origin.z) * inv_d.z;
        let tmin = t1.min(t2).max(t3.min(t4)).max(t5.min(t6));
        let tmax = t1.max(t2).min(t3.max(t4)).min(t5.max(t6));
        if tmax < 0.0 || tmin > tmax {
            None
        } else {
            Some(if tmin < 0.0 {
                0.0
            } else {
                tmin
            })
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct BvhTriangle {
    pub v0: Point3<f32>,
    pub v1: Point3<f32>,
    pub v2: Point3<f32>,
}

impl BvhTriangle {
    pub fn new(v0: Point3<f32>, v1: Point3<f32>, v2: Point3<f32>) -> Self {
        Self {
            v0,
            v1,
            v2,
        }
    }
    pub fn aabb(&self) -> AABB {
        AABB::from_triangle(self.v0, self.v1, self.v2)
    }
    pub fn centroid(&self) -> Point3<f32> {
        Point3::new(
            (self.v0.x + self.v1.x + self.v2.x) / 3.0,
            (self.v0.y + self.v1.y + self.v2.y) / 3.0,
            (self.v0.z + self.v1.z + self.v2.z) / 3.0,
        )
    }
}

pub enum BvhNode {
    Leaf {
        aabb: AABB,
        tris: Vec<BvhTriangle>,
    },
    Internal {
        aabb: AABB,
        left: Box<BvhNode>,
        right: Box<BvhNode>,
    },
}

impl BvhNode {
    pub fn aabb(&self) -> &AABB {
        match self {
            BvhNode::Leaf { aabb, .. } => aabb,
            BvhNode::Internal { aabb, .. } => aabb,
        }
    }
}

pub struct Bvh {
    pub root: Option<BvhNode>,
}

impl Bvh {
    pub fn build(mut tris: Vec<BvhTriangle>) -> Self {
        if tris.is_empty() {
            return Self {
                root: None
            };
        }
        let root = Self::build_recursive(&mut tris, 4);
        Self {
            root: Some(root)
        }
    }
    pub fn build_recursive(tris: &mut [BvhTriangle], max_leaf_size: usize) -> BvhNode {
        let mut aabb = AABB::empty();
        for tri in tris.iter() {
            aabb = aabb.union(&tri.aabb());
        }
        if tris.len() <= max_leaf_size {
            return BvhNode::Leaf { aabb, tris: tris.to_vec(), };
        }
        let mut centroid_bounds = AABB::empty();
        for tri in tris.iter() {
            centroid_bounds.grow(tri.centroid());
        }
        let axis = centroid_bounds.longest_axis();
        tris.sort_unstable_by(|a, b| {
            let ca = match axis {
                0 => a.centroid().x,
                1 => a.centroid().y,
                _ => a.centroid().z,
            };
            let cb = match axis {
                0 => b.centroid().x,
                1 => b.centroid().y,
                _ => b.centroid().z,
            };
            ca.partial_cmp(&cb).unwrap_or(std::cmp::Ordering::Equal)
        });
        let mid = tris.len() / 2;
        let (left_tris, right_tris) = tris.split_at_mut(mid);
        let left = Self::build_recursive(left_tris, max_leaf_size);
        let right = Self::build_recursive(right_tris, max_leaf_size);
        BvhNode::Internal { aabb, left: Box::new(left), right: Box::new(right) }
    }
    pub fn ray_intersect(&self, ray: &Ray) -> Option<f32> {
        let root = self.root.as_ref()?;
        let mut closest_t = f32::INFINITY;
        Self::intersect_recursive(root, ray, &mut closest_t);
        if closest_t.is_finite() {
            Some(closest_t)
        } else {
            None
        }
    }
    pub fn intersect_recursive(node: &BvhNode, ray: &Ray, closest_t: &mut f32) {
        if let Some(box_t) = node.aabb().intersect_ray(ray) {
            if box_t >= *closest_t {
                return;
            }
        } else {
            return;
        }
        match node {
            BvhNode::Leaf { tris, .. } => {
                for tri in tris {
                    if let Some(t) = ray.intersect_triangle(tri.v0, tri.v1, tri.v2) {
                        if t < *closest_t {
                            *closest_t = t;
                        }
                    }
                }
            }
            BvhNode::Internal { left, right, .. } => {
                Self::intersect_recursive(left, ray, closest_t);
                Self::intersect_recursive(right, ray, closest_t);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_aabb_ray_hit_miss() {
        let aabb = AABB {
            min: Point3::new(-1.0, -1.0, -1.0),
            max: Point3::new(1.0, 1.0, 1.0),
        };
        let hit_ray = Ray {
            origin: Point3::new(0.0, 0.0, -5.0),
            direction: Vector3::new(0.0, 0.0, 1.0),
        };
        assert!(aabb.intersect_ray(&hit_ray).is_some());
        let miss_ray = Ray {
            origin: Point3::new(0.0, 0.0, -5.0),
            direction: Vector3::new(0.0, 1.0, 0.0),
        };
        assert!(aabb.intersect_ray(&miss_ray).is_none());
    }
    #[test]
    fn test_bvh_tri_hit() {
        let tri = BvhTriangle::new(
            Point3::new(-1.0, -1.0, 0.0),
            Point3::new(1.0, -1.0, 0.0),
            Point3::new(0.0, 1.0, 0.0),
        );
        let bvh = Bvh::build(vec![tri]);
        let ray = Ray {
            origin: Point3::new(0.0, 0.0, -5.0),
            direction: Vector3::new(0.0, 0.0, 1.0),
        };
        let hit = bvh.ray_intersect(&ray);
        assert!(hit.is_some());
        assert!((hit.unwrap() - 5.0).abs() < 1e-4);
    }
}