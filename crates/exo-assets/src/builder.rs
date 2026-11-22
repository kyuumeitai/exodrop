use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Quat, Vec3};

/// Rich per-vertex PBR format supporting metallic armor + HDR glowing visors/cores
/// inside a single mesh with zero external textures required.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct PbrVertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub color: [f32; 3],
    pub metallic: f32,
    pub emissive: [f32; 3],
    pub roughness: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct Material {
    pub color: Vec3,
    pub metallic: f32,
    pub roughness: f32,
    pub emissive: Vec3,
}

impl Material {
    pub const fn matte(color: Vec3, roughness: f32) -> Self {
        Self {
            color,
            metallic: 0.05,
            roughness,
            emissive: Vec3::ZERO,
        }
    }

    pub const fn metal(color: Vec3, metallic: f32, roughness: f32) -> Self {
        Self {
            color,
            metallic,
            roughness,
            emissive: Vec3::ZERO,
        }
    }

    pub const fn glow(color: Vec3, emissive: Vec3) -> Self {
        Self {
            color,
            metallic: 0.1,
            roughness: 0.25,
            emissive,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct MeshData {
    pub vertices: Vec<PbrVertex>,
    pub indices: Vec<u32>,
}

#[derive(Default)]
pub struct MeshBuilder {
    pub vertices: Vec<PbrVertex>,
    pub indices: Vec<u32>,
}

impl MeshBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn build(self) -> MeshData {
        MeshData {
            vertices: self.vertices,
            indices: self.indices,
        }
    }

    pub fn push_tri(&mut self, a: Vec3, b: Vec3, c: Vec3, mat: Material) {
        let n = (b - a).cross(c - a).normalize_or_zero();
        let base = self.vertices.len() as u32;
        for p in [a, b, c] {
            self.vertices.push(PbrVertex {
                position: p.to_array(),
                normal: n.to_array(),
                color: mat.color.to_array(),
                metallic: mat.metallic,
                emissive: mat.emissive.to_array(),
                roughness: mat.roughness,
            });
        }
        self.indices.extend_from_slice(&[base, base + 1, base + 2]);
    }

    pub fn push_quad(&mut self, a: Vec3, b: Vec3, c: Vec3, d: Vec3, mat: Material) {
        let n = (b - a).cross(c - a).normalize_or_zero();
        let base = self.vertices.len() as u32;
        for p in [a, b, c, d] {
            self.vertices.push(PbrVertex {
                position: p.to_array(),
                normal: n.to_array(),
                color: mat.color.to_array(),
                metallic: mat.metallic,
                emissive: mat.emissive.to_array(),
                roughness: mat.roughness,
            });
        }
        self.indices
            .extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }

    /// Oriented box centered at `center` with `half` extents and rotation `rot`.
    pub fn add_box(&mut self, center: Vec3, half: Vec3, rot: Quat, mat: Material) {
        let m = Mat4::from_rotation_translation(rot, center);
        let p = |x: f32, y: f32, z: f32| m.transform_point3(Vec3::new(x * half.x, y * half.y, z * half.z));
        let v000 = p(-1.0, -1.0, -1.0);
        let v100 = p(1.0, -1.0, -1.0);
        let v110 = p(1.0, 1.0, -1.0);
        let v010 = p(-1.0, 1.0, -1.0);
        let v001 = p(-1.0, -1.0, 1.0);
        let v101 = p(1.0, -1.0, 1.0);
        let v111 = p(1.0, 1.0, 1.0);
        let v011 = p(-1.0, 1.0, 1.0);

        self.push_quad(v001, v101, v111, v011, mat); // +Z
        self.push_quad(v100, v000, v010, v110, mat); // -Z
        self.push_quad(v101, v100, v110, v111, mat); // +X
        self.push_quad(v000, v001, v011, v010, mat); // -X
        self.push_quad(v011, v111, v110, v010, mat); // +Y
        self.push_quad(v000, v100, v101, v001, mat); // -Y
    }

    /// Tapered box (frustum) from bottom half-extents `(bx, bz)` at `y0` to top `(tx, tz)` at `y1`.
    pub fn add_tapered_box(
        &mut self,
        center: Vec3,
        rot: Quat,
        y0: f32,
        y1: f32,
        bot_xz: (f32, f32),
        top_xz: (f32, f32),
        mat: Material,
    ) {
        let m = Mat4::from_rotation_translation(rot, center);
        let pt = |x: f32, y: f32, z: f32| m.transform_point3(Vec3::new(x, y, z));
        let (bx, bz) = bot_xz;
        let (tx, tz) = top_xz;
        let b0 = pt(-bx, y0, -bz);
        let b1 = pt(bx, y0, -bz);
        let b2 = pt(bx, y0, bz);
        let b3 = pt(-bx, y0, bz);
        let t0 = pt(-tx, y1, -tz);
        let t1 = pt(tx, y1, -tz);
        let t2 = pt(tx, y1, tz);
        let t3 = pt(-tx, y1, tz);

        self.push_quad(b3, b2, t2, t3, mat);
        self.push_quad(b1, b0, t0, t1, mat);
        self.push_quad(b2, b1, t1, t2, mat);
        self.push_quad(b0, b3, t3, t0, mat);
        self.push_quad(t3, t2, t1, t0, mat);
        self.push_quad(b0, b1, b2, b3, mat);
    }

    /// Tapered cylinder / prism between `p0` and `p1`.
    pub fn add_prism(
        &mut self,
        p0: Vec3,
        p1: Vec3,
        r0: f32,
        r1: f32,
        sides: usize,
        mat: Material,
    ) {
        let axis = p1 - p0;
        let len = axis.length();
        if len < 1e-5 {
            return;
        }
        let up = axis / len;
        let ref_v = if up.y.abs() < 0.95 { Vec3::Y } else { Vec3::X };
        let right = up.cross(ref_v).normalize();
        let fwd = right.cross(up).normalize();

        let mut bot = Vec::with_capacity(sides);
        let mut top = Vec::with_capacity(sides);
        for i in 0..sides {
            let a = (i as f32 / sides as f32) * std::f32::consts::TAU;
            let dir = right * a.cos() + fwd * a.sin();
            bot.push(p0 + dir * r0);
            top.push(p1 + dir * r1);
        }

        for i in 0..sides {
            let j = (i + 1) % sides;
            if r0 > 1e-4 && r1 > 1e-4 {
                self.push_quad(bot[i], bot[j], top[j], top[i], mat);
            } else if r1 <= 1e-4 {
                self.push_tri(bot[i], bot[j], p1, mat);
            } else {
                self.push_tri(p0, top[j], top[i], mat);
            }
            if r0 > 1e-4 {
                self.push_tri(p0, bot[j], bot[i], mat);
            }
            if r1 > 1e-4 {
                self.push_tri(p1, top[i], top[j], mat);
            }
        }
    }

    /// Smooth-shaded UV sphere (ellipsoid with `scale`).
    pub fn add_ellipsoid(
        &mut self,
        center: Vec3,
        scale: Vec3,
        rings: usize,
        sectors: usize,
        mat: Material,
    ) {
        let base = self.vertices.len() as u32;
        for r in 0..=rings {
            let v = r as f32 / rings as f32;
            let phi = v * std::f32::consts::PI;
            let sp = phi.sin();
            let cp = phi.cos();
            for s in 0..=sectors {
                let u = s as f32 / sectors as f32;
                let theta = u * std::f32::consts::TAU;
                let dir = Vec3::new(sp * theta.cos(), cp, sp * theta.sin());
                let pos = center + dir * scale;
                let n = (dir / scale.max(Vec3::splat(1e-4))).normalize_or_zero();
                self.vertices.push(PbrVertex {
                    position: pos.to_array(),
                    normal: n.to_array(),
                    color: mat.color.to_array(),
                    metallic: mat.metallic,
                    emissive: mat.emissive.to_array(),
                    roughness: mat.roughness,
                });
            }
        }
        let stride = (sectors + 1) as u32;
        for r in 0..rings as u32 {
            for s in 0..sectors as u32 {
                let i0 = base + r * stride + s;
                let i1 = i0 + 1;
                let i2 = i0 + stride;
                let i3 = i2 + 1;
                self.indices.extend_from_slice(&[i0, i2, i1, i1, i2, i3]);
            }
        }
    }

    /// Torus / glowing ring in XZ plane around `center`.
    pub fn add_torus(
        &mut self,
        center: Vec3,
        rot: Quat,
        major_r: f32,
        minor_r: f32,
        major_segs: usize,
        minor_segs: usize,
        mat: Material,
    ) {
        let m = Mat4::from_rotation_translation(rot, center);
        let base = self.vertices.len() as u32;
        for i in 0..=major_segs {
            let u = (i as f32 / major_segs as f32) * std::f32::consts::TAU;
            let cu = u.cos();
            let su = u.sin();
            for j in 0..=minor_segs {
                let v = (j as f32 / minor_segs as f32) * std::f32::consts::TAU;
                let cv = v.cos();
                let sv = v.sin();
                let local_p = Vec3::new((major_r + minor_r * cv) * cu, minor_r * sv, (major_r + minor_r * cv) * su);
                let local_n = Vec3::new(cv * cu, sv, cv * su).normalize();
                let wp = m.transform_point3(local_p);
                let wn = (rot * local_n).normalize();
                self.vertices.push(PbrVertex {
                    position: wp.to_array(),
                    normal: wn.to_array(),
                    color: mat.color.to_array(),
                    metallic: mat.metallic,
                    emissive: mat.emissive.to_array(),
                    roughness: mat.roughness,
                });
            }
        }
        let stride = (minor_segs + 1) as u32;
        for i in 0..major_segs as u32 {
            for j in 0..minor_segs as u32 {
                let a = base + i * stride + j;
                let b = a + 1;
                let c = a + stride;
                let d = c + 1;
                self.indices.extend_from_slice(&[a, c, b, b, c, d]);
            }
        }
    }
}
