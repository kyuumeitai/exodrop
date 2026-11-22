use exo_core::{smoothstep, BiomeId};
use glam::{Vec2, Vec3};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SurfaceKind {
    Ground,
    MesaTop,
    Cliff,
    LandingPad,
    Lava,
    Ice,
}

/// Procedural planetary heightfield with canyons, stepped mesas, landing plateaus,
/// and natural arena boundary ridges.
#[derive(Clone, Debug)]
pub struct Terrain {
    pub biome: BiomeId,
    pub half_extent: f32,
    pub grid_res: usize,
    pub heights: Vec<f32>,
    pub lava_level: f32,
    pub pad_centers: Vec<(Vec2, f32, f32)>, // (xz, radius, target_height)
}

impl Terrain {
    pub fn generate(biome: BiomeId) -> Self {
        let cfg = biome.config();
        let half_extent = 280.0;
        let grid_res = 161; // 160x160 quads -> 3.5m vertex spacing
        let mut heights = vec![0.0; grid_res * grid_res];

        // Strategic flat plateaus for player drop zone and mission spires
        let pad_centers = vec![
            (Vec2::new(0.0, 75.0), 22.0, 8.0),      // Allied Drop Zone plateau
            (Vec2::new(-95.0, -45.0), 20.0, 22.0),  // West Spire Mesa
            (Vec2::new(92.0, -35.0), 20.0, 26.0),   // East Spire Ridge
            (Vec2::new(0.0, -135.0), 24.0, 18.0),   // North Citadel / Boss Arena
        ];

        for iz in 0..grid_res {
            let vz = iz as f32 / (grid_res - 1) as f32;
            let z = (vz * 2.0 - 1.0) * half_extent;
            for ix in 0..grid_res {
                let vx = ix as f32 / (grid_res - 1) as f32;
                let x = (vx * 2.0 - 1.0) * half_extent;
                let h = eval_biome_height(biome, cfg.seed, x, z, half_extent, &pad_centers);
                heights[iz * grid_res + ix] = h;
            }
        }

        Self {
            biome,
            half_extent,
            grid_res,
            heights,
            lava_level: cfg.lava_level,
            pad_centers,
        }
    }

    #[inline]
    pub fn cell_size(&self) -> f32 {
        (self.half_extent * 2.0) / (self.grid_res - 1) as f32
    }

    /// Bilinear terrain height at any world (x, z).
    pub fn sample_height(&self, x: f32, z: f32) -> f32 {
        let fx = ((x + self.half_extent) / (self.half_extent * 2.0)) * (self.grid_res - 1) as f32;
        let fz = ((z + self.half_extent) / (self.half_extent * 2.0)) * (self.grid_res - 1) as f32;
        let fx = fx.clamp(0.0, (self.grid_res - 1) as f32 - 0.001);
        let fz = fz.clamp(0.0, (self.grid_res - 1) as f32 - 0.001);

        let ix = fx as usize;
        let iz = fz as usize;
        let tx = fx - ix as f32;
        let tz = fz - iz as f32;

        let stride = self.grid_res;
        let h00 = self.heights[iz * stride + ix];
        let h10 = self.heights[iz * stride + ix + 1];
        let h01 = self.heights[(iz + 1) * stride + ix];
        let h11 = self.heights[(iz + 1) * stride + ix + 1];

        let hx0 = h00 + (h10 - h00) * tx;
        let hx1 = h01 + (h11 - h01) * tx;
        hx0 + (hx1 - hx0) * tz
    }

    /// Smooth surface normal via central finite differences.
    pub fn sample_normal(&self, x: f32, z: f32) -> Vec3 {
        let eps = 1.6;
        let hl = self.sample_height(x - eps, z);
        let hr = self.sample_height(x + eps, z);
        let hd = self.sample_height(x, z - eps);
        let hu = self.sample_height(x, z + eps);
        Vec3::new(hl - hr, 2.0 * eps, hd - hu).normalize()
    }

    pub fn surface_kind(&self, x: f32, z: f32) -> SurfaceKind {
        let h = self.sample_height(x, z);
        if h <= self.lava_level + 0.35 {
            return SurfaceKind::Lava;
        }
        for &(center, radius, _) in &self.pad_centers {
            if Vec2::new(x, z).distance(center) < radius * 0.72 {
                return SurfaceKind::LandingPad;
            }
        }
        let n = self.sample_normal(x, z);
        if n.y < 0.78 {
            return SurfaceKind::Cliff;
        }
        if self.biome == BiomeId::EuropaBasin && h < 8.0 {
            return SurfaceKind::Ice;
        }
        if h > 20.0 {
            return SurfaceKind::MesaTop;
        }
        SurfaceKind::Ground
    }

    /// Playable soft radius before canyon rim walls push back.
    pub fn playable_radius(&self) -> f32 {
        self.half_extent - 36.0
    }
}

fn hash2(ix: i32, iz: i32, seed: u64) -> f32 {
    let mut n = (ix as i64).wrapping_mul(374_761_393)
        ^ (iz as i64).wrapping_mul(668_265_263)
        ^ (seed as i64).wrapping_mul(1_442_695_041);
    n = (n ^ (n >> 13)).wrapping_mul(1_274_126_177);
    let u = (n ^ (n >> 16)) as u32;
    (u & 0x00FF_FFFF) as f32 / 16_777_216.0
}

fn value_noise(x: f32, z: f32, seed: u64) -> f32 {
    let ix = x.floor() as i32;
    let iz = z.floor() as i32;
    let fx = x - ix as f32;
    let fz = z - iz as f32;
    let ux = fx * fx * (3.0 - 2.0 * fx);
    let uz = fz * fz * (3.0 - 2.0 * fz);

    let a = hash2(ix, iz, seed);
    let b = hash2(ix + 1, iz, seed);
    let c = hash2(ix, iz + 1, seed);
    let d = hash2(ix + 1, iz + 1, seed);

    let ab = a + (b - a) * ux;
    let cd = c + (d - c) * ux;
    ab + (cd - ab) * uz
}

fn fbm(x: f32, z: f32, seed: u64, octaves: u32) -> f32 {
    let mut sum = 0.0;
    let mut amp = 0.5;
    let mut freq = 1.0;
    let mut norm = 0.0;
    for i in 0..octaves {
        sum += value_noise(x * freq, z * freq, seed.wrapping_add(i as u64 * 97)) * amp;
        norm += amp;
        freq *= 2.03;
        amp *= 0.5;
    }
    sum / norm
}

fn ridged_fbm(x: f32, z: f32, seed: u64, octaves: u32) -> f32 {
    let mut sum = 0.0;
    let mut amp = 0.55;
    let mut freq = 1.0;
    let mut norm = 0.0;
    for i in 0..octaves {
        let v = value_noise(x * freq, z * freq, seed.wrapping_add(i as u64 * 131));
        let ridge = 1.0 - (v * 2.0 - 1.0).abs();
        sum += ridge * ridge * amp;
        norm += amp;
        freq *= 2.1;
        amp *= 0.48;
    }
    sum / norm
}

fn eval_biome_height(
    biome: BiomeId,
    seed: u64,
    x: f32,
    z: f32,
    half_extent: f32,
    pads: &[(Vec2, f32, f32)],
) -> f32 {
    let p = Vec2::new(x, z);
    let dist_center = p.length();

    // Base biome landform
    let mut h = match biome {
        BiomeId::CrimsonMesa => {
            // Dramatic Martian mesas + sweeping central canyon corridor
            let macro_n = fbm(x * 0.0085 + 3.1, z * 0.0085 - 1.7, seed, 5);
            let detail = fbm(x * 0.032, z * 0.032, seed + 55, 3) * 4.5;
            // Quantize into dramatic flat-topped mesas with steep cliffs
            let mesa_raw = (macro_n * 38.0).max(0.0);
            let terrace = (mesa_raw / 11.0).floor() * 11.0
                + smoothstep(0.22, 0.78, (mesa_raw / 11.0).fract()) * 11.0;
            // Carve a sweeping canyon winding through the center
            let canyon_curve = (z * 0.018).sin() * 48.0;
            let canyon_dist = (x - canyon_curve).abs();
            let canyon_cut = (1.0 - smoothstep(14.0, 56.0, canyon_dist)) * 12.0;
            (terrace + detail - canyon_cut).max(2.0)
        }
        BiomeId::EuropaBasin => {
            // Sharp glacial ridges surrounding frozen crevasse lakes
            let ridge = ridged_fbm(x * 0.011 - 2.0, z * 0.011 + 4.0, seed, 5);
            let basin = fbm(x * 0.007, z * 0.007, seed + 19, 4);
            let crags = ridge * 34.0 + (basin - 0.35) * 18.0;
            // Frozen flat ice floor at y = 3.5
            crags.max(3.5) + fbm(x * 0.04, z * 0.04, seed + 88, 2) * 1.4
        }
        BiomeId::IoCaldera => {
            // Volcanic basalt ridges + molten magma channels
            let ridge = ridged_fbm(x * 0.013 + 1.5, z * 0.013 - 2.5, seed, 5);
            let swells = fbm(x * 0.009, z * 0.009, seed + 41, 4);
            // Carve glowing lava channels
            let fissure = ((x * 0.021 + (z * 0.017).sin() * 1.6).sin()).abs();
            let lava_cut = (1.0 - smoothstep(0.06, 0.28, fissure)) * 14.0;
            let raw = ridge * 36.0 + swells * 16.0 - 8.0 - lava_cut;
            raw.max(1.2)
        }
    };

    // Sculpt smooth strategic plateaus for landing pad & objective spires
    for &(center, radius, target_h) in pads {
        let d = p.distance(center);
        let blend = 1.0 - smoothstep(radius * 0.65, radius * 1.75, d);
        h = h + (target_h - h) * blend;
    }

    // Perimeter dramatic canyon wall so arena feels vast & naturally enclosed
    let rim_t = smoothstep(half_extent * 0.68, half_extent * 0.98, dist_center);
    let rim_noise = ridged_fbm(x * 0.018, z * 0.018, seed + 999, 3);
    h += rim_t * rim_t * (58.0 + rim_noise * 42.0);

    h
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terrain_samples_smooth_heights_and_normals() {
        for biome in BiomeId::ALL {
            let t = Terrain::generate(biome);
            let h = t.sample_height(0.0, 75.0);
            assert!((h - 8.0).abs() < 2.0, "drop pad height off for {:?}", biome);
            let n = t.sample_normal(0.0, 75.0);
            assert!((n.length() - 1.0).abs() < 1e-4);
            assert!(n.y > 0.85, "drop pad should be mostly flat");
        }
    }
}
