use crate::Terrain;
use glam::Vec3;

#[derive(Clone, Copy, Debug)]
pub struct Aabb {
    pub min: Vec3,
    pub max: Vec3,
}

impl Aabb {
    pub fn from_center_half(center: Vec3, half_extents: Vec3) -> Self {
        Self {
            min: center - half_extents,
            max: center + half_extents,
        }
    }

    pub fn center(&self) -> Vec3 {
        (self.min + self.max) * 0.5
    }

    pub fn half_extents(&self) -> Vec3 {
        (self.max - self.min) * 0.5
    }

    /// Resolve a vertical capsule (feet at `pos`, radius `r`, height `h`) against this AABB.
    /// Returns `(push_vector, landed_on_top)`.
    pub fn resolve_capsule(&self, pos: Vec3, radius: f32, height: f32) -> Option<(Vec3, bool)> {
        let cap_min_y = pos.y;
        let cap_max_y = pos.y + height;

        if cap_max_y <= self.min.y || cap_min_y >= self.max.y {
            return None;
        }

        let closest_x = pos.x.clamp(self.min.x, self.max.x);
        let closest_z = pos.z.clamp(self.min.z, self.max.z);
        let dx = pos.x - closest_x;
        let dz = pos.z - closest_z;
        let dist_sq = dx * dx + dz * dz;

        if dist_sq >= radius * radius {
            return None;
        }

        // Check if standing on top of the box
        let pen_top = self.max.y - cap_min_y;
        let pen_bot = cap_max_y - self.min.y;

        if pen_top > 0.0 && pen_top < 0.75 && pen_top < pen_bot {
            return Some((Vec3::new(0.0, pen_top, 0.0), true));
        }

        if dist_sq > 1e-6 {
            let dist = dist_sq.sqrt();
            let push = radius - dist;
            Some((Vec3::new(dx / dist * push, 0.0, dz / dist * push), false))
        } else {
            // Inside box XZ footprint: push out along nearest face
            let px_min = (pos.x - self.min.x).abs();
            let px_max = (self.max.x - pos.x).abs();
            let pz_min = (pos.z - self.min.z).abs();
            let pz_max = (self.max.z - pos.z).abs();
            let min_p = px_min.min(px_max).min(pz_min).min(pz_max);
            if pen_top < min_p + radius {
                Some((Vec3::new(0.0, pen_top, 0.0), true))
            } else if min_p == px_min {
                Some((Vec3::new(-(px_min + radius), 0.0, 0.0), false))
            } else if min_p == px_max {
                Some((Vec3::new(px_max + radius, 0.0, 0.0), false))
            } else if min_p == pz_min {
                Some((Vec3::new(0.0, 0.0, -(pz_min + radius)), false))
            } else {
                Some((Vec3::new(0.0, 0.0, pz_max + radius), false))
            }
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct RayHit {
    pub t: f32,
    pub point: Vec3,
    pub normal: Vec3,
}

/// Ray-sphere intersection. Returns closest positive distance `t`.
pub fn raycast_sphere(origin: Vec3, dir: Vec3, center: Vec3, radius: f32) -> Option<f32> {
    let oc = origin - center;
    let b = oc.dot(dir);
    let c = oc.dot(oc) - radius * radius;
    let disc = b * b - c;
    if disc < 0.0 {
        return None;
    }
    let sqrt_d = disc.sqrt();
    let t0 = -b - sqrt_d;
    if t0 >= 0.0 {
        return Some(t0);
    }
    let t1 = -b + sqrt_d;
    if t1 >= 0.0 {
        Some(t1)
    } else {
        None
    }
}

/// Slab ray-AABB intersection.
pub fn raycast_aabb(origin: Vec3, dir: Vec3, aabb: &Aabb, max_t: f32) -> Option<RayHit> {
    let mut tmin = 0.0_f32;
    let mut tmax = max_t;
    let mut hit_normal = Vec3::Y;

    for axis in 0..3 {
        let o = origin[axis];
        let d = dir[axis];
        let min_b = aabb.min[axis];
        let max_b = aabb.max[axis];

        if d.abs() < 1e-7 {
            if o < min_b || o > max_b {
                return None;
            }
        } else {
            let inv_d = 1.0 / d;
            let mut t1 = (min_b - o) * inv_d;
            let mut t2 = (max_b - o) * inv_d;
            let mut n_sign = -1.0;
            if t1 > t2 {
                std::mem::swap(&mut t1, &mut t2);
                n_sign = 1.0;
            }
            if t1 > tmin {
                tmin = t1;
                hit_normal = Vec3::ZERO;
                hit_normal[axis] = n_sign;
            }
            tmax = tmax.min(t2);
            if tmin > tmax {
                return None;
            }
        }
    }

    Some(RayHit {
        t: tmin,
        point: origin + dir * tmin,
        normal: hit_normal,
    })
}

/// Raymarch + binary refinement against terrain heightfield.
pub fn raycast_terrain(origin: Vec3, dir: Vec3, terrain: &Terrain, max_dist: f32) -> Option<RayHit> {
    let mut t = 0.0_f32;
    let mut step = 1.5_f32;
    let mut prev_t = 0.0_f32;

    while t < max_dist {
        let p = origin + dir * t;
        let h = terrain.sample_height(p.x, p.z);
        if p.y <= h {
            // Binary refine between prev_t and t
            let mut lo = prev_t;
            let mut hi = t;
            for _ in 0..6 {
                let mid = (lo + hi) * 0.5;
                let mp = origin + dir * mid;
                if mp.y <= terrain.sample_height(mp.x, mp.z) {
                    hi = mid;
                } else {
                    lo = mid;
                }
            }
            let hit_p = origin + dir * hi;
            let normal = terrain.sample_normal(hit_p.x, hit_p.z);
            return Some(RayHit {
                t: hi,
                point: hit_p,
                normal,
            });
        }
        prev_t = t;
        // Adaptive step based on altitude above terrain
        let clearance = (p.y - h).max(0.0);
        step = (clearance * 0.45).clamp(1.2, 10.0);
        t += step;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use exo_core::BiomeId;

    #[test]
    fn raycast_sphere_hits() {
        let t = raycast_sphere(Vec3::ZERO, Vec3::Z, Vec3::new(0.0, 0.0, 10.0), 2.0);
        assert!((t.unwrap() - 8.0).abs() < 1e-4);
    }

    #[test]
    fn raycast_terrain_hits_ground() {
        let terrain = Terrain::generate(BiomeId::CrimsonMesa);
        let hit = raycast_terrain(Vec3::new(0.0, 50.0, 75.0), Vec3::NEG_Y, &terrain, 200.0);
        assert!(hit.is_some());
        let hit = hit.unwrap();
        assert!((hit.point.y - terrain.sample_height(0.0, 75.0)).abs() < 0.2);
    }
}
