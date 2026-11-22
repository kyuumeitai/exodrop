use crate::models::MeshId;
use exo_core::{BiomeId, Rng};
use exo_physics::{Aabb, Terrain};
use glam::{Mat4, Quat, Vec3};

#[derive(Clone, Debug)]
pub struct StaticProp {
    pub mesh: MeshId,
    pub transform: Mat4,
    pub tint: Vec3,
}

#[derive(Clone, Debug)]
pub struct SpirePlacement {
    pub position: Vec3,
    pub label: &'static str,
}

#[derive(Clone, Debug)]
pub struct WorldLayout {
    pub props: Vec<StaticProp>,
    pub colliders: Vec<Aabb>,
    pub spires: Vec<SpirePlacement>,
    pub player_spawn: Vec3,
    pub wingman_spawns: [Vec3; 2],
    pub boss_spawn: Vec3,
    pub static_lights: Vec<(Vec3, Vec3, f32)>, // (pos, color*intensity, radius)
}

impl WorldLayout {
    pub fn generate(biome: BiomeId, terrain: &Terrain) -> Self {
        let cfg = biome.config();
        let mut rng = Rng::new(cfg.seed ^ 0xA5A5_1337);
        let mut props = Vec::new();
        let mut colliders = Vec::new();
        let mut static_lights = Vec::new();

        // 1. Allied Drop Zone Plateau at (0, 75)
        let pad_pos = Vec3::new(0.0, terrain.sample_height(0.0, 75.0), 75.0);
        props.push(StaticProp {
            mesh: MeshId::LandingPad,
            transform: Mat4::from_translation(pad_pos),
            tint: Vec3::ONE,
        });
        static_lights.push((pad_pos + Vec3::new(0.0, 4.0, 0.0), cfg.accent_glow * 14.0, 34.0));

        // Deployed Orbital Drop Pods around the landing zone
        let pod_offsets = [
            (Vec3::new(-12.5, 0.0, 7.5), 0.4),
            (Vec3::new(13.0, 0.0, 6.0), -0.5),
            (Vec3::new(1.5, 0.0, 14.5), 2.8),
        ];
        for (off, yaw) in pod_offsets {
            let x = pad_pos.x + off.x;
            let z = pad_pos.z + off.z;
            let y = terrain.sample_height(x, z);
            let p = Vec3::new(x, y, z);
            props.push(StaticProp {
                mesh: MeshId::DropPod,
                transform: Mat4::from_rotation_translation(Quat::from_rotation_y(yaw), p),
                tint: Vec3::ONE,
            });
            colliders.push(Aabb::from_center_half(p + Vec3::new(0.0, 1.9, 0.0), Vec3::new(1.4, 1.9, 1.4)));
        }

        // Outpost Bunkers & Comms Tower flanking the drop zone
        let bunker_coords = [
            (Vec3::new(-24.0, 0.0, 58.0), 0.35),
            (Vec3::new(26.0, 0.0, 55.0), -0.40),
            (Vec3::new(-32.0, 0.0, -8.0), 0.75),
            (Vec3::new(38.0, 0.0, -15.0), -0.60),
        ];
        for (c, yaw) in bunker_coords {
            let y = terrain.sample_height(c.x, c.z) - 0.3;
            let p = Vec3::new(c.x, y, c.z);
            props.push(StaticProp {
                mesh: MeshId::OutpostBunker,
                transform: Mat4::from_rotation_translation(Quat::from_rotation_y(yaw), p),
                tint: Vec3::ONE,
            });
            colliders.push(Aabb::from_center_half(
                p + Vec3::new(0.0, 2.4, 0.0),
                Vec3::new(5.6, 2.4, 4.0),
            ));
        }

        let tower_coords = [
            Vec3::new(-18.0, 0.0, 88.0),
            Vec3::new(42.0, 0.0, 32.0),
            Vec3::new(-52.0, 0.0, 20.0),
        ];
        for c in tower_coords {
            let y = terrain.sample_height(c.x, c.z) - 0.3;
            let p = Vec3::new(c.x, y, c.z);
            props.push(StaticProp {
                mesh: MeshId::CommsTower,
                transform: Mat4::from_rotation_translation(Quat::IDENTITY, p),
                tint: Vec3::ONE,
            });
            colliders.push(Aabb::from_center_half(
                p + Vec3::new(0.0, 9.0, 0.0),
                Vec3::new(2.0, 9.0, 2.0),
            ));
            static_lights.push((p + Vec3::new(0.0, 19.2, 0.0), cfg.accent_glow * 12.0, 30.0));
        }

        // 2. Three Alien Spore Spire Objectives on high mesas
        let spire_locs = [
            (-95.0, -45.0, "ALPHA SPIRE"),
            (92.0, -35.0, "BETA SPIRE"),
            (0.0, -135.0, "OMEGA CITADEL SPIRE"),
        ];
        let mut spires = Vec::new();
        for (sx, sz, label) in spire_locs {
            let sy = terrain.sample_height(sx, sz);
            let p = Vec3::new(sx, sy, sz);
            spires.push(SpirePlacement { position: p, label });
            colliders.push(Aabb::from_center_half(
                p + Vec3::new(0.0, 7.5, 0.0),
                Vec3::new(2.6, 7.5, 2.6),
            ));
        }

        // 3. Procedural biome rock / ice-crystal / obsidian crags scattered across canyon
        let rock_tint = match biome {
            BiomeId::CrimsonMesa => Vec3::new(0.68, 0.34, 0.22),
            BiomeId::EuropaBasin => Vec3::new(0.60, 0.86, 0.98),
            BiomeId::IoCaldera => Vec3::new(0.18, 0.16, 0.20),
        };
        for _ in 0..70 {
            let angle = rng.range(0.0, std::f32::consts::TAU);
            let dist = rng.range(28.0, 215.0);
            let x = angle.cos() * dist;
            let z = angle.sin() * dist;
            // Avoid blocking landing pad or spires
            if Vec3::new(x, 0.0, z).distance(Vec3::new(0.0, 0.0, 75.0)) < 26.0 {
                continue;
            }
            if spires
                .iter()
                .any(|s| Vec3::new(x, 0.0, z).distance(Vec3::new(s.position.x, 0.0, s.position.z)) < 22.0)
            {
                continue;
            }
            let y = terrain.sample_height(x, z);
            if y < cfg.lava_level + 1.5 {
                continue;
            }
            let scale = Vec3::new(
                rng.range(0.8, 2.4),
                rng.range(0.9, 3.2),
                rng.range(0.8, 2.4),
            );
            let rot = Quat::from_rotation_y(rng.range(0.0, std::f32::consts::TAU));
            let pos = Vec3::new(x, y - 0.4, z);
            props.push(StaticProp {
                mesh: MeshId::CragRock,
                transform: Mat4::from_scale_rotation_translation(scale, rot, pos),
                tint: rock_tint * rng.range(0.82, 1.15),
            });
            if scale.x > 1.35 {
                colliders.push(Aabb::from_center_half(
                    pos + Vec3::new(0.0, scale.y * 1.5, 0.0),
                    Vec3::new(scale.x * 1.6, scale.y * 1.5, scale.z * 1.6),
                ));
            }
        }

        let player_spawn = Vec3::new(0.0, pad_pos.y + 0.65, 73.0);
        let wingman_spawns = [
            Vec3::new(-6.5, terrain.sample_height(-6.5, 76.0) + 0.65, 76.0),
            Vec3::new(6.5, terrain.sample_height(6.5, 76.0) + 0.65, 76.0),
        ];
        let boss_spawn = Vec3::new(0.0, terrain.sample_height(0.0, -110.0), -110.0);

        Self {
            props,
            colliders,
            spires,
            player_spawn,
            wingman_spawns,
            boss_spawn,
            static_lights,
        }
    }
}
