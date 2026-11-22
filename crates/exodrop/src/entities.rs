use exo_assets::MeshId;
use glam::{Vec3, Vec4};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EnemyKind {
    Swarmer,
    Spitter,
    Interceptor,
    Goliath,
}

impl EnemyKind {
    pub fn max_hp(self) -> f32 {
        match self {
            EnemyKind::Swarmer => 55.0,
            EnemyKind::Spitter => 115.0,
            EnemyKind::Interceptor => 85.0,
            EnemyKind::Goliath => 780.0,
        }
    }

    pub fn hit_radius(self) -> f32 {
        match self {
            EnemyKind::Swarmer => 1.35,
            EnemyKind::Spitter => 1.95,
            EnemyKind::Interceptor => 2.1,
            EnemyKind::Goliath => 3.8,
        }
    }

    pub fn center_y_offset(self) -> f32 {
        match self {
            EnemyKind::Swarmer => 0.85,
            EnemyKind::Spitter => 1.9,
            EnemyKind::Interceptor => 0.0,
            EnemyKind::Goliath => 3.8,
        }
    }

    pub fn score_value(self) -> u32 {
        match self {
            EnemyKind::Swarmer => 100,
            EnemyKind::Spitter => 220,
            EnemyKind::Interceptor => 260,
            EnemyKind::Goliath => 2500,
        }
    }

    pub fn mesh_id(self) -> MeshId {
        match self {
            EnemyKind::Swarmer => MeshId::EnemySwarmer,
            EnemyKind::Spitter => MeshId::EnemySpitter,
            EnemyKind::Interceptor => MeshId::EnemyInterceptor,
            EnemyKind::Goliath => MeshId::EnemyGoliath,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            EnemyKind::Swarmer => "SWARMER",
            EnemyKind::Spitter => "PLASMA SPITTER",
            EnemyKind::Interceptor => "INTERCEPTOR",
            EnemyKind::Goliath => "DREADNOUGHT GOLIATH",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Enemy {
    pub id: u32,
    pub kind: EnemyKind,
    pub pos: Vec3,
    pub vel: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub hp: f32,
    pub max_hp: f32,
    pub fire_timer: f32,
    pub hit_flash: f32,
    pub anim_phase: f32,
    pub strafe_sign: f32,
    pub alive: bool,
}

impl Enemy {
    pub fn center(&self) -> Vec3 {
        self.pos + Vec3::new(0.0, self.kind.center_y_offset(), 0.0)
    }
}

#[derive(Clone, Debug)]
pub struct Spire {
    pub id: u32,
    pub label: &'static str,
    pub pos: Vec3,
    pub hp: f32,
    pub max_hp: f32,
    pub ring_angle: f32,
    pub spawn_timer: f32,
    pub hit_flash: f32,
    pub alive: bool,
}

impl Spire {
    pub fn core_pos(&self) -> Vec3 {
        self.pos + Vec3::new(0.0, 7.6, 0.0)
    }
}

#[derive(Clone, Debug)]
pub struct Wingman {
    pub callsign: &'static str,
    pub pos: Vec3,
    pub vel: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub thruster: f32,
    pub gait_phase: f32,
    pub grounded: bool,
    pub fire_timer: f32,
    pub formation_offset: Vec3,
    pub armor_tint: Vec3,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProjectileKind {
    PlayerPulse,
    WingmanPulse,
    PlayerMissile,
    PlayerMortar,
    EnemyPlasma,
    EnemyHeavy,
}

#[derive(Clone, Debug)]
pub struct Projectile {
    pub kind: ProjectileKind,
    pub pos: Vec3,
    pub vel: Vec3,
    pub damage: f32,
    pub blast_radius: f32,
    pub age: f32,
    pub max_age: f32,
    pub target_enemy_id: Option<u32>,
    pub target_spire_id: Option<u32>,
    pub color: Vec3,
    pub alive: bool,
}

#[derive(Clone, Debug)]
pub struct Particle {
    pub pos: Vec3,
    pub vel: Vec3,
    pub color: Vec4,
    pub size: f32,
    pub age: f32,
    pub max_age: f32,
    pub gravity: f32,
}

#[derive(Clone, Debug)]
pub struct BeamFx {
    pub start: Vec3,
    pub end: Vec3,
    pub color: Vec4,
    pub width: f32,
    pub age: f32,
    pub max_age: f32,
}

#[derive(Clone, Debug)]
pub struct Shockwave {
    pub center: Vec3,
    pub color: Vec3,
    pub max_radius: f32,
    pub age: f32,
    pub max_age: f32,
}

#[derive(Clone, Debug)]
pub struct CommsMsg {
    pub text: String,
    pub color: Vec4,
    pub ttl: f32,
}
