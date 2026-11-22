use glam::{Vec2, Vec3};
use std::collections::HashSet;

/// Fixed physics simulation step (120 Hz) for rock-solid jetpack & collision feel.
pub const FIXED_DT: f32 = 1.0 / 120.0;

/// Fast, deterministic XorShift64* PRNG for procedural worlds, particles, and AI.
#[derive(Clone, Debug)]
pub struct Rng {
    state: u64,
}

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self {
            state: if seed == 0 { 0x9E37_79B9_7F4A_7C15 } else { seed },
        }
    }

    #[inline]
    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.state = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// Uniform float in [0.0, 1.0).
    #[inline]
    pub fn f32(&mut self) -> f32 {
        ((self.next_u64() >> 40) as f32) * (1.0 / 16_777_216.0)
    }

    /// Uniform float in [min, max).
    #[inline]
    pub fn range(&mut self, min: f32, max: f32) -> f32 {
        min + (max - min) * self.f32()
    }

    /// Random unit vector on the sphere.
    pub fn unit_vec3(&mut self) -> Vec3 {
        let z = self.range(-1.0, 1.0);
        let a = self.range(0.0, std::f32::consts::TAU);
        let r = (1.0 - z * z).max(0.0).sqrt();
        Vec3::new(r * a.cos(), z, r * a.sin())
    }

    /// Random horizontal unit vector.
    pub fn unit_xz(&mut self) -> Vec3 {
        let a = self.range(0.0, std::f32::consts::TAU);
        Vec3::new(a.cos(), 0.0, a.sin())
    }
}

/// Fixed-timestep accumulator clock.
#[derive(Clone, Debug)]
pub struct SimClock {
    pub accumulator: f32,
    pub total_time: f32,
    pub step_count: u64,
}

impl Default for SimClock {
    fn default() -> Self {
        Self {
            accumulator: 0.0,
            total_time: 0.0,
            step_count: 0,
        }
    }
}

impl SimClock {
    pub fn advance(&mut self, frame_dt: f32) -> u32 {
        let clamped = frame_dt.clamp(0.0, 0.05);
        self.accumulator += clamped;
        let mut steps = 0;
        while self.accumulator >= FIXED_DT && steps < 6 {
            self.accumulator -= FIXED_DT;
            self.total_time += FIXED_DT;
            self.step_count += 1;
            steps += 1;
        }
        steps
    }
}

/// The three playable planetary sectors in EXODROP.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BiomeId {
    CrimsonMesa = 0,
    EuropaBasin = 1,
    IoCaldera = 2,
}

impl BiomeId {
    pub const ALL: [BiomeId; 3] = [
        BiomeId::CrimsonMesa,
        BiomeId::EuropaBasin,
        BiomeId::IoCaldera,
    ];

    pub fn from_index(idx: usize) -> Self {
        Self::ALL[idx % Self::ALL.len()]
    }

    pub fn next(self) -> Self {
        Self::from_index(self as usize + 1)
    }

    pub fn config(self) -> BiomeConfig {
        match self {
            BiomeId::CrimsonMesa => BiomeConfig {
                id: self,
                code: "SECTOR 01",
                name: "ARES CRIMSON MESA",
                subtitle: "ARID CANYON RELAY // 0.85G",
                seed: 2026_01,
                gravity: 17.5,
                sun_dir: Vec3::new(0.58, 0.56, -0.59).normalize(),
                sun_color: Vec3::new(1.0, 0.86, 0.68) * 3.1,
                sky_zenith: Vec3::new(0.05, 0.08, 0.22),
                sky_horizon: Vec3::new(0.86, 0.44, 0.26),
                ground_bounce: Vec3::new(0.32, 0.14, 0.09),
                fog_color: Vec3::new(0.76, 0.40, 0.26),
                fog_density: 0.0022,
                planet_color: Vec3::new(0.85, 0.68, 0.48),
                ring_color: Vec3::new(0.95, 0.82, 0.65),
                aurora_strength: 0.0,
                lava_level: -100.0,
                terrain_low: Vec3::new(0.42, 0.19, 0.11),
                terrain_mid: Vec3::new(0.62, 0.29, 0.17),
                terrain_high: Vec3::new(0.76, 0.43, 0.27),
                terrain_cliff: Vec3::new(0.30, 0.16, 0.12),
                accent_glow: Vec3::new(0.18, 0.95, 1.0),
                spire_glow: Vec3::new(0.95, 0.20, 0.45),
            },
            BiomeId::EuropaBasin => BiomeConfig {
                id: self,
                code: "SECTOR 02",
                name: "EUROPA GLACIAL RIFT",
                subtitle: "CRYO ICE SHELF // 0.68G LOW-G",
                seed: 2026_02,
                gravity: 14.2,
                sun_dir: Vec3::new(-0.50, 0.46, -0.73).normalize(),
                sun_color: Vec3::new(0.82, 0.93, 1.0) * 2.8,
                sky_zenith: Vec3::new(0.015, 0.03, 0.10),
                sky_horizon: Vec3::new(0.24, 0.56, 0.78),
                ground_bounce: Vec3::new(0.14, 0.26, 0.38),
                fog_color: Vec3::new(0.26, 0.52, 0.72),
                fog_density: 0.0020,
                planet_color: Vec3::new(0.84, 0.58, 0.36),
                ring_color: Vec3::new(0.65, 0.85, 0.98),
                aurora_strength: 1.0,
                lava_level: -100.0,
                terrain_low: Vec3::new(0.22, 0.46, 0.64),
                terrain_mid: Vec3::new(0.62, 0.82, 0.94),
                terrain_high: Vec3::new(0.88, 0.95, 0.99),
                terrain_cliff: Vec3::new(0.14, 0.22, 0.33),
                accent_glow: Vec3::new(0.22, 1.0, 0.85),
                spire_glow: Vec3::new(0.65, 0.18, 1.0),
            },
            BiomeId::IoCaldera => BiomeConfig {
                id: self,
                code: "SECTOR 03",
                name: "IO OBSIDIAN CALDERA",
                subtitle: "VOLCANIC SIEGE // 1.05G",
                seed: 2026_03,
                gravity: 20.5,
                sun_dir: Vec3::new(0.40, 0.42, -0.81).normalize(),
                sun_color: Vec3::new(1.0, 0.64, 0.38) * 2.9,
                sky_zenith: Vec3::new(0.04, 0.02, 0.06),
                sky_horizon: Vec3::new(0.55, 0.20, 0.10),
                ground_bounce: Vec3::new(0.45, 0.14, 0.04),
                fog_color: Vec3::new(0.42, 0.16, 0.09),
                fog_density: 0.0026,
                planet_color: Vec3::new(0.92, 0.62, 0.30),
                ring_color: Vec3::new(0.98, 0.55, 0.24),
                aurora_strength: 0.25,
                lava_level: 1.8,
                terrain_low: Vec3::new(0.12, 0.10, 0.11),
                terrain_mid: Vec3::new(0.20, 0.17, 0.18),
                terrain_high: Vec3::new(0.36, 0.28, 0.22),
                terrain_cliff: Vec3::new(0.09, 0.08, 0.10),
                accent_glow: Vec3::new(1.0, 0.45, 0.08),
                spire_glow: Vec3::new(1.0, 0.28, 0.05),
            },
        }
    }
}

#[derive(Clone, Debug)]
pub struct BiomeConfig {
    pub id: BiomeId,
    pub code: &'static str,
    pub name: &'static str,
    pub subtitle: &'static str,
    pub seed: u64,
    pub gravity: f32,
    pub sun_dir: Vec3,
    pub sun_color: Vec3,
    pub sky_zenith: Vec3,
    pub sky_horizon: Vec3,
    pub ground_bounce: Vec3,
    pub fog_color: Vec3,
    pub fog_density: f32,
    pub planet_color: Vec3,
    pub ring_color: Vec3,
    pub aurora_strength: f32,
    pub lava_level: f32,
    pub terrain_low: Vec3,
    pub terrain_mid: Vec3,
    pub terrain_high: Vec3,
    pub terrain_cliff: Vec3,
    pub accent_glow: Vec3,
    pub spire_glow: Vec3,
}

/// Player weapon classes in EXODROP.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum WeaponKind {
    PulseCarbine = 0,
    RailLance = 1,
    MicroMissiles = 2,
    ArcMortar = 3,
}

impl WeaponKind {
    pub const ALL: [WeaponKind; 4] = [
        WeaponKind::PulseCarbine,
        WeaponKind::RailLance,
        WeaponKind::MicroMissiles,
        WeaponKind::ArcMortar,
    ];

    pub fn from_index(i: usize) -> Self {
        Self::ALL[i % Self::ALL.len()]
    }

    pub fn name(self) -> &'static str {
        match self {
            WeaponKind::PulseCarbine => "VX-9 PULSE CARBINE",
            WeaponKind::RailLance => "HELIOS RAIL-LANCE",
            WeaponKind::MicroMissiles => "VALKYRIE SWARM POD",
            WeaponKind::ArcMortar => "NOVA ARC-MORTAR",
        }
    }

    pub fn short_name(self) -> &'static str {
        match self {
            WeaponKind::PulseCarbine => "PULSE",
            WeaponKind::RailLance => "RAIL",
            WeaponKind::MicroMissiles => "SWARM",
            WeaponKind::ArcMortar => "MORTAR",
        }
    }

    pub fn cooldown(self) -> f32 {
        match self {
            WeaponKind::PulseCarbine => 0.105,
            WeaponKind::RailLance => 0.85,
            WeaponKind::MicroMissiles => 0.95,
            WeaponKind::ArcMortar => 0.90,
        }
    }

    pub fn accent_color(self) -> Vec3 {
        match self {
            WeaponKind::PulseCarbine => Vec3::new(0.20, 0.92, 1.0),
            WeaponKind::RailLance => Vec3::new(1.0, 0.86, 0.25),
            WeaponKind::MicroMissiles => Vec3::new(1.0, 0.42, 0.16),
            WeaponKind::ArcMortar => Vec3::new(0.66, 0.36, 1.0),
        }
    }
}

/// Tactical orders for AI wingmen Echo-2 and Echo-3.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SquadOrder {
    Follow,
    AttackTarget,
    HoldPosition,
}

impl SquadOrder {
    pub fn label(self) -> &'static str {
        match self {
            SquadOrder::Follow => "FORMATION",
            SquadOrder::AttackTarget => "ENGAGE TARGET",
            SquadOrder::HoldPosition => "HOLD SECTOR",
        }
    }
}

/// Logical key codes decoupled from winit so engine crates stay independent.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Key {
    W,
    A,
    S,
    D,
    Space,
    Shift,
    Ctrl,
    C,
    E,
    R,
    V,
    Q,
    F,
    N,
    Tab,
    Enter,
    Escape,
    Digit1,
    Digit2,
    Digit3,
    Digit4,
    F1,
    F2,
    F3,
    F11,
}

/// Frame input snapshot with held and just-pressed state.
#[derive(Clone, Debug, Default)]
pub struct InputState {
    pub keys_down: HashSet<Key>,
    pub keys_pressed: HashSet<Key>,
    pub analog_move: Vec2,
    pub mouse_left: bool,
    pub mouse_left_pressed: bool,
    pub mouse_left_released: bool,
    pub mouse_right: bool,
    pub mouse_delta: Vec2,
    pub scroll_delta: f32,
}

impl InputState {
    pub fn key_down(&self, k: Key) -> bool {
        self.keys_down.contains(&k)
    }

    pub fn key_pressed(&self, k: Key) -> bool {
        self.keys_pressed.contains(&k)
    }

    pub fn on_key(&mut self, k: Key, down: bool) {
        if down {
            if !self.keys_down.contains(&k) {
                self.keys_pressed.insert(k);
            }
            self.keys_down.insert(k);
        } else {
            self.keys_down.remove(&k);
        }
    }

    pub fn on_mouse_button(&mut self, left: bool, down: bool) {
        if left {
            if down && !self.mouse_left {
                self.mouse_left_pressed = true;
            }
            if !down && self.mouse_left {
                self.mouse_left_released = true;
            }
            self.mouse_left = down;
        } else {
            self.mouse_right = down;
        }
    }

    pub fn end_frame(&mut self) {
        self.keys_pressed.clear();
        self.mouse_left_pressed = false;
        self.mouse_left_released = false;
        self.mouse_delta = Vec2::ZERO;
        self.scroll_delta = 0.0;
    }
}

/// Smooth exponential damping helper (framerate-independent).
#[inline]
pub fn damp(current: f32, target: f32, lambda: f32, dt: f32) -> f32 {
    current + (target - current) * (1.0 - (-lambda * dt).exp())
}

#[inline]
pub fn damp_vec3(current: Vec3, target: Vec3, lambda: f32, dt: f32) -> Vec3 {
    current + (target - current) * (1.0 - (-lambda * dt).exp())
}

#[inline]
pub fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rng_is_deterministic_and_bounded() {
        let mut a = Rng::new(42);
        let mut b = Rng::new(42);
        for _ in 0..100 {
            let va = a.f32();
            let vb = b.f32();
            assert_eq!(va, vb);
            assert!((0.0..1.0).contains(&va));
        }
    }

    #[test]
    fn sim_clock_steps_fixed_rate() {
        let mut clock = SimClock::default();
        let steps = clock.advance(1.0 / 60.0);
        assert_eq!(steps, 2);
        assert_eq!(clock.step_count, 2);
    }

    #[test]
    fn biomes_have_normalized_sun_dirs() {
        for b in BiomeId::ALL {
            let cfg = b.config();
            assert!((cfg.sun_dir.length() - 1.0).abs() < 1e-4);
        }
    }
}
