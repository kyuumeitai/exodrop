use crate::builder::{MeshData, PbrVertex};
use exo_core::{smoothstep, BiomeId};
use exo_physics::Terrain;
use glam::{Vec2, Vec3};

/// Generates a rich PBR vertex-colored planetary terrain mesh matching `Terrain` heightfield.
pub fn build_terrain_mesh(terrain: &Terrain) -> MeshData {
    let cfg = terrain.biome.config();
    let res = terrain.grid_res;
    let mut vertices = Vec::with_capacity(res * res);
    let mut indices = Vec::with_capacity((res - 1) * (res - 1) * 6);

    for iz in 0..res {
        let vz = iz as f32 / (res - 1) as f32;
        let z = (vz * 2.0 - 1.0) * terrain.half_extent;
        for ix in 0..res {
            let vx = ix as f32 / (res - 1) as f32;
            let x = (vx * 2.0 - 1.0) * terrain.half_extent;
            let h = terrain.heights[iz * res + ix];
            let n = terrain.sample_normal(x, z);

            // Height & slope stratification for crisp geological realism
            let h_t = ((h - 2.0) / 34.0).clamp(0.0, 1.0);
            let strata = ((h * 0.45).sin() * 0.5 + 0.5) * 0.14;
            let slope = 1.0 - n.y.clamp(0.0, 1.0);
            let cliff_blend = smoothstep(0.14, 0.36, slope);

            let base_col = if h_t < 0.5 {
                cfg.terrain_low.lerp(cfg.terrain_mid, h_t * 2.0)
            } else {
                cfg.terrain_mid.lerp(cfg.terrain_high, (h_t - 0.5) * 2.0)
            };
            let mut color = base_col.lerp(cfg.terrain_cliff, cliff_blend) * (0.93 + strata);
            let mut metallic = 0.04_f32;
            let mut roughness = 0.86_f32;
            let mut emissive = Vec3::ZERO;

            // Biome special materials
            match terrain.biome {
                BiomeId::EuropaBasin => {
                    // Gleaming frozen glacial floor
                    let ice_t = 1.0 - smoothstep(3.8, 7.5, h);
                    roughness = 0.82 - ice_t * 0.62;
                    metallic = 0.04 + ice_t * 0.22;
                    color = color.lerp(Vec3::new(0.28, 0.66, 0.86), ice_t * 0.65);
                }
                BiomeId::IoCaldera => {
                    // Obsidian shimmer + glowing molten lava channels!
                    metallic = 0.24 * (1.0 - cliff_blend * 0.5);
                    roughness = 0.52;
                    let lava_t = 1.0 - smoothstep(terrain.lava_level - 0.1, terrain.lava_level + 1.4, h);
                    if lava_t > 0.01 {
                        let pulse = ((x * 0.18).sin() * (z * 0.18).cos() * 0.5 + 0.5) * 0.45 + 0.75;
                        color = color.lerp(Vec3::new(1.0, 0.32, 0.05), lava_t);
                        emissive = Vec3::new(4.8, 1.25, 0.15) * lava_t * lava_t * pulse;
                        roughness = 0.22;
                    }
                }
                BiomeId::CrimsonMesa => {
                    // Subtle warm desert mineral highlight on mesa tops
                    if h > 22.0 && slope < 0.15 {
                        roughness = 0.78;
                    }
                }
            }

            // Subtle scorched/graveled ring around tactical plateaus
            let p = Vec2::new(x, z);
            for &(center, radius, _) in &terrain.pad_centers {
                let d = p.distance(center);
                if d < radius * 0.78 {
                    let pad_t = 1.0 - smoothstep(radius * 0.55, radius * 0.78, d);
                    color = color.lerp(Vec3::new(0.24, 0.27, 0.31), pad_t * 0.55);
                    roughness = roughness * (1.0 - pad_t * 0.35);
                }
            }

            vertices.push(PbrVertex {
                position: [x, h, z],
                normal: n.to_array(),
                color: color.to_array(),
                metallic,
                emissive: emissive.to_array(),
                roughness,
            });
        }
    }

    for iz in 0..(res - 1) as u32 {
        for ix in 0..(res - 1) as u32 {
            let a = iz * res as u32 + ix;
            let b = a + 1;
            let c = a + res as u32;
            let d = c + 1;
            indices.extend_from_slice(&[a, c, b, b, c, d]);
        }
    }

    MeshData { vertices, indices }
}
