use crate::builder::{Material, MeshBuilder, MeshData};
use glam::{Quat, Vec3};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(usize)]
pub enum MeshId {
    ExoTorso = 0,
    ExoArm = 1,
    ExoLeg = 2,
    WeaponCarbine = 3,
    WeaponRail = 4,
    WeaponMissile = 5,
    WeaponMortar = 6,
    ThrusterPlume = 7,
    EnemySwarmer = 8,
    EnemySpitter = 9,
    EnemyInterceptor = 10,
    EnemyGoliath = 11,
    DropPod = 12,
    SporeSpire = 13,
    SpireRing = 14,
    OutpostBunker = 15,
    CommsTower = 16,
    LandingPad = 17,
    CragRock = 18,
    ProjectileBolt = 19,
    Missile = 20,
    GlowSphere = 21,
    ShockRing = 22,
}

impl MeshId {
    pub const COUNT: usize = 23;
}

pub struct AssetCatalog {
    pub meshes: Vec<MeshData>,
}

impl AssetCatalog {
    pub fn generate_all() -> Self {
        let mut meshes = Vec::with_capacity(MeshId::COUNT);
        meshes.push(build_exo_torso());
        meshes.push(build_exo_arm());
        meshes.push(build_exo_leg());
        meshes.push(build_weapon_carbine());
        meshes.push(build_weapon_rail());
        meshes.push(build_weapon_missile());
        meshes.push(build_weapon_mortar());
        meshes.push(build_thruster_plume());
        meshes.push(build_enemy_swarmer());
        meshes.push(build_enemy_spitter());
        meshes.push(build_enemy_interceptor());
        meshes.push(build_enemy_goliath());
        meshes.push(build_drop_pod());
        meshes.push(build_spore_spire());
        meshes.push(build_spire_ring());
        meshes.push(build_outpost_bunker());
        meshes.push(build_comms_tower());
        meshes.push(build_landing_pad());
        meshes.push(build_crag_rock());
        meshes.push(build_projectile_bolt());
        meshes.push(build_missile());
        meshes.push(build_glow_sphere());
        meshes.push(build_shock_ring());
        debug_assert_eq!(meshes.len(), MeshId::COUNT);
        Self { meshes }
    }
}

/// Sleek 2026 Orbital Exo-Suit upper torso, helmet, visor, and twin angled jetpack thrusters.
/// Origin is at waist/pelvis (y = 0.0 -> head top ~ 0.95). Faces -Z.
fn build_exo_torso() -> MeshData {
    let mut b = MeshBuilder::new();
    let armor_main = Material::metal(Vec3::new(0.84, 0.87, 0.92), 0.62, 0.30);
    let armor_dark = Material::metal(Vec3::new(0.14, 0.16, 0.21), 0.78, 0.36);
    let accent_trim = Material::metal(Vec3::new(0.92, 0.44, 0.12), 0.55, 0.32);
    let visor_glow = Material::glow(Vec3::new(0.25, 0.95, 1.0), Vec3::new(0.35, 2.8, 3.6));
    let nozzle_glow = Material::glow(Vec3::new(0.15, 0.82, 1.0), Vec3::new(0.25, 2.2, 3.8));

    // Pelvis & armored waist core
    b.add_tapered_box(Vec3::ZERO, Quat::IDENTITY, -0.05, 0.18, (0.20, 0.15), (0.24, 0.18), armor_dark);
    // Broad V-tapered tactical chestplate
    b.add_tapered_box(Vec3::ZERO, Quat::IDENTITY, 0.18, 0.58, (0.22, 0.17), (0.34, 0.22), armor_main);
    // Sloped front sternum armor & glowing reactor core
    b.add_box(Vec3::new(0.0, 0.41, -0.20), Vec3::new(0.18, 0.13, 0.06), Quat::from_rotation_x(-0.18), armor_main);
    b.add_box(Vec3::new(0.0, 0.43, -0.24), Vec3::new(0.075, 0.045, 0.03), Quat::IDENTITY, visor_glow);
    // Orange shoulder collar trim
    b.add_box(Vec3::new(0.0, 0.58, -0.02), Vec3::new(0.26, 0.045, 0.18), Quat::IDENTITY, accent_trim);

    // Armored neck & sculpted sci-fi combat helmet
    b.add_prism(Vec3::new(0.0, 0.58, 0.0), Vec3::new(0.0, 0.68, -0.01), 0.10, 0.09, 8, armor_dark);
    b.add_tapered_box(Vec3::new(0.0, 0.67, -0.02), Quat::IDENTITY, 0.0, 0.27, (0.145, 0.165), (0.125, 0.145), armor_main);
    // Helmet crest & comms fin
    b.add_box(Vec3::new(0.0, 0.94, 0.02), Vec3::new(0.035, 0.04, 0.14), Quat::IDENTITY, accent_trim);
    b.add_prism(Vec3::new(0.14, 0.80, 0.06), Vec3::new(0.17, 1.04, 0.12), 0.014, 0.006, 6, armor_dark);
    // Sleek recessed glowing panoramic visor (-Z face)
    b.add_box(Vec3::new(0.0, 0.80, -0.155), Vec3::new(0.125, 0.055, 0.035), Quat::from_rotation_x(0.10), visor_glow);
    // Chin guard
    b.add_box(Vec3::new(0.0, 0.71, -0.145), Vec3::new(0.095, 0.045, 0.05), Quat::from_rotation_x(-0.22), armor_dark);

    // --- TWIN VECTOR-THRUST JETPACK WING-PODS (+Z back) ---
    b.add_box(Vec3::new(0.0, 0.42, 0.24), Vec3::new(0.17, 0.22, 0.11), Quat::IDENTITY, armor_dark);
    b.add_box(Vec3::new(0.0, 0.45, 0.34), Vec3::new(0.06, 0.12, 0.025), Quat::IDENTITY, visor_glow);

    for side in [-1.0_f32, 1.0_f32] {
        let pod_rot = Quat::from_rotation_z(side * -0.16) * Quat::from_rotation_x(0.14);
        // Main turbine housing
        b.add_box(Vec3::new(side * 0.24, 0.44, 0.29), Vec3::new(0.105, 0.23, 0.12), pod_rot, armor_main);
        // Swept aero winglet fin!
        let fin_rot = Quat::from_rotation_z(side * -0.45) * Quat::from_rotation_y(side * -0.22);
        b.add_box(Vec3::new(side * 0.46, 0.56, 0.34), Vec3::new(0.20, 0.028, 0.11), fin_rot, accent_trim);
        // Intake scoop top
        b.add_box(Vec3::new(side * 0.24, 0.67, 0.27), Vec3::new(0.08, 0.05, 0.09), pod_rot, armor_dark);
        // Thruster bell nozzle bottom
        let noz_top = Vec3::new(side * 0.22, 0.22, 0.31);
        let noz_bot = Vec3::new(side * 0.24, 0.04, 0.36);
        b.add_prism(noz_top, noz_bot, 0.075, 0.105, 10, armor_dark);
        b.add_prism(noz_bot + Vec3::new(0.0, 0.03, 0.0), noz_bot - Vec3::new(0.0, 0.01, 0.0), 0.07, 0.078, 10, nozzle_glow);
    }

    b.build()
}

/// Exo-suit arm extending downward from shoulder pivot (0, 0, 0) to hand (0, -0.64, 0).
fn build_exo_arm() -> MeshData {
    let mut b = MeshBuilder::new();
    let armor = Material::metal(Vec3::new(0.84, 0.87, 0.92), 0.62, 0.30);
    let dark = Material::metal(Vec3::new(0.15, 0.17, 0.22), 0.78, 0.36);
    let trim = Material::metal(Vec3::new(0.92, 0.44, 0.12), 0.55, 0.32);

    // Armored shoulder pauldron
    b.add_box(Vec3::new(0.0, 0.03, 0.0), Vec3::new(0.12, 0.10, 0.13), Quat::from_rotation_z(-0.2), trim);
    // Upper arm
    b.add_prism(Vec3::new(0.0, -0.02, 0.0), Vec3::new(0.0, -0.32, 0.0), 0.075, 0.068, 8, dark);
    // Forearm gauntlet
    b.add_tapered_box(Vec3::new(0.0, -0.32, 0.0), Quat::IDENTITY, -0.30, 0.0, (0.062, 0.068), (0.082, 0.086), armor);
    b.build()
}

/// Exo-suit leg extending downward from hip pivot (0, 0, 0) to boot sole (0, -0.96, 0).
fn build_exo_leg() -> MeshData {
    let mut b = MeshBuilder::new();
    let armor = Material::metal(Vec3::new(0.84, 0.87, 0.92), 0.62, 0.30);
    let dark = Material::metal(Vec3::new(0.14, 0.16, 0.21), 0.78, 0.36);
    let boot_glow = Material::glow(Vec3::new(0.2, 0.9, 1.0), Vec3::new(0.2, 1.8, 2.8));

    // Thigh armor
    b.add_tapered_box(Vec3::new(0.0, -0.44, 0.0), Quat::IDENTITY, 0.0, 0.44, (0.085, 0.095), (0.115, 0.12), armor);
    // Knee guard
    b.add_box(Vec3::new(0.0, -0.46, -0.08), Vec3::new(0.08, 0.07, 0.05), Quat::IDENTITY, dark);
    // Shin greave
    b.add_tapered_box(Vec3::new(0.0, -0.88, 0.01), Quat::IDENTITY, 0.0, 0.42, (0.078, 0.095), (0.092, 0.105), dark);
    // Heavy jump-boot
    b.add_box(Vec3::new(0.0, -0.91, -0.05), Vec3::new(0.095, 0.065, 0.16), Quat::IDENTITY, armor);
    // Sole micro-thruster glow
    b.add_box(Vec3::new(0.0, -0.965, 0.01), Vec3::new(0.06, 0.015, 0.07), Quat::IDENTITY, boot_glow);
    b.build()
}

fn build_weapon_carbine() -> MeshData {
    let mut b = MeshBuilder::new();
    let body = Material::metal(Vec3::new(0.18, 0.20, 0.25), 0.80, 0.30);
    let trim = Material::metal(Vec3::new(0.75, 0.78, 0.82), 0.70, 0.28);
    let glow = Material::glow(Vec3::new(0.2, 0.95, 1.0), Vec3::new(0.3, 2.5, 3.6));

    b.add_box(Vec3::new(0.0, 0.02, -0.25), Vec3::new(0.048, 0.068, 0.32), Quat::IDENTITY, body);
    b.add_box(Vec3::new(0.0, 0.07, -0.22), Vec3::new(0.036, 0.025, 0.24), Quat::IDENTITY, trim);
    b.add_prism(Vec3::new(0.0, 0.03, -0.55), Vec3::new(0.0, 0.03, -0.82), 0.024, 0.020, 8, trim);
    b.add_box(Vec3::new(0.0, 0.04, -0.44), Vec3::new(0.052, 0.016, 0.16), Quat::IDENTITY, glow);
    b.build()
}

fn build_weapon_rail() -> MeshData {
    let mut b = MeshBuilder::new();
    let body = Material::metal(Vec3::new(0.16, 0.18, 0.22), 0.82, 0.26);
    let gold = Material::glow(Vec3::new(1.0, 0.86, 0.28), Vec3::new(3.5, 2.6, 0.5));
    b.add_box(Vec3::new(0.0, 0.03, -0.30), Vec3::new(0.055, 0.075, 0.38), Quat::IDENTITY, body);
    // Twin parallel accelerator rails
    b.add_box(Vec3::new(-0.04, 0.035, -0.86), Vec3::new(0.016, 0.035, 0.45), Quat::IDENTITY, body);
    b.add_box(Vec3::new(0.04, 0.035, -0.86), Vec3::new(0.016, 0.035, 0.45), Quat::IDENTITY, body);
    b.add_box(Vec3::new(0.0, 0.035, -0.82), Vec3::new(0.018, 0.018, 0.42), Quat::IDENTITY, gold);
    b.build()
}

fn build_weapon_missile() -> MeshData {
    let mut b = MeshBuilder::new();
    let pod = Material::metal(Vec3::new(0.24, 0.27, 0.30), 0.70, 0.35);
    let warn = Material::glow(Vec3::new(1.0, 0.42, 0.15), Vec3::new(3.2, 1.1, 0.2));
    b.add_box(Vec3::new(0.0, 0.05, -0.28), Vec3::new(0.11, 0.11, 0.32), Quat::IDENTITY, pod);
    for ix in [-0.045, 0.045] {
        for iy in [0.005, 0.095] {
            b.add_prism(Vec3::new(ix, iy, -0.58), Vec3::new(ix, iy, -0.63), 0.03, 0.025, 8, warn);
        }
    }
    b.build()
}

fn build_weapon_mortar() -> MeshData {
    let mut b = MeshBuilder::new();
    let body = Material::metal(Vec3::new(0.20, 0.18, 0.26), 0.72, 0.32);
    let violet = Material::glow(Vec3::new(0.70, 0.35, 1.0), Vec3::new(2.6, 0.9, 4.0));
    b.add_prism(Vec3::new(0.0, 0.04, -0.10), Vec3::new(0.0, 0.04, -0.70), 0.085, 0.095, 10, body);
    b.add_torus(Vec3::new(0.0, 0.04, -0.55), Quat::from_rotation_x(std::f32::consts::FRAC_PI_2), 0.105, 0.022, 12, 6, violet);
    b.build()
}

/// Layered additive-friendly thruster exhaust plume cone pointing downward (-Y).
fn build_thruster_plume() -> MeshData {
    let mut b = MeshBuilder::new();
    let core = Material::glow(Vec3::new(0.85, 0.97, 1.0), Vec3::new(3.0, 4.5, 6.0));
    let outer = Material::glow(Vec3::new(0.18, 0.75, 1.0), Vec3::new(0.6, 2.2, 4.2));
    b.add_prism(Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.0, -0.75, 0.0), 0.065, 0.005, 8, core);
    b.add_prism(Vec3::new(0.0, -0.02, 0.0), Vec3::new(0.0, -1.35, 0.0), 0.105, 0.015, 8, outer);
    b.build()
}

/// Alien Chitin Swarmer — fast 6-legged bio-mech arachnid predator. Faces -Z.
fn build_enemy_swarmer() -> MeshData {
    let mut b = MeshBuilder::new();
    let chitin = Material::metal(Vec3::new(0.14, 0.11, 0.17), 0.45, 0.38);
    let plate = Material::metal(Vec3::new(0.32, 0.10, 0.18), 0.35, 0.42);
    let acid = Material::glow(Vec3::new(0.35, 1.0, 0.25), Vec3::new(1.2, 4.0, 0.6));

    // Segmented armored thorax & abdomen
    b.add_ellipsoid(Vec3::new(0.0, 0.75, 0.35), Vec3::new(0.52, 0.38, 0.72), 7, 10, chitin);
    b.add_ellipsoid(Vec3::new(0.0, 0.72, -0.35), Vec3::new(0.42, 0.32, 0.50), 6, 9, plate);
    // Glowing dorsal bio-vents
    b.add_box(Vec3::new(0.0, 1.06, 0.15), Vec3::new(0.12, 0.08, 0.38), Quat::IDENTITY, acid);
    // Menacing glowing optic cluster + scythe mandibles
    b.add_box(Vec3::new(-0.14, 0.78, -0.82), Vec3::new(0.09, 0.055, 0.06), Quat::IDENTITY, acid);
    b.add_box(Vec3::new(0.14, 0.78, -0.82), Vec3::new(0.09, 0.055, 0.06), Quat::IDENTITY, acid);
    for s in [-1.0_f32, 1.0] {
        b.add_prism(Vec3::new(s * 0.24, 0.62, -0.75), Vec3::new(s * 0.10, 0.44, -1.25), 0.08, 0.015, 6, plate);
        // 3 articulated legs per side
        for (idx, z_off) in [-0.35_f32, 0.08, 0.52].iter().enumerate() {
            let knee = Vec3::new(s * (0.95 + idx as f32 * 0.05), 1.05, *z_off);
            let foot = Vec3::new(s * 1.15, 0.0, z_off - 0.12 + idx as f32 * 0.18);
            let hip = Vec3::new(s * 0.36, 0.65, *z_off);
            b.add_prism(hip, knee, 0.075, 0.055, 6, chitin);
            b.add_prism(knee, foot, 0.055, 0.02, 6, plate);
        }
    }
    b.build()
}

/// Alien Plasma Spitter — tri-pedal artillery walker with pulsing plasma sac.
fn build_enemy_spitter() -> MeshData {
    let mut b = MeshBuilder::new();
    let armor = Material::metal(Vec3::new(0.16, 0.14, 0.24), 0.50, 0.36);
    let plasma = Material::glow(Vec3::new(1.0, 0.25, 0.75), Vec3::new(4.2, 0.7, 3.0));

    // Elevated armored hood
    b.add_tapered_box(Vec3::new(0.0, 1.65, 0.0), Quat::IDENTITY, 0.0, 0.90, (0.72, 0.78), (0.48, 0.55), armor);
    // Pulsing glowing plasma mortar sac
    b.add_ellipsoid(Vec3::new(0.0, 2.45, 0.18), Vec3::new(0.54, 0.48, 0.62), 8, 12, plasma);
    // Forward cannon mouth
    b.add_prism(Vec3::new(0.0, 1.95, -0.55), Vec3::new(0.0, 2.05, -1.35), 0.26, 0.18, 8, armor);
    b.add_ellipsoid(Vec3::new(0.0, 2.05, -1.32), Vec3::new(0.15, 0.15, 0.15), 5, 8, plasma);

    // 3 heavy splayed legs
    for i in 0..3 {
        let a = (i as f32 / 3.0) * std::f32::consts::TAU + 0.52;
        let dir = Vec3::new(a.cos(), 0.0, a.sin());
        let hip = Vec3::new(dir.x * 0.55, 1.70, dir.z * 0.55);
        let knee = Vec3::new(dir.x * 1.45, 1.95, dir.z * 1.45);
        let foot = Vec3::new(dir.x * 1.55, 0.0, dir.z * 1.55);
        b.add_prism(hip, knee, 0.14, 0.11, 6, armor);
        b.add_prism(knee, foot, 0.11, 0.05, 6, armor);
    }
    b.build()
}

/// Alien Aerial Interceptor — sleek swept-wing bio-drone that dogfights in 3D sky.
fn build_enemy_interceptor() -> MeshData {
    let mut b = MeshBuilder::new();
    let hull = Material::metal(Vec3::new(0.15, 0.12, 0.20), 0.70, 0.28);
    let trim = Material::metal(Vec3::new(0.55, 0.12, 0.24), 0.55, 0.34);
    let glow = Material::glow(Vec3::new(1.0, 0.30, 0.15), Vec3::new(4.5, 1.2, 0.4));

    // Sleek central fuselage
    b.add_prism(Vec3::new(0.0, 0.0, 1.35), Vec3::new(0.0, 0.0, -1.85), 0.38, 0.08, 8, hull);
    // Glowing cyclops eye
    b.add_ellipsoid(Vec3::new(0.0, 0.12, -1.15), Vec3::new(0.24, 0.16, 0.34), 6, 10, glow);

    // Swept razor delta wings + twin ion pods
    for s in [-1.0_f32, 1.0] {
        let wrot = Quat::from_rotation_y(s * 0.35) * Quat::from_rotation_z(s * -0.14);
        b.add_box(Vec3::new(s * 1.45, 0.05, 0.05), Vec3::new(1.35, 0.055, 0.42), wrot, trim);
        b.add_box(Vec3::new(s * 2.55, -0.15, -0.45), Vec3::new(0.08, 0.34, 0.45), Quat::IDENTITY, hull);
        // Glowing engine nozzle
        b.add_prism(Vec3::new(s * 0.65, -0.05, 0.35), Vec3::new(s * 0.65, -0.05, 1.25), 0.22, 0.16, 8, hull);
        b.add_ellipsoid(Vec3::new(s * 0.65, -0.05, 1.28), Vec3::new(0.15, 0.15, 0.22), 5, 8, glow);
    }
    b.build()
}

/// Alien Dreadnought Goliath (Boss) — towering heavy armored siege walker.
fn build_enemy_goliath() -> MeshData {
    let mut b = MeshBuilder::new();
    let armor = Material::metal(Vec3::new(0.16, 0.15, 0.20), 0.78, 0.32);
    let crimson = Material::metal(Vec3::new(0.55, 0.11, 0.14), 0.55, 0.35);
    let core = Material::glow(Vec3::new(1.0, 0.35, 0.08), Vec3::new(5.5, 1.6, 0.25));

    // Massive armored torso citadel
    b.add_tapered_box(Vec3::new(0.0, 2.8, 0.0), Quat::IDENTITY, 0.0, 2.6, (1.45, 1.25), (2.15, 1.65), armor);
    // Armored crown + horns
    b.add_box(Vec3::new(0.0, 5.75, -0.25), Vec3::new(0.95, 0.55, 0.90), Quat::IDENTITY, crimson);
    b.add_box(Vec3::new(0.0, 5.72, -1.16), Vec3::new(0.65, 0.18, 0.10), Quat::IDENTITY, core);
    // Exposed glowing reactor weakspot in chest!
    b.add_ellipsoid(Vec3::new(0.0, 4.15, -1.45), Vec3::new(0.68, 0.68, 0.45), 8, 12, core);

    // Twin heavy shoulder siege cannons + armored legs
    for s in [-1.0_f32, 1.0] {
        b.add_box(Vec3::new(s * 2.55, 4.95, -0.35), Vec3::new(0.68, 0.62, 1.35), Quat::IDENTITY, crimson);
        b.add_prism(Vec3::new(s * 2.55, 4.95, -1.65), Vec3::new(s * 2.55, 4.95, -3.45), 0.32, 0.24, 10, armor);
        b.add_ellipsoid(Vec3::new(s * 2.55, 4.95, -3.45), Vec3::new(0.22, 0.22, 0.22), 5, 8, core);
        // Heavy hydraulic legs
        let hip = Vec3::new(s * 1.25, 2.8, 0.0);
        let knee = Vec3::new(s * 1.85, 1.65, -0.45);
        let foot = Vec3::new(s * 1.65, 0.0, 0.15);
        b.add_prism(hip, knee, 0.45, 0.36, 8, armor);
        b.add_prism(knee, foot, 0.38, 0.48, 8, crimson);
        b.add_box(Vec3::new(s * 1.65, 0.22, -0.15), Vec3::new(0.65, 0.22, 0.95), Quat::IDENTITY, armor);
    }
    b.build()
}

/// Orbital Drop Pod — armored insertion capsule with deployed airbrake petals.
fn build_drop_pod() -> MeshData {
    let mut b = MeshBuilder::new();
    let hull = Material::metal(Vec3::new(0.76, 0.80, 0.85), 0.65, 0.34);
    let dark = Material::metal(Vec3::new(0.16, 0.18, 0.22), 0.78, 0.40);
    let stripe = Material::metal(Vec3::new(0.95, 0.45, 0.10), 0.45, 0.35);
    let beacon = Material::glow(Vec3::new(0.20, 0.95, 1.0), Vec3::new(0.4, 3.2, 4.5));

    // Hexagonal armored capsule
    b.add_prism(Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.0, 0.8, 0.0), 1.55, 1.35, 6, dark);
    b.add_prism(Vec3::new(0.0, 0.8, 0.0), Vec3::new(0.0, 3.3, 0.0), 1.35, 1.05, 6, hull);
    b.add_prism(Vec3::new(0.0, 3.3, 0.0), Vec3::new(0.0, 4.0, 0.0), 1.05, 0.42, 6, stripe);
    b.add_ellipsoid(Vec3::new(0.0, 4.15, 0.0), Vec3::new(0.26, 0.26, 0.26), 6, 8, beacon);

    // 4 deployed thruster airbrake fins
    for i in 0..4 {
        let a = (i as f32 / 4.0) * std::f32::consts::TAU + 0.78;
        let rot = Quat::from_rotation_y(a) * Quat::from_rotation_x(0.45);
        b.add_box(
            Vec3::new(a.sin() * 1.45, 1.15, a.cos() * 1.45),
            Vec3::new(0.45, 1.05, 0.10),
            rot,
            dark,
        );
    }
    b.build()
}

/// Alien Spore Spire — towering crystalline bio-monolith mission objective!
fn build_spore_spire() -> MeshData {
    let mut b = MeshBuilder::new();
    let obsidian = Material::metal(Vec3::new(0.11, 0.09, 0.15), 0.60, 0.28);
    let vein = Material::glow(Vec3::new(1.0, 0.22, 0.48), Vec3::new(4.5, 0.65, 1.8));

    // Base root buttresses
    for i in 0..5 {
        let a = (i as f32 / 5.0) * std::f32::consts::TAU;
        let base = Vec3::new(a.cos() * 4.2, -0.5, a.sin() * 4.2);
        let tip = Vec3::new(a.cos() * 1.1, 6.2, a.sin() * 1.1);
        b.add_prism(base, tip, 1.15, 0.42, 5, obsidian);
    }
    // Tall twisted central monolith
    b.add_prism(Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.0, 8.5, 0.0), 2.1, 1.35, 6, obsidian);
    b.add_prism(Vec3::new(0.0, 8.5, 0.0), Vec3::new(0.0, 15.5, 0.0), 1.25, 0.22, 6, obsidian);
    // Glowing exposed energy core crystal suspended in center
    b.add_ellipsoid(Vec3::new(0.0, 7.6, 0.0), Vec3::new(1.55, 2.35, 1.55), 8, 10, vein);
    b.add_prism(Vec3::new(0.0, 15.2, 0.0), Vec3::new(0.0, 18.2, 0.0), 0.35, 0.05, 6, vein);
    b.build()
}

/// Floating rotating energy ring around a Spore Spire.
fn build_spire_ring() -> MeshData {
    let mut b = MeshBuilder::new();
    let dark = Material::metal(Vec3::new(0.15, 0.12, 0.22), 0.72, 0.30);
    let glow = Material::glow(Vec3::new(1.0, 0.25, 0.55), Vec3::new(4.0, 0.6, 1.9));
    b.add_torus(Vec3::ZERO, Quat::IDENTITY, 4.0, 0.22, 24, 6, dark);
    b.add_torus(Vec3::ZERO, Quat::IDENTITY, 3.65, 0.10, 24, 6, glow);
    b.build()
}

fn build_outpost_bunker() -> MeshData {
    let mut b = MeshBuilder::new();
    let concrete = Material::matte(Vec3::new(0.46, 0.49, 0.53), 0.72);
    let steel = Material::metal(Vec3::new(0.22, 0.25, 0.30), 0.75, 0.38);
    let light = Material::glow(Vec3::new(0.25, 0.92, 1.0), Vec3::new(0.5, 2.8, 3.8));

    b.add_tapered_box(Vec3::ZERO, Quat::IDENTITY, 0.0, 4.4, (6.2, 4.4), (5.2, 3.6), concrete);
    b.add_box(Vec3::new(0.0, 4.6, 0.0), Vec3::new(4.8, 0.35, 3.2), Quat::IDENTITY, steel);
    // Glowing viewport slit
    b.add_box(Vec3::new(0.0, 3.0, -3.95), Vec3::new(3.4, 0.32, 0.18), Quat::IDENTITY, light);
    b.add_box(Vec3::new(0.0, 3.0, 3.95), Vec3::new(3.4, 0.32, 0.18), Quat::IDENTITY, light);
    b.build()
}

fn build_comms_tower() -> MeshData {
    let mut b = MeshBuilder::new();
    let steel = Material::metal(Vec3::new(0.32, 0.36, 0.42), 0.76, 0.35);
    let orange = Material::metal(Vec3::new(0.90, 0.42, 0.12), 0.50, 0.38);
    let beacon = Material::glow(Vec3::new(0.20, 0.95, 1.0), Vec3::new(0.4, 3.5, 5.0));

    b.add_tapered_box(Vec3::ZERO, Quat::IDENTITY, 0.0, 2.5, (2.8, 2.8), (1.8, 1.8), steel);
    b.add_prism(Vec3::new(0.0, 2.5, 0.0), Vec3::new(0.0, 18.0, 0.0), 1.15, 0.32, 6, steel);
    b.add_torus(Vec3::new(0.0, 13.5, 0.0), Quat::IDENTITY, 2.4, 0.22, 16, 6, orange);
    b.add_ellipsoid(Vec3::new(0.0, 18.6, 0.0), Vec3::new(0.55, 0.55, 0.55), 7, 10, beacon);
    b.build()
}

fn build_landing_pad() -> MeshData {
    let mut b = MeshBuilder::new();
    let deck = Material::metal(Vec3::new(0.20, 0.23, 0.28), 0.65, 0.42);
    let rim = Material::metal(Vec3::new(0.45, 0.48, 0.54), 0.72, 0.35);
    let guide = Material::glow(Vec3::new(0.20, 0.95, 1.0), Vec3::new(0.35, 2.4, 3.4));

    b.add_prism(Vec3::new(0.0, -0.4, 0.0), Vec3::new(0.0, 0.55, 0.0), 12.5, 11.4, 8, rim);
    b.add_prism(Vec3::new(0.0, 0.50, 0.0), Vec3::new(0.0, 0.62, 0.0), 10.6, 10.6, 8, deck);
    b.add_torus(Vec3::new(0.0, 0.64, 0.0), Quat::IDENTITY, 8.6, 0.14, 24, 6, guide);
    b.build()
}

fn build_crag_rock() -> MeshData {
    let mut b = MeshBuilder::new();
    let rock = Material::matte(Vec3::new(0.88, 0.88, 0.90), 0.82); // tinted per-biome via instance tint!
    b.add_prism(Vec3::new(0.0, -0.5, 0.0), Vec3::new(0.2, 4.2, -0.1), 2.8, 1.1, 7, rock);
    b.add_prism(Vec3::new(1.5, -0.4, 0.8), Vec3::new(1.8, 2.6, 0.6), 1.6, 0.5, 6, rock);
    b.add_prism(Vec3::new(-1.4, -0.4, -0.6), Vec3::new(-1.6, 2.2, -0.8), 1.5, 0.45, 6, rock);
    b.build()
}

fn build_projectile_bolt() -> MeshData {
    let mut b = MeshBuilder::new();
    let glow = Material::glow(Vec3::ONE, Vec3::splat(4.5));
    b.add_prism(Vec3::new(0.0, 0.0, 0.65), Vec3::new(0.0, 0.0, -0.65), 0.09, 0.09, 6, glow);
    b.build()
}

fn build_missile() -> MeshData {
    let mut b = MeshBuilder::new();
    let body = Material::metal(Vec3::new(0.85, 0.88, 0.92), 0.7, 0.3);
    let warhead = Material::glow(Vec3::new(1.0, 0.45, 0.12), Vec3::new(4.2, 1.4, 0.3));
    b.add_prism(Vec3::new(0.0, 0.0, 0.55), Vec3::new(0.0, 0.0, -0.45), 0.11, 0.10, 8, body);
    b.add_prism(Vec3::new(0.0, 0.0, -0.45), Vec3::new(0.0, 0.0, -0.78), 0.10, 0.01, 8, warhead);
    b.add_ellipsoid(Vec3::new(0.0, 0.0, 0.62), Vec3::new(0.14, 0.14, 0.24), 5, 6, warhead);
    b.build()
}

fn build_glow_sphere() -> MeshData {
    let mut b = MeshBuilder::new();
    let glow = Material::glow(Vec3::ONE, Vec3::splat(3.5));
    b.add_ellipsoid(Vec3::ZERO, Vec3::ONE, 8, 12, glow);
    b.build()
}

fn build_shock_ring() -> MeshData {
    let mut b = MeshBuilder::new();
    let glow = Material::glow(Vec3::ONE, Vec3::splat(4.0));
    b.add_torus(Vec3::ZERO, Quat::IDENTITY, 1.0, 0.09, 28, 6, glow);
    b.build()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_meshes_are_non_empty_and_valid() {
        let catalog = AssetCatalog::generate_all();
        assert_eq!(catalog.meshes.len(), MeshId::COUNT);
        for (i, mesh) in catalog.meshes.iter().enumerate() {
            assert!(!mesh.vertices.is_empty(), "mesh {} empty verts", i);
            assert!(!mesh.indices.is_empty(), "mesh {} empty indices", i);
            assert_eq!(mesh.indices.len() % 3, 0);
            for &idx in &mesh.indices {
                assert!((idx as usize) < mesh.vertices.len());
            }
        }
    }
}
