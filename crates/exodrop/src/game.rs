use crate::entities::{
    BeamFx, CommsMsg, Enemy, EnemyKind, Particle, Projectile, ProjectileKind, Shockwave, Spire,
    Wingman,
};
use exo_assets::{MeshId, WorldLayout};
use exo_audio::{AudioEngine, SoundEvent};
use exo_core::{damp, damp_vec3, BiomeConfig, BiomeId, InputState, Key, Rng, SquadOrder, WeaponKind};
use exo_gfx::renderer::{push_fx_beam, push_fx_billboard};
use exo_gfx::{Camera, FxVertex, InstanceData, PointLight};
use exo_physics::{raycast_aabb, raycast_sphere, raycast_terrain, ExoSuitInput, ExoSuitState, Terrain};
use glam::{Mat4, Quat, Vec2, Vec3, Vec4};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GameMode {
    Title,
    Playing,
    Paused,
    SectorCleared,
    GameOver,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LockTarget {
    Enemy(u32),
    Spire(u32),
}

pub struct GameState {
    pub mode: GameMode,
    pub biome: BiomeId,
    pub biome_cfg: BiomeConfig,
    pub terrain: Terrain,
    pub layout: WorldLayout,
    pub rng: Rng,
    pub time: f32,

    // Player Exo-Suit
    pub player: ExoSuitState,
    pub shield: f32,
    pub max_shield: f32,
    pub hull: f32,
    pub max_hull: f32,
    pub shield_regen_delay: f32,
    pub weapon: WeaponKind,
    pub fire_cooldown: f32,
    pub weapon_recoil: f32,
    pub first_person: bool,

    // Camera state
    pub cam_yaw: f32,
    pub cam_pitch: f32,
    pub cam_pos: Vec3,
    pub cam_target: Vec3,
    pub cam_fov_deg: f32,
    pub cam_roll: f32,
    pub cam_shake: f32,
    pub boost_fx: f32,
    pub damage_fx: f32,

    // Multi-lock missile targeting
    pub locking: bool,
    pub lock_targets: Vec<LockTarget>,
    pub lock_scan_timer: f32,

    // Squad & Mission
    pub squad_order: SquadOrder,
    pub hold_point: Vec3,
    pub wingmen: Vec<Wingman>,
    pub spires: Vec<Spire>,
    pub enemies: Vec<Enemy>,
    pub boss_spawned: bool,
    pub next_entity_id: u32,

    // Combat FX & Telemetry
    pub projectiles: Vec<Projectile>,
    pub particles: Vec<Particle>,
    pub beams: Vec<BeamFx>,
    pub shockwaves: Vec<Shockwave>,
    pub comms: Vec<CommsMsg>,
    pub score: u32,
    pub kills: u32,
    pub combo: u32,
    pub combo_timer: f32,
    pub hitmarker_timer: f32,
    pub hitmarker_crit: bool,
    pub hitmarker_kill: bool,
    pub muzzle_flash_timer: f32,
    pub muzzle_pos: Vec3,
    pub muzzle_color: Vec3,
    pub terrain_dirty: bool,
}

impl GameState {
    pub fn new(biome: BiomeId) -> Self {
        let biome_cfg = biome.config();
        let terrain = Terrain::generate(biome);
        let layout = WorldLayout::generate(biome, &terrain);
        let player = ExoSuitState::new(layout.player_spawn, 0.0);
        let rng = Rng::new(biome_cfg.seed ^ 0xDEAD_BEEF);

        let mut gs = Self {
            mode: GameMode::Title,
            biome,
            biome_cfg,
            terrain,
            layout,
            rng,
            time: 0.0,
            player,
            shield: 100.0,
            max_shield: 100.0,
            hull: 100.0,
            max_hull: 100.0,
            shield_regen_delay: 0.0,
            weapon: WeaponKind::PulseCarbine,
            fire_cooldown: 0.0,
            weapon_recoil: 0.0,
            first_person: false,
            cam_yaw: 0.0,
            cam_pitch: -0.06,
            cam_pos: Vec3::ZERO,
            cam_target: Vec3::ZERO,
            cam_fov_deg: 68.0,
            cam_roll: 0.0,
            cam_shake: 0.0,
            boost_fx: 0.0,
            damage_fx: 0.0,
            locking: false,
            lock_targets: Vec::new(),
            lock_scan_timer: 0.0,
            squad_order: SquadOrder::Follow,
            hold_point: Vec3::ZERO,
            wingmen: Vec::new(),
            spires: Vec::new(),
            enemies: Vec::new(),
            boss_spawned: false,
            next_entity_id: 1,
            projectiles: Vec::new(),
            particles: Vec::new(),
            beams: Vec::new(),
            shockwaves: Vec::new(),
            comms: Vec::new(),
            score: 0,
            kills: 0,
            combo: 0,
            combo_timer: 0.0,
            hitmarker_timer: 0.0,
            hitmarker_crit: false,
            hitmarker_kill: false,
            muzzle_flash_timer: 0.0,
            muzzle_pos: Vec3::ZERO,
            muzzle_color: Vec3::ONE,
            terrain_dirty: true,
        };
        gs.reset_sector(biome, GameMode::Title);
        gs
    }

    pub fn reset_sector(&mut self, biome: BiomeId, target_mode: GameMode) {
        self.biome = biome;
        self.biome_cfg = biome.config();
        self.terrain = Terrain::generate(biome);
        self.layout = WorldLayout::generate(biome, &self.terrain);
        self.rng = Rng::new(self.biome_cfg.seed ^ 0x51A7_2026);
        self.mode = target_mode;
        self.terrain_dirty = true;

        self.player = ExoSuitState::new(self.layout.player_spawn, 0.0);
        self.shield = self.max_shield;
        self.hull = self.max_hull;
        self.shield_regen_delay = 0.0;
        self.fire_cooldown = 0.0;
        self.weapon_recoil = 0.0;
        self.cam_yaw = 0.0;
        self.cam_pitch = -0.08;
        self.cam_fov_deg = 68.0;
        self.cam_roll = 0.0;
        self.cam_shake = 0.0;
        self.boost_fx = 0.0;
        self.damage_fx = 0.0;
        self.locking = false;
        self.lock_targets.clear();
        self.squad_order = SquadOrder::Follow;
        self.hold_point = self.layout.player_spawn;
        self.boss_spawned = false;
        self.next_entity_id = 1;
        self.projectiles.clear();
        self.particles.clear();
        self.beams.clear();
        self.shockwaves.clear();
        self.comms.clear();
        self.enemies.clear();
        self.spires.clear();
        self.combo = 0;
        self.combo_timer = 0.0;

        // Spawn AI Squad Wingmen ECHO-2 & ECHO-3
        self.wingmen = vec![
            Wingman {
                callsign: "ECHO-2",
                pos: self.layout.wingman_spawns[0],
                vel: Vec3::ZERO,
                yaw: 0.0,
                pitch: 0.0,
                thruster: 0.0,
                gait_phase: 0.0,
                grounded: true,
                fire_timer: 0.3,
                formation_offset: Vec3::new(-7.5, 1.5, 4.5),
                armor_tint: Vec3::new(0.76, 0.90, 1.0),
            },
            Wingman {
                callsign: "ECHO-3",
                pos: self.layout.wingman_spawns[1],
                vel: Vec3::ZERO,
                yaw: 0.0,
                pitch: 0.0,
                thruster: 0.0,
                gait_phase: 1.5,
                grounded: true,
                fire_timer: 0.6,
                formation_offset: Vec3::new(7.5, 2.0, 4.5),
                armor_tint: Vec3::new(0.95, 0.88, 0.72),
            },
        ];

        // Spawn 3 Alien Spore Spires
        let placements = self.layout.spires.clone();
        for sp in placements {
            let id = self.alloc_id();
            self.spires.push(Spire {
                id,
                label: sp.label,
                pos: sp.position,
                hp: 420.0,
                max_hp: 420.0,
                ring_angle: self.rng.range(0.0, std::f32::consts::TAU),
                spawn_timer: self.rng.range(4.0, 9.0),
                hit_flash: 0.0,
                alive: true,
            });
        }

        // Populate initial combat vanguard across the canyon!
        for _ in 0..8 {
            let x = self.rng.range(-95.0, 95.0);
            let z = self.rng.range(-55.0, 22.0);
            self.spawn_enemy(EnemyKind::Swarmer, Vec3::new(x, 0.0, z));
        }
        for &(sx, sz) in &[(-58.0, -25.0), (54.0, -22.0), (0.0, -68.0)] {
            self.spawn_enemy(EnemyKind::Spitter, Vec3::new(sx, 0.0, sz));
        }
        for &(ix, iz) in &[(-45.0, -15.0), (48.0, -30.0), (0.0, -45.0)] {
            self.spawn_enemy(EnemyKind::Interceptor, Vec3::new(ix, 26.0, iz));
        }

        self.cam_pos = self.player.position + Vec3::new(0.0, 3.2, 6.2);
        self.cam_target = self.player.position + Vec3::new(0.0, 1.8, -10.0);

        self.push_comms(
            format!("ORBITAL COMMAND: DROP COMPLETE ON {}", self.biome_cfg.name),
            Vec4::new(0.35, 0.95, 1.0, 1.0),
        );
        self.push_comms(
            "ECHO-2: JUMP-JETS HOT. READY TO CLEAR ALIEN SPIRES!".into(),
            Vec4::new(0.75, 0.92, 1.0, 1.0),
        );
    }

    fn alloc_id(&mut self) -> u32 {
        let id = self.next_entity_id;
        self.next_entity_id += 1;
        id
    }

    pub fn push_comms(&mut self, text: String, color: Vec4) {
        self.comms.push(CommsMsg { text, color, ttl: 6.5 });
        if self.comms.len() > 4 {
            self.comms.remove(0);
        }
    }

    pub fn spawn_enemy(&mut self, kind: EnemyKind, raw_pos: Vec3) {
        let y = match kind {
            EnemyKind::Interceptor => {
                let g = self.terrain.sample_height(raw_pos.x, raw_pos.z);
                (g + self.rng.range(16.0, 32.0)).max(raw_pos.y)
            }
            _ => self.terrain.sample_height(raw_pos.x, raw_pos.z),
        };
        let id = self.alloc_id();
        let max_hp = kind.max_hp();
        self.enemies.push(Enemy {
            id,
            kind,
            pos: Vec3::new(raw_pos.x, y, raw_pos.z),
            vel: Vec3::ZERO,
            yaw: self.rng.range(0.0, std::f32::consts::TAU),
            pitch: 0.0,
            hp: max_hp,
            max_hp,
            fire_timer: self.rng.range(0.8, 2.4),
            hit_flash: 0.0,
            anim_phase: self.rng.range(0.0, std::f32::consts::TAU),
            strafe_sign: if self.rng.f32() > 0.5 { 1.0 } else { -1.0 },
            alive: true,
        });
    }

    pub fn look_dir(&self) -> Vec3 {
        Vec3::new(
            -self.cam_yaw.sin() * self.cam_pitch.cos(),
            self.cam_pitch.sin(),
            -self.cam_yaw.cos() * self.cam_pitch.cos(),
        )
        .normalize()
    }

    pub fn current_camera(&self) -> Camera {
        if self.mode == GameMode::Title {
            let orbit = self.time * 0.18 + 0.55;
            let focus = self.layout.player_spawn + Vec3::new(0.0, 2.8, -3.5);
            let eye = focus + Vec3::new(orbit.sin() * 14.0, 2.6 + (orbit * 1.2).sin() * 0.7, orbit.cos() * 14.0);
            let min_y = self.terrain.sample_height(eye.x, eye.z) + 1.8;
            return Camera {
                eye: Vec3::new(eye.x, eye.y.max(min_y), eye.z),
                target: focus + Vec3::new(-2.0, 2.4, -18.0),
                up: Vec3::Y,
                fov_y_rad: 60.0_f32.to_radians(),
                near: 0.25,
                far: 750.0,
                roll: 0.0,
            };
        }

        let shake_off = if self.cam_shake > 0.002 {
            Vec3::new(
                (self.time * 57.0).sin() * self.cam_shake,
                (self.time * 63.0).cos() * self.cam_shake,
                (self.time * 49.0).sin() * self.cam_shake * 0.5,
            )
        } else {
            Vec3::ZERO
        };
        Camera {
            eye: self.cam_pos + shake_off,
            target: self.cam_target + shake_off * 0.4,
            up: Vec3::Y,
            fov_y_rad: self.cam_fov_deg.to_radians(),
            near: 0.2,
            far: 780.0,
            roll: self.cam_roll,
        }
    }

    /// Handle frame-level UI/mode keys & mouse look.
    pub fn handle_frame_input(&mut self, input: &InputState, audio: &AudioEngine) {
        // Mouse look while playing
        if self.mode == GameMode::Playing {
            let sens = if input.mouse_right { 0.0014 } else { 0.0024 };
            self.cam_yaw -= input.mouse_delta.x * sens;
            self.cam_pitch = (self.cam_pitch - input.mouse_delta.y * sens).clamp(-1.28, 1.32);

            if input.key_pressed(Key::V) {
                self.first_person = !self.first_person;
            }
            if input.key_pressed(Key::Digit1) { self.select_weapon(WeaponKind::PulseCarbine); }
            if input.key_pressed(Key::Digit2) { self.select_weapon(WeaponKind::RailLance); }
            if input.key_pressed(Key::Digit3) { self.select_weapon(WeaponKind::MicroMissiles); }
            if input.key_pressed(Key::Digit4) { self.select_weapon(WeaponKind::ArcMortar); }
            if input.scroll_delta.abs() > 0.1 {
                let idx = self.weapon as isize + if input.scroll_delta > 0.0 { -1 } else { 1 };
                self.select_weapon(WeaponKind::from_index(idx.rem_euclid(4) as usize));
            }
            if input.key_pressed(Key::F1) {
                self.squad_order = SquadOrder::Follow;
                audio.play(SoundEvent::SquadAck);
                self.push_comms("ECHO SQUAD: FORMING UP ON YOUR WING!".into(), Vec4::new(0.4, 0.95, 1.0, 1.0));
            }
            if input.key_pressed(Key::F2) {
                self.squad_order = SquadOrder::AttackTarget;
                audio.play(SoundEvent::SquadAck);
                self.push_comms("ECHO SQUAD: WEAPONS FREE — FOCUSING HOSTILES!".into(), Vec4::new(1.0, 0.72, 0.25, 1.0));
            }
            if input.key_pressed(Key::F3) {
                self.squad_order = SquadOrder::HoldPosition;
                self.hold_point = self.player.position;
                audio.play(SoundEvent::SquadAck);
                self.push_comms("ECHO SQUAD: HOLDING AIRSPACE OVER MARKED SECTOR!".into(), Vec4::new(0.55, 1.0, 0.65, 1.0));
            }
            if input.key_pressed(Key::N) {
                let next = self.biome.next();
                self.reset_sector(next, GameMode::Playing);
            }
            if input.key_pressed(Key::Escape) {
                self.mode = GameMode::Paused;
            }
        } else if self.mode == GameMode::Title {
            if input.key_pressed(Key::Digit1) { self.reset_sector(BiomeId::CrimsonMesa, GameMode::Title); }
            if input.key_pressed(Key::Digit2) { self.reset_sector(BiomeId::EuropaBasin, GameMode::Title); }
            if input.key_pressed(Key::Digit3) { self.reset_sector(BiomeId::IoCaldera, GameMode::Title); }
            if input.key_pressed(Key::Enter) || input.key_pressed(Key::Space) || input.mouse_left_pressed {
                self.mode = GameMode::Playing;
                audio.play(SoundEvent::BoostBurst);
            }
        } else if self.mode == GameMode::Paused {
            if input.key_pressed(Key::Escape) || input.key_pressed(Key::Enter) {
                self.mode = GameMode::Playing;
            }
            if input.key_pressed(Key::R) {
                self.reset_sector(self.biome, GameMode::Playing);
            }
            if input.key_pressed(Key::N) {
                let next = self.biome.next();
                self.reset_sector(next, GameMode::Playing);
            }
        } else if self.mode == GameMode::SectorCleared {
            if input.key_pressed(Key::Enter) || input.key_pressed(Key::Space) || input.key_pressed(Key::N) {
                let next = self.biome.next();
                self.reset_sector(next, GameMode::Playing);
            }
        } else if self.mode == GameMode::GameOver {
            if input.key_pressed(Key::Enter) || input.key_pressed(Key::R) || input.key_pressed(Key::Space) {
                self.reset_sector(self.biome, GameMode::Playing);
            }
        }
    }

    fn select_weapon(&mut self, w: WeaponKind) {
        if self.weapon != w {
            self.weapon = w;
            self.locking = false;
            self.lock_targets.clear();
        }
    }

    /// Fixed-timestep simulation tick (120 Hz).
    pub fn fixed_update(&mut self, dt: f32, input: &InputState, audio: &AudioEngine) {
        self.time += dt;

        // Always tick ambient Spire rotations & particles on Title screen too
        for spire in &mut self.spires {
            if spire.alive {
                spire.ring_angle += dt * 1.15;
                spire.hit_flash = (spire.hit_flash - dt * 5.0).max(0.0);
            }
        }

        if self.mode != GameMode::Playing {
            audio.update_telemetry(0.0, 0.0, 0.0);
            self.update_fx_only(dt);
            return;
        }

        // Decay timers
        self.fire_cooldown = (self.fire_cooldown - dt).max(0.0);
        self.weapon_recoil = damp(self.weapon_recoil, 0.0, 14.0, dt);
        self.cam_shake = damp(self.cam_shake, 0.0, 8.5, dt);
        self.damage_fx = damp(self.damage_fx, 0.0, 4.0, dt);
        self.hitmarker_timer = (self.hitmarker_timer - dt).max(0.0);
        self.muzzle_flash_timer = (self.muzzle_flash_timer - dt).max(0.0);
        self.shield_regen_delay = (self.shield_regen_delay - dt).max(0.0);
        if self.combo_timer > 0.0 {
            self.combo_timer -= dt;
            if self.combo_timer <= 0.0 {
                self.combo = 0;
            }
        }
        for c in &mut self.comms {
            c.ttl -= dt;
        }
        self.comms.retain(|c| c.ttl > 0.0);

        // Shield regeneration
        if self.shield_regen_delay <= 0.0 && self.shield < self.max_shield {
            self.shield = (self.shield + 22.0 * dt).min(self.max_shield);
        }

        // 1. Step Player Exo-Suit Physics
        let mut mv = Vec2::ZERO;
        if input.key_down(Key::W) { mv.y += 1.0; }
        if input.key_down(Key::S) { mv.y -= 1.0; }
        if input.key_down(Key::D) { mv.x += 1.0; }
        if input.key_down(Key::A) { mv.x -= 1.0; }

        let suit_in = ExoSuitInput {
            move_dir: mv,
            yaw: self.cam_yaw,
            pitch: self.cam_pitch,
            jump_jet: input.key_down(Key::Space),
            jump_pressed: input.key_pressed(Key::Space),
            boost: input.key_down(Key::Shift),
            hover_ads: input.mouse_right,
            dive_slam: input.key_pressed(Key::C) || input.key_pressed(Key::Ctrl),
        };
        let events = self.player.step(
            &suit_in,
            &self.terrain,
            &self.layout.colliders,
            self.biome_cfg.gravity,
            dt,
        );

        if events.boost_burst {
            audio.play(SoundEvent::BoostBurst);
            self.cam_shake = self.cam_shake.max(0.14);
        }
        if events.dive_slam_impact {
            audio.play(SoundEvent::DiveSlam);
            self.cam_shake = 0.55;
            self.spawn_explosion(self.player.position + Vec3::Y * 0.4, 15.0, 95.0, Vec3::new(0.25, 0.90, 1.0), true, audio);
            self.push_comms("SUIT AI: SEISMIC DIVE-SLAM DISCHARGED!".into(), Vec4::new(0.25, 0.95, 1.0, 1.0));
        }

        // Molten lava hazard on Io Caldera!
        if self.biome == BiomeId::IoCaldera && self.player.position.y <= self.terrain.lava_level + 0.45 {
            self.damage_player(28.0 * dt, audio);
        }

        // Thruster exhaust sparks
        if (self.player.thruster_intensity > 0.25 || self.player.afterburner_intensity > 0.2)
            && self.rng.f32() < 0.65
        {
            let back = -self.player.forward_xz();
            let p_pos = self.player.position + Vec3::Y * 1.15 + back * 0.45;
            let col = if self.player.afterburner_intensity > 0.3 {
                Vec4::new(2.8, 1.2, 0.25, 0.95)
            } else {
                Vec4::new(0.35, 2.2, 3.2, 0.85)
            };
            self.particles.push(Particle {
                pos: p_pos + self.rng.unit_vec3() * 0.22,
                vel: Vec3::new(0.0, -5.5, 0.0) + back * (4.0 + self.player.afterburner_intensity * 8.0) + self.rng.unit_vec3() * 1.5,
                color: col,
                size: 0.26 + self.player.afterburner_intensity * 0.16,
                age: 0.0,
                max_age: 0.24,
                gravity: -2.0,
            });
        }

        // Update audio synth telemetry
        audio.update_telemetry(
            self.player.thruster_intensity,
            self.player.afterburner_intensity,
            self.player.speed(),
        );

        // 2. Smooth 3rd-Person / 1st-Person Camera Rig
        let look = self.look_dir();
        let right = look.cross(Vec3::Y).normalize_or_zero();
        let target_fov = if input.mouse_right {
            46.0
        } else {
            68.0 + self.player.afterburner_intensity * 16.0
        };
        self.cam_fov_deg = damp(self.cam_fov_deg, target_fov, 10.0, dt);
        self.boost_fx = damp(self.boost_fx, self.player.afterburner_intensity, 9.0, dt);
        self.cam_roll = damp(self.cam_roll, -self.player.lean_roll * 0.22, 8.0, dt);

        if self.first_person {
            self.cam_pos = self.player.position + Vec3::Y * 1.82;
            self.cam_target = self.cam_pos + look * 40.0;
        } else {
            let shoulder_dist = if input.mouse_right { 3.2 } else { 5.8 + self.player.afterburner_intensity * 1.1 };
            let shoulder_right = if input.mouse_right { 0.95 } else { 1.15 };
            let pivot = self.player.position + Vec3::Y * 1.95 + right * shoulder_right;
            let mut desired_eye = pivot - look * shoulder_dist + Vec3::Y * 0.55;
            let terrain_floor = self.terrain.sample_height(desired_eye.x, desired_eye.z) + 0.85;
            if desired_eye.y < terrain_floor {
                desired_eye.y = terrain_floor;
            }
            self.cam_pos = damp_vec3(self.cam_pos, desired_eye, 24.0, dt);
            self.cam_target = pivot + look * 60.0;
        }

        // 3. Player Weapons & Multi-Lock Missile Pod
        self.update_player_weapons(dt, input, audio);

        // 4. AI Wingmen (ECHO-2 & ECHO-3)
        self.update_wingmen(dt, audio);

        // 5. Alien Spires & Enemy AI
        self.update_spires_and_enemies(dt, audio);

        // 6. Projectiles, Beams, Particles & Shockwaves
        self.update_projectiles(dt, audio);
        self.update_fx_only(dt);

        // 7. Mission Victory / Boss Spawn check
        let alive_spires = self.spires.iter().filter(|s| s.alive).count();
        if !self.boss_spawned && alive_spires <= 1 {
            self.boss_spawned = true;
            let bpos = self.layout.boss_spawn;
            self.spawn_enemy(EnemyKind::Goliath, bpos);
            self.cam_shake = 0.45;
            audio.play(SoundEvent::Explosion { intensity: 1.0 });
            self.push_comms(
                "WARNING: DREADNOUGHT GOLIATH DEPLOYED IN NORTH CITADEL!".into(),
                Vec4::new(1.0, 0.25, 0.20, 1.0),
            );
        }
        let boss_alive = self.enemies.iter().any(|e| e.alive && e.kind == EnemyKind::Goliath);
        if alive_spires == 0 && self.boss_spawned && !boss_alive {
            self.mode = GameMode::SectorCleared;
            audio.play(SoundEvent::KillConfirm);
            self.push_comms(
                format!("SECTOR CLEARED! {} SECURED!", self.biome_cfg.name),
                Vec4::new(0.25, 1.0, 0.55, 1.0),
            );
        }
    }

    fn aim_world_point(&self) -> Vec3 {
        let origin = self.cam_pos;
        let dir = (self.cam_target - self.cam_pos).normalize_or_zero();
        let mut best_t = 280.0_f32;
        if let Some(hit) = raycast_terrain(origin, dir, &self.terrain, best_t) {
            best_t = hit.t;
        }
        for e in &self.enemies {
            if !e.alive { continue; }
            if let Some(t) = raycast_sphere(origin, dir, e.center(), e.kind.hit_radius() * 1.1) {
                if t < best_t { best_t = t; }
            }
        }
        for s in &self.spires {
            if !s.alive { continue; }
            if let Some(t) = raycast_sphere(origin, dir, s.core_pos(), 3.2) {
                if t < best_t { best_t = t; }
            }
        }
        origin + dir * best_t
    }

    fn player_muzzle_world(&self) -> Vec3 {
        let right = self.player.right_xz();
        let fwd = self.look_dir();
        self.player.position + Vec3::Y * 1.55 + right * 0.46 + fwd * 0.85
    }

    fn update_player_weapons(&mut self, dt: f32, input: &InputState, audio: &AudioEngine) {
        let aim_pt = self.aim_world_point();
        let muzzle = self.player_muzzle_world();
        let shot_dir = (aim_pt - muzzle).normalize_or_zero();

        match self.weapon {
            WeaponKind::MicroMissiles => {
                if input.mouse_left && self.fire_cooldown <= 0.0 {
                    self.locking = true;
                    self.lock_scan_timer -= dt;
                    if self.lock_scan_timer <= 0.0 && self.lock_targets.len() < 4 {
                        if let Some(t) = self.find_best_lock_candidate() {
                            self.lock_targets.push(t);
                            self.lock_scan_timer = 0.16;
                            audio.play(SoundEvent::LockBeep);
                        }
                    }
                }
                if input.mouse_left_released && self.locking {
                    self.locking = false;
                    // If quick tap without locks, acquire up to 2 targets in front immediately
                    if self.lock_targets.is_empty() {
                        for _ in 0..2 {
                            if let Some(t) = self.find_best_lock_candidate() {
                                self.lock_targets.push(t);
                            }
                        }
                    }
                    let targets = std::mem::take(&mut self.lock_targets);
                    let salvo_count = targets.len().max(2);
                    for i in 0..salvo_count {
                        let tgt = targets.get(i % targets.len().max(1)).copied();
                        let spread_x = (i as f32 - (salvo_count as f32 - 1.0) * 0.5) * 3.8;
                        let right = self.player.right_xz();
                        let init_vel = shot_dir * 28.0 + Vec3::Y * 11.0 + right * spread_x;
                        let (te, ts) = match tgt {
                            Some(LockTarget::Enemy(id)) => (Some(id), None),
                            Some(LockTarget::Spire(id)) => (None, Some(id)),
                            None => (None, None),
                        };
                        self.projectiles.push(Projectile {
                            kind: ProjectileKind::PlayerMissile,
                            pos: muzzle + right * (spread_x * 0.08),
                            vel: init_vel,
                            damage: 62.0,
                            blast_radius: 7.5,
                            age: 0.0,
                            max_age: 4.5,
                            target_enemy_id: te,
                            target_spire_id: ts,
                            color: Vec3::new(1.0, 0.45, 0.14),
                            alive: true,
                        });
                    }
                    self.fire_cooldown = self.weapon.cooldown();
                    self.weapon_recoil = 0.25;
                    self.cam_shake = self.cam_shake.max(0.14);
                    self.muzzle_flash_timer = 0.09;
                    self.muzzle_pos = muzzle;
                    self.muzzle_color = Vec3::new(3.8, 1.5, 0.4);
                    audio.play(SoundEvent::MissileLaunch);
                }
            }
            WeaponKind::PulseCarbine => {
                if input.mouse_left && self.fire_cooldown <= 0.0 {
                    let spread = self.rng.unit_vec3() * if input.mouse_right { 0.005 } else { 0.015 };
                    let dir = (shot_dir + spread).normalize();
                    self.projectiles.push(Projectile {
                        kind: ProjectileKind::PlayerPulse,
                        pos: muzzle,
                        vel: dir * 135.0,
                        damage: 24.0,
                        blast_radius: 0.0,
                        age: 0.0,
                        max_age: 2.2,
                        target_enemy_id: None,
                        target_spire_id: None,
                        color: Vec3::new(0.20, 0.95, 1.0),
                        alive: true,
                    });
                    self.fire_cooldown = self.weapon.cooldown();
                    self.weapon_recoil = 0.14;
                    self.cam_shake = self.cam_shake.max(0.035);
                    self.muzzle_flash_timer = 0.055;
                    self.muzzle_pos = muzzle;
                    self.muzzle_color = Vec3::new(0.6, 2.8, 3.6);
                    audio.play(SoundEvent::PulseShot);
                }
            }
            WeaponKind::RailLance => {
                if input.mouse_left && self.fire_cooldown <= 0.0 {
                    self.fire_rail_lance(muzzle, shot_dir, audio);
                    self.fire_cooldown = self.weapon.cooldown();
                    self.weapon_recoil = 0.42;
                    self.cam_shake = self.cam_shake.max(0.24);
                    self.muzzle_flash_timer = 0.12;
                    self.muzzle_pos = muzzle;
                    self.muzzle_color = Vec3::new(4.2, 3.2, 0.8);
                    audio.play(SoundEvent::RailShot);
                }
            }
            WeaponKind::ArcMortar => {
                if input.mouse_left && self.fire_cooldown <= 0.0 {
                    let launch_vel = shot_dir * 46.0 + Vec3::Y * 9.5;
                    self.projectiles.push(Projectile {
                        kind: ProjectileKind::PlayerMortar,
                        pos: muzzle,
                        vel: launch_vel,
                        damage: 115.0,
                        blast_radius: 14.0,
                        age: 0.0,
                        max_age: 4.5,
                        target_enemy_id: None,
                        target_spire_id: None,
                        color: Vec3::new(0.75, 0.35, 1.0),
                        alive: true,
                    });
                    self.fire_cooldown = self.weapon.cooldown();
                    self.weapon_recoil = 0.30;
                    self.cam_shake = self.cam_shake.max(0.16);
                    self.muzzle_flash_timer = 0.09;
                    self.muzzle_pos = muzzle;
                    self.muzzle_color = Vec3::new(2.8, 1.0, 4.5);
                    audio.play(SoundEvent::MortarShot);
                }
            }
        }
    }

    fn find_best_lock_candidate(&self) -> Option<LockTarget> {
        let dir = (self.cam_target - self.cam_pos).normalize_or_zero();
        let mut best: Option<(f32, LockTarget)> = None;

        for e in &self.enemies {
            if !e.alive { continue; }
            let cand = LockTarget::Enemy(e.id);
            if self.lock_targets.contains(&cand) { continue; }
            let to = e.center() - self.cam_pos;
            let dist = to.length();
            if dist > 230.0 || dist < 3.0 { continue; }
            let dot = (to / dist).dot(dir);
            if dot > 0.88 {
                let score = dot - dist * 0.0005;
                if best.map_or(true, |(b, _)| score > b) {
                    best = Some((score, cand));
                }
            }
        }
        for s in &self.spires {
            if !s.alive { continue; }
            let cand = LockTarget::Spire(s.id);
            if self.lock_targets.contains(&cand) { continue; }
            let to = s.core_pos() - self.cam_pos;
            let dist = to.length();
            if dist > 250.0 { continue; }
            let dot = (to / dist).dot(dir);
            if dot > 0.88 {
                let score = dot - dist * 0.0005;
                if best.map_or(true, |(b, _)| score > b) {
                    best = Some((score, cand));
                }
            }
        }
        best.map(|(_, t)| t)
    }

    fn fire_rail_lance(&mut self, muzzle: Vec3, dir: Vec3, audio: &AudioEngine) {
        let mut max_t = 290.0_f32;
        if let Some(hit) = raycast_terrain(muzzle, dir, &self.terrain, max_t) {
            max_t = hit.t;
        }
        for aabb in &self.layout.colliders {
            if let Some(hit) = raycast_aabb(muzzle, dir, aabb, max_t) {
                max_t = max_t.min(hit.t);
            }
        }

        // Rail-lance pierces all enemies along the beam!
        let mut enemy_hits = Vec::new();
        for (idx, e) in self.enemies.iter().enumerate() {
            if !e.alive { continue; }
            if let Some(t) = raycast_sphere(muzzle, dir, e.center(), e.kind.hit_radius() * 1.25) {
                if t < max_t {
                    let crit = e.kind == EnemyKind::Goliath || e.kind == EnemyKind::Interceptor;
                    enemy_hits.push((idx, muzzle + dir * t, crit));
                }
            }
        }
        for (idx, hit_p, crit) in enemy_hits {
            let dmg = if crit { 165.0 } else { 105.0 };
            self.damage_enemy(idx, dmg, hit_p, crit, true, audio);
        }

        let mut spire_hits = Vec::new();
        for (idx, s) in self.spires.iter().enumerate() {
            if !s.alive { continue; }
            if let Some(t) = raycast_sphere(muzzle, dir, s.core_pos(), 3.6) {
                if t < max_t + 4.0 {
                    spire_hits.push((idx, muzzle + dir * t));
                }
            }
        }
        for (idx, hit_p) in spire_hits {
            self.damage_spire(idx, 110.0, hit_p, true, audio);
        }

        let end = muzzle + dir * max_t;
        self.beams.push(BeamFx {
            start: muzzle,
            end,
            color: Vec4::new(4.5, 3.6, 0.9, 1.0),
            width: 0.34,
            age: 0.0,
            max_age: 0.38,
        });
        self.spawn_hit_sparks(end, Vec3::new(1.0, 0.85, 0.3), 14);
    }

    fn update_wingmen(&mut self, dt: f32, audio: &AudioEngine) {
        let p_pos = self.player.position;
        let p_fwd = self.player.forward_xz();
        let p_right = self.player.right_xz();
        let order = self.squad_order;
        let hold_pt = self.hold_point;

        let mut new_projectiles = Vec::new();
        let mut new_particles = Vec::new();

        for w in &mut self.wingmen {
            w.fire_timer = (w.fire_timer - dt).max(0.0);

            // Find nearest threat
            let mut nearest_tgt: Option<(Vec3, f32)> = None;
            let search_origin = if order == SquadOrder::HoldPosition { hold_pt } else { p_pos };
            for e in &self.enemies {
                if !e.alive { continue; }
                let d = e.center().distance(search_origin);
                if d < 145.0 && nearest_tgt.map_or(true, |(_, bd)| d < bd) {
                    nearest_tgt = Some((e.center(), d));
                }
            }
            if nearest_tgt.is_none() {
                for s in &self.spires {
                    if !s.alive { continue; }
                    let d = s.core_pos().distance(search_origin);
                    if d < 165.0 && nearest_tgt.map_or(true, |(_, bd)| d < bd) {
                        nearest_tgt = Some((s.core_pos(), d));
                    }
                }
            }

            // Determine desired flight/ground destination
            let desired_pos = match order {
                SquadOrder::Follow => {
                    let base = p_pos + p_right * w.formation_offset.x + p_fwd * (-w.formation_offset.z);
                    let gy = self.terrain.sample_height(base.x, base.z);
                    let y = (p_pos.y + w.formation_offset.y).max(gy + 0.1);
                    Vec3::new(base.x, y, base.z)
                }
                SquadOrder::AttackTarget => {
                    if let Some((tpos, _)) = nearest_tgt {
                        let orbit = self.time * 0.8 + if w.formation_offset.x < 0.0 { 0.0 } else { 3.14 };
                        let off = Vec3::new(orbit.cos() * 24.0, 0.0, orbit.sin() * 24.0);
                        let gx = tpos.x + off.x;
                        let gz = tpos.z + off.z;
                        let gy = self.terrain.sample_height(gx, gz);
                        Vec3::new(gx, (gy + 11.0).max(tpos.y + 4.0), gz)
                    } else {
                        let base = p_pos + p_right * w.formation_offset.x;
                        Vec3::new(base.x, self.terrain.sample_height(base.x, base.z), base.z)
                    }
                }
                SquadOrder::HoldPosition => {
                    let base = hold_pt + Vec3::new(w.formation_offset.x, 0.0, w.formation_offset.z);
                    let gy = self.terrain.sample_height(base.x, base.z);
                    Vec3::new(base.x, gy + 5.0, base.z)
                }
            };

            let to_dest = desired_pos - w.pos;
            let dist = to_dest.length();
            let desired_vel = if dist > 1.5 {
                to_dest.normalize() * (dist * 1.45).clamp(6.0, 28.0)
            } else {
                Vec3::ZERO
            };
            w.vel = damp_vec3(w.vel, desired_vel, 5.5, dt);
            w.pos += w.vel * dt;
            let ground_h = self.terrain.sample_height(w.pos.x, w.pos.z);
            if w.pos.y <= ground_h + 0.15 {
                w.pos.y = ground_h;
                w.grounded = true;
            } else {
                w.grounded = false;
            }
            w.thruster = damp(w.thruster, if w.grounded { 0.0 } else { 0.85 }, 8.0, dt);
            if w.grounded {
                w.gait_phase += Vec2::new(w.vel.x, w.vel.z).length() * dt * 0.8;
            } else if self.rng.f32() < 0.35 {
                new_particles.push(Particle {
                    pos: w.pos + Vec3::Y * 1.1,
                    vel: Vec3::new(0.0, -4.5, 0.0) + self.rng.unit_vec3() * 1.0,
                    color: Vec4::new(0.3, 1.9, 3.0, 0.75),
                    size: 0.22,
                    age: 0.0,
                    max_age: 0.22,
                    gravity: 0.0,
                });
            }

            // Face target and engage!
            if let Some((tpos, _)) = nearest_tgt {
                let to_t = tpos - (w.pos + Vec3::Y * 1.6);
                let target_yaw = (-to_t.x).atan2(-to_t.z);
                w.yaw = target_yaw;
                w.pitch = (to_t.y / to_t.length().max(0.1)).asin().clamp(-0.9, 0.9);
                if w.fire_timer <= 0.0 {
                    let dir = to_t.normalize_or_zero();
                    new_projectiles.push(Projectile {
                        kind: ProjectileKind::WingmanPulse,
                        pos: w.pos + Vec3::Y * 1.55 + dir * 0.9,
                        vel: dir * 115.0,
                        damage: 14.0,
                        blast_radius: 0.0,
                        age: 0.0,
                        max_age: 2.0,
                        target_enemy_id: None,
                        target_spire_id: None,
                        color: Vec3::new(0.35, 0.95, 0.75),
                        alive: true,
                    });
                    w.fire_timer = self.rng.range(0.32, 0.55);
                }
            } else if Vec2::new(w.vel.x, w.vel.z).length() > 1.0 {
                w.yaw = (-w.vel.x).atan2(-w.vel.z);
                w.pitch = 0.0;
            }
        }
        let _ = audio;
        self.projectiles.extend(new_projectiles);
        self.particles.extend(new_particles);
    }

    fn update_spires_and_enemies(&mut self, dt: f32, _audio: &AudioEngine) {
        // Spore Spires periodically reinforce the swarm up to cap
        let alive_enemies = self.enemies.iter().filter(|e| e.alive).count();
        let mut to_spawn = Vec::new();
        for spire in &mut self.spires {
            if !spire.alive { continue; }
            spire.spawn_timer -= dt;
            if spire.spawn_timer <= 0.0 && alive_enemies + to_spawn.len() < 20 {
                let roll = self.rng.f32();
                let kind = if roll < 0.52 {
                    EnemyKind::Swarmer
                } else if roll < 0.78 {
                    EnemyKind::Interceptor
                } else {
                    EnemyKind::Spitter
                };
                let off = self.rng.unit_xz() * self.rng.range(8.0, 16.0);
                to_spawn.push((kind, spire.pos + off));
                spire.spawn_timer = self.rng.range(9.5, 14.0);
            }
        }
        for (k, p) in to_spawn {
            self.spawn_enemy(k, p);
        }

        let p_center = self.player.position + Vec3::Y * 1.25;
        let mut enemy_shots = Vec::new();

        for e in &mut self.enemies {
            if !e.alive { continue; }
            e.hit_flash = (e.hit_flash - dt * 6.0).max(0.0);
            e.anim_phase += dt * 5.5;
            e.fire_timer -= dt;

            let to_p = p_center - e.center();
            let dist = to_p.length().max(0.1);
            let dir_xz = Vec3::new(to_p.x, 0.0, to_p.z).normalize_or_zero();
            e.yaw = (-to_p.x).atan2(-to_p.z);

            match e.kind {
                EnemyKind::Swarmer => {
                    let perp = Vec3::new(-dir_xz.z, 0.0, dir_xz.x) * e.strafe_sign;
                    let move_dir = if dist > 12.0 {
                        (dir_xz + perp * 0.35).normalize()
                    } else {
                        (perp * 0.8 + dir_xz * 0.25).normalize()
                    };
                    let speed = 13.5;
                    e.vel = damp_vec3(e.vel, move_dir * speed, 6.0, dt);
                    e.pos += e.vel * dt;
                    e.pos.y = self.terrain.sample_height(e.pos.x, e.pos.z);

                    if dist < 48.0 && e.fire_timer <= 0.0 {
                        let aim = (p_center + self.player.velocity * 0.18 - e.center()).normalize_or_zero();
                        enemy_shots.push((ProjectileKind::EnemyPlasma, e.center() + aim * 1.1, aim * 44.0, 10.0, Vec3::new(0.45, 1.0, 0.25)));
                        e.fire_timer = self.rng.range(1.6, 2.5);
                    }
                }
                EnemyKind::Spitter => {
                    // Holds mid-long distance and lobs bright magenta plasma artillery
                    let desired_v = if dist > 78.0 {
                        dir_xz * 7.5
                    } else if dist < 34.0 {
                        -dir_xz * 6.0
                    } else {
                        Vec3::new(-dir_xz.z, 0.0, dir_xz.x) * e.strafe_sign * 4.5
                    };
                    e.vel = damp_vec3(e.vel, desired_v, 4.0, dt);
                    e.pos += e.vel * dt;
                    e.pos.y = self.terrain.sample_height(e.pos.x, e.pos.z);

                    if dist < 135.0 && e.fire_timer <= 0.0 {
                        let aim = (p_center + self.player.velocity * 0.35 - e.center()).normalize_or_zero();
                        enemy_shots.push((ProjectileKind::EnemyHeavy, e.center() + aim * 1.6, aim * 52.0, 16.0, Vec3::new(1.0, 0.22, 0.78)));
                        e.fire_timer = self.rng.range(2.2, 3.2);
                    }
                }
                EnemyKind::Interceptor => {
                    // Swooping aerial dogfighter!
                    let perp = Vec3::new(-dir_xz.z, 0.0, dir_xz.x) * e.strafe_sign;
                    let target_alt = self.terrain.sample_height(e.pos.x, e.pos.z) + 18.0 + (e.anim_phase * 0.35).sin() * 8.0;
                    let desire = if dist > 55.0 {
                        dir_xz * 26.0 + perp * 12.0
                    } else {
                        perp * 24.0 - dir_xz * 6.0
                    };
                    let desired_vel = Vec3::new(desire.x, (target_alt - e.pos.y) * 2.2, desire.z);
                    e.vel = damp_vec3(e.vel, desired_vel, 3.2, dt);
                    e.pos += e.vel * dt;
                    e.pitch = (e.vel.y * 0.03).clamp(-0.5, 0.5);

                    if dist < 115.0 && e.fire_timer <= 0.0 {
                        let aim = (p_center + self.player.velocity * 0.22 - e.center()).normalize_or_zero();
                        enemy_shots.push((ProjectileKind::EnemyPlasma, e.center() + aim * 1.8, aim * 64.0, 11.0, Vec3::new(1.0, 0.32, 0.16)));
                        e.fire_timer = self.rng.range(1.2, 2.0);
                    }
                }
                EnemyKind::Goliath => {
                    // Relentless boss siege walker with twin salvo cannons
                    let move_v = if dist > 48.0 { dir_xz * 6.8 } else { Vec3::new(-dir_xz.z, 0.0, dir_xz.x) * 3.8 };
                    e.vel = damp_vec3(e.vel, move_v, 3.0, dt);
                    e.pos += e.vel * dt;
                    e.pos.y = self.terrain.sample_height(e.pos.x, e.pos.z);

                    if dist < 165.0 && e.fire_timer <= 0.0 {
                        let right = Vec3::new(e.yaw.cos(), 0.0, -e.yaw.sin());
                        for side in [-2.55_f32, 2.55] {
                            let cannon_p = e.pos + Vec3::Y * 4.95 + right * side;
                            let aim = (p_center + self.player.velocity * 0.3 - cannon_p).normalize_or_zero();
                            enemy_shots.push((ProjectileKind::EnemyHeavy, cannon_p + aim * 2.2, aim * 62.0, 18.0, Vec3::new(1.0, 0.36, 0.08)));
                        }
                        e.fire_timer = 1.75;
                    }
                }
            }
        }

        for (kind, pos, vel, dmg, col) in enemy_shots {
            self.projectiles.push(Projectile {
                kind,
                pos,
                vel,
                damage: dmg,
                blast_radius: if kind == ProjectileKind::EnemyHeavy { 4.2 } else { 0.0 },
                age: 0.0,
                max_age: 4.5,
                target_enemy_id: None,
                target_spire_id: None,
                color: col,
                alive: true,
            });
        }
    }

    fn update_projectiles(&mut self, dt: f32, audio: &AudioEngine) {
        let mut explosions: Vec<(Vec3, f32, f32, Vec3, bool)> = Vec::new();
        let mut enemy_hits: Vec<(usize, f32, Vec3, bool)> = Vec::new();
        let mut spire_hits: Vec<(usize, f32, Vec3, bool)> = Vec::new();
        let mut spark_bursts: Vec<(Vec3, Vec3, usize)> = Vec::new();
        let mut player_hit_dmg = 0.0_f32;

        let p_center = self.player.position + Vec3::Y * 1.1;

        for proj in &mut self.projectiles {
            if !proj.alive { continue; }
            proj.age += dt;
            if proj.age >= proj.max_age {
                proj.alive = false;
                continue;
            }

            // Homing guidance for Valkyrie Micro-Missiles!
            if proj.kind == ProjectileKind::PlayerMissile {
                let mut target_pos = None;
                if let Some(eid) = proj.target_enemy_id {
                    target_pos = self.enemies.iter().find(|e| e.alive && e.id == eid).map(|e| e.center());
                } else if let Some(sid) = proj.target_spire_id {
                    target_pos = self.spires.iter().find(|s| s.alive && s.id == sid).map(|s| s.core_pos());
                }
                if let Some(tp) = target_pos {
                    let desired = (tp - proj.pos).normalize_or_zero() * 68.0;
                    proj.vel = damp_vec3(proj.vel, desired, 6.2, dt);
                } else {
                    proj.vel = proj.vel.normalize_or_zero() * 62.0;
                }
                if self.rng.f32() < 0.75 {
                    self.particles.push(Particle {
                        pos: proj.pos,
                        vel: self.rng.unit_vec3() * 1.2,
                        color: Vec4::new(3.2, 1.3, 0.35, 0.85),
                        size: 0.25,
                        age: 0.0,
                        max_age: 0.32,
                        gravity: 0.0,
                    });
                }
            } else if proj.kind == ProjectileKind::PlayerMortar {
                proj.vel.y -= self.biome_cfg.gravity * 1.05 * dt;
                if self.rng.f32() < 0.65 {
                    self.particles.push(Particle {
                        pos: proj.pos,
                        vel: self.rng.unit_vec3() * 1.5,
                        color: Vec4::new(2.2, 0.9, 3.8, 0.85),
                        size: 0.28,
                        age: 0.0,
                        max_age: 0.28,
                        gravity: 0.0,
                    });
                }
            }

            proj.pos += proj.vel * dt;

            // Terrain collision
            let gh = self.terrain.sample_height(proj.pos.x, proj.pos.z);
            if proj.pos.y <= gh {
                proj.alive = false;
                if proj.blast_radius > 0.0 {
                    let friendly = !matches!(proj.kind, ProjectileKind::EnemyPlasma | ProjectileKind::EnemyHeavy);
                    explosions.push((proj.pos, proj.blast_radius, proj.damage, proj.color, friendly));
                } else {
                    spark_bursts.push((proj.pos, proj.color, 6));
                }
                continue;
            }

            let is_friendly = matches!(
                proj.kind,
                ProjectileKind::PlayerPulse | ProjectileKind::WingmanPulse | ProjectileKind::PlayerMissile | ProjectileKind::PlayerMortar
            );
            let is_player = proj.kind != ProjectileKind::WingmanPulse && is_friendly;

            if is_friendly {
                // Check enemies
                for (idx, e) in self.enemies.iter().enumerate() {
                    if !e.alive { continue; }
                    if proj.pos.distance(e.center()) <= e.kind.hit_radius() + 0.55 {
                        proj.alive = false;
                        if proj.blast_radius > 0.0 {
                            explosions.push((proj.pos, proj.blast_radius, proj.damage, proj.color, true));
                        } else {
                            enemy_hits.push((idx, proj.damage, proj.pos, is_player));
                        }
                        break;
                    }
                }
                if !proj.alive { continue; }

                // Check Spore Spires
                for (idx, s) in self.spires.iter().enumerate() {
                    if !s.alive { continue; }
                    if proj.pos.distance(s.core_pos()) <= 3.6 || (Vec2::new(proj.pos.x - s.pos.x, proj.pos.z - s.pos.z).length() < 2.5 && proj.pos.y < s.pos.y + 15.5) {
                        proj.alive = false;
                        if proj.blast_radius > 0.0 {
                            explosions.push((proj.pos, proj.blast_radius, proj.damage, proj.color, true));
                        } else {
                            spire_hits.push((idx, proj.damage, proj.pos, is_player));
                        }
                        break;
                    }
                }
            } else {
                // Enemy shot vs Player
                if proj.pos.distance(p_center) <= 1.25 {
                    proj.alive = false;
                    player_hit_dmg += proj.damage;
                    spark_bursts.push((proj.pos, proj.color, 8));
                }
            }
        }

        self.projectiles.retain(|p| p.alive);

        for (p, col, cnt) in spark_bursts {
            self.spawn_hit_sparks(p, col, cnt);
        }

        for (idx, dmg, p, from_player) in enemy_hits {
            self.damage_enemy(idx, dmg, p, false, from_player, audio);
        }
        for (idx, dmg, p, from_player) in spire_hits {
            self.damage_spire(idx, dmg, p, from_player, audio);
        }
        for (center, radius, dmg, col, friendly) in explosions {
            self.spawn_explosion(center, radius, dmg, col, friendly, audio);
        }
        if player_hit_dmg > 0.0 {
            self.damage_player(player_hit_dmg, audio);
        }
        self.enemies.retain(|e| e.alive);
    }

    fn damage_enemy(&mut self, idx: usize, dmg: f32, hit_pos: Vec3, crit: bool, from_player: bool, audio: &AudioEngine) {
        if idx >= self.enemies.len() || !self.enemies[idx].alive {
            return;
        }
        self.enemies[idx].hp -= dmg;
        self.enemies[idx].hit_flash = 1.0;
        self.spawn_hit_sparks(hit_pos, Vec3::new(1.0, 0.72, 0.25), if crit { 14 } else { 7 });

        if self.enemies[idx].hp <= 0.0 {
            self.enemies[idx].alive = false;
            let pos = self.enemies[idx].center();
            let kind = self.enemies[idx].kind;
            let val = kind.score_value();
            self.kills += 1;
            self.combo += 1;
            self.combo_timer = 5.0;
            self.score += val * self.combo.min(8);
            if from_player {
                self.hitmarker_timer = 0.26;
                self.hitmarker_crit = crit;
                self.hitmarker_kill = true;
                audio.play(SoundEvent::KillConfirm);
            }
            let blast_r = if kind == EnemyKind::Goliath { 18.0 } else { 6.0 };
            self.spawn_explosion(pos, blast_r, 0.0, Vec3::new(1.0, 0.42, 0.14), false, audio);
            if kind == EnemyKind::Goliath {
                self.cam_shake = 0.65;
                self.push_comms(format!("ORBITAL COMMAND: {} ELIMINATED!", kind.name()), Vec4::new(0.3, 1.0, 0.55, 1.0));
            } else if from_player && self.combo >= 3 {
                self.push_comms(format!("SUIT AI: {} DOWN // {}X COMBO!", kind.name(), self.combo), Vec4::new(1.0, 0.80, 0.25, 1.0));
            }
        } else if from_player {
            self.hitmarker_timer = 0.15;
            self.hitmarker_crit = crit;
            self.hitmarker_kill = false;
            audio.play(SoundEvent::HitConfirm { crit });
        }
    }

    fn damage_spire(&mut self, idx: usize, dmg: f32, hit_pos: Vec3, from_player: bool, audio: &AudioEngine) {
        if idx >= self.spires.len() || !self.spires[idx].alive {
            return;
        }
        self.spires[idx].hp -= dmg;
        self.spires[idx].hit_flash = 1.0;
        self.spawn_hit_sparks(hit_pos, self.biome_cfg.spire_glow, 9);

        if self.spires[idx].hp <= 0.0 {
            self.spires[idx].alive = false;
            let label = self.spires[idx].label;
            let pos = self.spires[idx].core_pos();
            self.score += 1000;
            self.cam_shake = 0.55;
            self.spawn_explosion(pos, 22.0, 40.0, self.biome_cfg.spire_glow, true, audio);
            audio.play(SoundEvent::KillConfirm);
            self.push_comms(format!("ORBITAL COMMAND: {} DESTROYED!", label), Vec4::new(0.3, 1.0, 0.65, 1.0));
        } else if from_player {
            self.hitmarker_timer = 0.14;
            self.hitmarker_crit = false;
            self.hitmarker_kill = false;
            audio.play(SoundEvent::HitConfirm { crit: false });
        }
    }

    fn damage_player(&mut self, dmg: f32, audio: &AudioEngine) {
        self.shield_regen_delay = 3.5;
        self.damage_fx = (self.damage_fx + 0.55).min(1.0);
        self.cam_shake = self.cam_shake.max(0.22);
        if self.shield > 0.0 {
            self.shield -= dmg;
            if self.shield <= 0.0 {
                self.hull += self.shield;
                self.shield = 0.0;
                audio.play(SoundEvent::ShieldBreak);
                self.push_comms("SUIT AI: WARNING — DEFLECTOR SHIELDS DEPLETED!".into(), Vec4::new(1.0, 0.28, 0.22, 1.0));
            } else {
                audio.play(SoundEvent::ShieldHit);
            }
        } else {
            self.hull -= dmg;
            audio.play(SoundEvent::ShieldHit);
        }
        if self.hull <= 0.0 {
            self.hull = 0.0;
            self.mode = GameMode::GameOver;
            audio.play(SoundEvent::Explosion { intensity: 1.0 });
        }
    }

    fn spawn_hit_sparks(&mut self, pos: Vec3, color: Vec3, count: usize) {
        for _ in 0..count {
            let v = self.rng.unit_vec3() * self.rng.range(6.0, 19.0) + Vec3::Y * 3.0;
            self.particles.push(Particle {
                pos,
                vel: v,
                color: Vec4::new(color.x * 3.5, color.y * 3.5, color.z * 3.5, 1.0),
                size: self.rng.range(0.14, 0.30),
                age: 0.0,
                max_age: self.rng.range(0.20, 0.48),
                gravity: 14.0,
            });
        }
    }

    fn spawn_explosion(&mut self, center: Vec3, radius: f32, dmg: f32, color: Vec3, damage_enemies: bool, audio: &AudioEngine) {
        audio.play(SoundEvent::Explosion { intensity: (radius / 12.0).clamp(0.4, 1.3) });
        self.shockwaves.push(Shockwave {
            center,
            color,
            max_radius: radius,
            age: 0.0,
            max_age: 0.45,
        });
        for _ in 0..26 {
            let v = self.rng.unit_vec3() * self.rng.range(4.0, radius * 2.2) + Vec3::Y * self.rng.range(2.0, 9.0);
            self.particles.push(Particle {
                pos: center + self.rng.unit_vec3() * 0.6,
                vel: v,
                color: Vec4::new(color.x * 4.2, color.y * 3.2, color.z * 2.4, 1.0),
                size: self.rng.range(0.32, 0.78),
                age: 0.0,
                max_age: self.rng.range(0.30, 0.75),
                gravity: 8.0,
            });
        }

        if damage_enemies && dmg > 0.0 {
            let mut hits = Vec::new();
            for (idx, e) in self.enemies.iter().enumerate() {
                if !e.alive { continue; }
                let d = e.center().distance(center);
                if d < radius + e.kind.hit_radius() {
                    let falloff = (1.0 - d / (radius + e.kind.hit_radius())).clamp(0.25, 1.0);
                    hits.push((idx, dmg * falloff, e.center()));
                }
            }
            for (idx, d, p) in hits {
                self.damage_enemy(idx, d, p, false, true, audio);
            }
            let mut sh = Vec::new();
            for (idx, s) in self.spires.iter().enumerate() {
                if !s.alive { continue; }
                let d = s.core_pos().distance(center);
                if d < radius + 4.0 {
                    sh.push((idx, dmg * 0.85, s.core_pos()));
                }
            }
            for (idx, d, p) in sh {
                self.damage_spire(idx, d, p, true, audio);
            }
        }
    }

    fn update_fx_only(&mut self, dt: f32) {
        for p in &mut self.particles {
            p.age += dt;
            p.vel.y -= p.gravity * dt;
            p.pos += p.vel * dt;
        }
        self.particles.retain(|p| p.age < p.max_age);
        for b in &mut self.beams {
            b.age += dt;
        }
        self.beams.retain(|b| b.age < b.max_age);
        for sw in &mut self.shockwaves {
            sw.age += dt;
        }
        self.shockwaves.retain(|sw| sw.age < sw.max_age);
    }

    /// Build all GPU render instances, point lights, and additive FX vertices for this frame.
    pub fn build_render_batches(
        &self,
        cam: &Camera,
        instances: &mut [Vec<InstanceData>],
        lights: &mut Vec<PointLight>,
        fx_verts: &mut Vec<FxVertex>,
    ) {
        for list in instances.iter_mut() {
            list.clear();
        }
        lights.clear();
        fx_verts.clear();

        // 1. Static Outpost, Drop Pods & Biome Crags
        for prop in &self.layout.props {
            instances[prop.mesh as usize].push(InstanceData::new(prop.transform, prop.tint));
        }

        // 2. Alien Spore Spires + Floating Rotating Energy Rings
        for spire in &self.spires {
            if !spire.alive { continue; }
            let flash = Vec3::splat(spire.hit_flash * 2.5);
            instances[MeshId::SporeSpire as usize].push(InstanceData::with_emissive(
                Mat4::from_translation(spire.pos),
                Vec3::ONE,
                flash,
                1.0,
            ));
            let ring_y = spire.pos.y + 7.6 + (self.time * 1.8).sin() * 0.6;
            let r1 = Quat::from_rotation_y(spire.ring_angle) * Quat::from_rotation_x(0.32);
            let r2 = Quat::from_rotation_y(-spire.ring_angle * 1.4) * Quat::from_rotation_z(-0.40);
            instances[MeshId::SpireRing as usize].push(InstanceData::new(
                Mat4::from_rotation_translation(r1, Vec3::new(spire.pos.x, ring_y, spire.pos.z)),
                Vec3::ONE,
            ));
            instances[MeshId::SpireRing as usize].push(InstanceData::new(
                Mat4::from_scale_rotation_translation(
                    Vec3::splat(0.76),
                    r2,
                    Vec3::new(spire.pos.x, ring_y + 1.8, spire.pos.z),
                ),
                Vec3::ONE,
            ));
            lights.push(PointLight {
                position: spire.core_pos(),
                radius: 42.0,
                color: self.biome_cfg.spire_glow * 18.0,
            });
        }

        // 3. Player Exo-Suit (hidden in First-Person mode)
        if !self.first_person || self.mode == GameMode::Title {
            emit_exosuit_instances(
                instances,
                self.player.position,
                self.player.yaw,
                self.cam_pitch,
                self.player.lean_pitch,
                self.player.lean_roll,
                self.player.gait_phase,
                self.player.grounded,
                self.player.thruster_intensity,
                self.player.afterburner_intensity,
                self.weapon,
                self.weapon_recoil,
                Vec3::ONE,
            );
        }
        if self.player.thruster_intensity > 0.1 || self.player.afterburner_intensity > 0.1 {
            let intensity = (self.player.thruster_intensity + self.player.afterburner_intensity * 1.4) * 10.0;
            let col = if self.player.afterburner_intensity > 0.3 {
                Vec3::new(1.0, 0.52, 0.18) * intensity
            } else {
                Vec3::new(0.22, 0.85, 1.0) * intensity
            };
            lights.push(PointLight {
                position: self.player.position + Vec3::Y * 1.0,
                radius: 18.0,
                color: col,
            });
        }

        // 4. AI Squad Wingmen (ECHO-2 & ECHO-3)
        for w in &self.wingmen {
            emit_exosuit_instances(
                instances,
                w.pos,
                w.yaw,
                w.pitch,
                if w.grounded { 0.08 } else { 0.32 },
                0.0,
                w.gait_phase,
                w.grounded,
                w.thruster,
                0.0,
                WeaponKind::PulseCarbine,
                0.0,
                w.armor_tint,
            );
        }

        // 5. Alien Enemies
        for e in &self.enemies {
            if !e.alive { continue; }
            let bob = match e.kind {
                EnemyKind::Interceptor => (e.anim_phase * 0.7).sin() * 0.45,
                EnemyKind::Swarmer => (e.anim_phase * 2.2).sin().abs() * 0.18,
                _ => 0.0,
            };
            let rot = Quat::from_rotation_y(e.yaw) * Quat::from_rotation_x(e.pitch);
            let mat = Mat4::from_rotation_translation(rot, e.pos + Vec3::Y * bob);
            let flash = Vec3::splat(e.hit_flash * 3.0);
            instances[e.kind.mesh_id() as usize].push(InstanceData::with_emissive(mat, Vec3::ONE, flash, 1.0));
        }

        // 6. Projectiles
        for p in &self.projectiles {
            let dir = p.vel.normalize_or_zero();
            let rot = Quat::from_rotation_arc(Vec3::NEG_Z, dir);
            match p.kind {
                ProjectileKind::PlayerMissile => {
                    instances[MeshId::Missile as usize].push(InstanceData::new(
                        Mat4::from_scale_rotation_translation(Vec3::splat(1.25), rot, p.pos),
                        Vec3::ONE,
                    ));
                }
                ProjectileKind::PlayerMortar | ProjectileKind::EnemyHeavy => {
                    let s = if p.kind == ProjectileKind::PlayerMortar { 0.46 } else { 0.38 };
                    instances[MeshId::GlowSphere as usize].push(InstanceData::with_emissive(
                        Mat4::from_scale_rotation_translation(Vec3::splat(s), Quat::IDENTITY, p.pos),
                        p.color,
                        p.color * 3.2,
                        1.0,
                    ));
                }
                _ => {
                    instances[MeshId::ProjectileBolt as usize].push(InstanceData::with_emissive(
                        Mat4::from_scale_rotation_translation(Vec3::new(1.0, 1.0, 2.2), rot, p.pos),
                        p.color,
                        p.color * 3.0,
                        1.0,
                    ));
                }
            }
        }

        // 7. Shockwaves
        for sw in &self.shockwaves {
            let t = (sw.age / sw.max_age).clamp(0.0, 1.0);
            let r = 0.8 + sw.max_radius * t.sqrt();
            let fade = (1.0 - t) * (1.0 - t);
            instances[MeshId::ShockRing as usize].push(InstanceData::with_emissive(
                Mat4::from_scale_rotation_translation(Vec3::new(r, 1.0 + (1.0 - t) * 2.0, r), Quat::IDENTITY, sw.center),
                sw.color,
                sw.color * (4.5 * fade),
                fade,
            ));
            lights.push(PointLight {
                position: sw.center + Vec3::Y * 1.5,
                radius: sw.max_radius * 2.2,
                color: sw.color * (28.0 * fade),
            });
        }

        // 8. Muzzle flash & static beacon lights
        if self.muzzle_flash_timer > 0.0 {
            lights.push(PointLight {
                position: self.muzzle_pos,
                radius: 16.0,
                color: self.muzzle_color * 6.0,
            });
        }
        for &(pos, col, rad) in &self.layout.static_lights {
            if lights.len() < 16 {
                lights.push(PointLight { position: pos, radius: rad, color: col });
            }
        }

        // 9. Additive FX Billboards (Particles) & Ionized Beams
        let fwd = (cam.target - cam.eye).normalize_or_zero();
        let cam_right = fwd.cross(Vec3::Y).normalize_or_zero();
        let cam_up = cam_right.cross(fwd).normalize_or_zero();

        for p in &self.particles {
            let t = (p.age / p.max_age).clamp(0.0, 1.0);
            let alpha = (1.0 - t) * (1.0 - t);
            let col = Vec4::new(p.color.x, p.color.y, p.color.z, p.color.w * alpha);
            push_fx_billboard(fx_verts, p.pos, cam_right, cam_up, p.size * (1.0 + t * 0.7), col);
        }
        for b in &self.beams {
            let t = (b.age / b.max_age).clamp(0.0, 1.0);
            let alpha = (1.0 - t) * (1.0 - t);
            let w = b.width * (1.0 - t * 0.5);
            let ca = Vec4::new(b.color.x, b.color.y, b.color.z, alpha);
            let cb = Vec4::new(b.color.x * 1.2, b.color.y * 1.2, b.color.z * 1.2, alpha * 0.85);
            push_fx_beam(fx_verts, b.start, b.end, cam.eye, w, ca, cb);
        }
    }

    /// Configure an epic mid-air action diorama for `--screenshot` visual verification!
    pub fn setup_showcase(&mut self, preset: &str) {
        if preset == "title" {
            self.mode = GameMode::Title;
            self.time = 2.4;
            return;
        }
        self.mode = GameMode::Playing;
        self.time = 12.5;
        let base_y = self.terrain.sample_height(0.0, 20.0);

        // Put the player soaring mid-air with jetpacks blazing!
        self.player.position = Vec3::new(-4.0, base_y + 14.5, 22.0);
        self.player.velocity = Vec3::new(5.0, 3.0, -24.0);
        self.player.grounded = false;
        self.player.altitude_agl = 14.5;
        self.player.thruster_intensity = 1.0;
        self.player.afterburner_intensity = if preset == "aerial" { 0.9 } else { 0.45 };
        self.player.lean_pitch = 0.34;
        self.player.lean_roll = -0.18;
        self.player.energy = 74.0;
        self.cam_yaw = 0.14;
        self.cam_pitch = -0.14;
        self.weapon = WeaponKind::MicroMissiles;
        self.score = 4850;
        self.kills = 14;
        self.combo = 4;
        self.combo_timer = 4.2;

        // Place wingmen flying in V-formation nearby
        if self.wingmen.len() >= 2 {
            self.wingmen[0].pos = self.player.position + Vec3::new(-11.0, 2.5, -6.0);
            self.wingmen[0].grounded = false;
            self.wingmen[0].thruster = 1.0;
            self.wingmen[0].yaw = 0.18;
            self.wingmen[1].pos = self.player.position + Vec3::new(12.5, 3.8, -9.0);
            self.wingmen[1].grounded = false;
            self.wingmen[1].thruster = 1.0;
            self.wingmen[1].yaw = -0.12;
        }

        // Bring a dramatic wave of Interceptors, Spitters & Goliath into frame!
        self.enemies.clear();
        let gx = -12.0;
        let gz = -48.0;
        self.spawn_enemy(EnemyKind::Goliath, Vec3::new(gx, 0.0, gz));
        self.boss_spawned = true;
        self.spawn_enemy(EnemyKind::Interceptor, Vec3::new(14.0, base_y + 24.0, -22.0));
        self.spawn_enemy(EnemyKind::Interceptor, Vec3::new(-24.0, base_y + 27.0, -30.0));
        self.spawn_enemy(EnemyKind::Spitter, Vec3::new(18.0, 0.0, -34.0));
        self.spawn_enemy(EnemyKind::Swarmer, Vec3::new(-6.0, 0.0, -16.0));
        self.spawn_enemy(EnemyKind::Swarmer, Vec3::new(6.0, 0.0, -20.0));

        // Lock onto 2 targets for HUD lock-on diamond showcase!
        if self.enemies.len() >= 2 {
            self.lock_targets = vec![
                LockTarget::Enemy(self.enemies[0].id),
                LockTarget::Enemy(self.enemies[1].id),
            ];
            self.locking = true;
        }

        // Add active plasma bolts, ionized rail beam, thruster sparks, and a live shockwave!
        let mpos = self.player.position + Vec3::new(0.6, 1.5, -1.0);
        let gcenter = self.enemies[0].center();
        self.beams.push(BeamFx {
            start: self.wingmen[0].pos + Vec3::Y * 1.5,
            end: gcenter + Vec3::new(0.0, 1.2, 0.0),
            color: Vec4::new(0.4, 3.4, 4.5, 1.0),
            width: 0.28,
            age: 0.04,
            max_age: 0.4,
        });
        self.shockwaves.push(Shockwave {
            center: gcenter,
            color: Vec3::new(1.0, 0.45, 0.15),
            max_radius: 12.0,
            age: 0.14,
            max_age: 0.45,
        });
        for i in 0..3 {
            let frac = 0.25 + i as f32 * 0.22;
            self.projectiles.push(Projectile {
                kind: ProjectileKind::PlayerMissile,
                pos: mpos.lerp(gcenter, frac) + Vec3::new((i as f32 - 1.0) * 3.5, 2.5, 0.0),
                vel: (gcenter - mpos).normalize() * 55.0,
                damage: 50.0,
                blast_radius: 6.0,
                age: 0.2,
                max_age: 4.0,
                target_enemy_id: None,
                target_spire_id: None,
                color: Vec3::new(1.0, 0.48, 0.14),
                alive: true,
            });
        }
        self.spawn_hit_sparks(gcenter, Vec3::new(1.0, 0.55, 0.15), 24);
        // Dispersed thruster trail embers behind the soaring Exo-Suit
        for i in 0..10 {
            let t = i as f32 * 0.35;
            self.particles.push(Particle {
                pos: self.player.position + Vec3::new(0.0, 0.5 - t * 0.6, 0.8 + t * 1.1) + self.rng.unit_vec3() * 0.25,
                vel: Vec3::ZERO,
                color: Vec4::new(0.4, 1.8, 3.0, 0.85),
                size: 0.28 + t * 0.08,
                age: t * 0.08,
                max_age: 0.5,
                gravity: 0.0,
            });
        }

        let look = self.look_dir();
        let right = look.cross(Vec3::Y).normalize();
        if preset == "hero" {
            // Dramatic cinematic 3/4 side-rear view showcasing the Exo-Suit, wingmen & skyline!
            self.cam_pos = self.player.position + Vec3::new(4.8, 1.9, 5.4);
            self.cam_target = self.player.position + Vec3::new(-4.0, 1.4, -22.0);
        } else {
            let pivot = self.player.position + Vec3::Y * 2.0 + right * 1.15;
            self.cam_pos = pivot - look * 6.0 + Vec3::Y * 0.6;
            self.cam_target = pivot + look * 60.0;
        }
        self.boost_fx = if preset == "aerial" { 0.7 } else { 0.25 };
    }
}

#[allow(clippy::too_many_arguments)]
fn emit_exosuit_instances(
    instances: &mut [Vec<InstanceData>],
    pos: Vec3,
    yaw: f32,
    aim_pitch: f32,
    lean_pitch: f32,
    lean_roll: f32,
    gait_phase: f32,
    grounded: bool,
    thruster: f32,
    afterburner: f32,
    weapon: WeaponKind,
    recoil: f32,
    tint: Vec3,
) {
    let pelvis_h = if grounded { 0.96 + (gait_phase * 2.0).sin().abs() * 0.04 } else { 1.04 };
    let root_rot = Quat::from_rotation_y(yaw)
        * Quat::from_rotation_z(lean_roll)
        * Quat::from_rotation_x(-lean_pitch);
    let root_mat = Mat4::from_rotation_translation(root_rot, pos + Vec3::Y * pelvis_h);

    // 1. Torso + Helmet + Jetpack Wing-Pods
    instances[MeshId::ExoTorso as usize].push(InstanceData::new(root_mat, tint));

    // 2. Articulated Legs (stride on ground, dynamic swept tuck while jetpacking!)
    for side in [-1.0_f32, 1.0_f32] {
        let leg_swing = if grounded {
            (gait_phase + if side < 0.0 { 0.0 } else { std::f32::consts::PI }).sin() * 0.52
        } else {
            -0.30 - afterburner * 0.45 + side * 0.10
        };
        let hip_local = Mat4::from_rotation_translation(
            Quat::from_rotation_x(leg_swing),
            Vec3::new(side * 0.15, 0.0, 0.0),
        );
        instances[MeshId::ExoLeg as usize].push(InstanceData::new(root_mat * hip_local, tint));
    }

    // 3. Articulated Arms + Held Weapon
    let aim_x = (aim_pitch + 1.18 + recoil * 0.35).clamp(0.3, 2.3);
    let left_arm = Mat4::from_rotation_translation(
        Quat::from_rotation_x(aim_x * 0.65) * Quat::from_rotation_z(0.18),
        Vec3::new(-0.36, 0.50, -0.02),
    );
    let right_arm = Mat4::from_rotation_translation(
        Quat::from_rotation_x(aim_x) * Quat::from_rotation_z(-0.08),
        Vec3::new(0.36, 0.50, -0.02),
    );
    instances[MeshId::ExoArm as usize].push(InstanceData::new(root_mat * left_arm, tint));
    instances[MeshId::ExoArm as usize].push(InstanceData::new(root_mat * right_arm, tint));

    let weapon_mesh = match weapon {
        WeaponKind::PulseCarbine => MeshId::WeaponCarbine,
        WeaponKind::RailLance => MeshId::WeaponRail,
        WeaponKind::MicroMissiles => MeshId::WeaponMissile,
        WeaponKind::ArcMortar => MeshId::WeaponMortar,
    };
    let weapon_local = Mat4::from_rotation_translation(
        Quat::from_rotation_x(aim_pitch + recoil * 0.25),
        Vec3::new(0.38, 0.44, -0.34 + recoil * 0.14),
    );
    instances[weapon_mesh as usize].push(InstanceData::new(root_mat * weapon_local, Vec3::ONE));

    // 4. Dynamic Twin Thruster Exhaust Plumes!
    let jet = (thruster + afterburner * 1.15).clamp(0.0, 1.7);
    if jet > 0.05 {
        let plume_tint = if afterburner > 0.3 {
            Vec3::new(1.0, 0.52, 0.16)
        } else {
            Vec3::new(0.32, 0.90, 1.0)
        };
        let em_add = plume_tint * (0.75 * jet);
        for side in [-1.0_f32, 1.0_f32] {
            let noz = Vec3::new(side * 0.24, 0.04, 0.36);
            let scale = Vec3::new(0.68 + jet * 0.18, 0.48 + jet * 0.68, 0.68 + jet * 0.18);
            let plume_rot = Quat::from_rotation_x(0.32 + afterburner * 0.45);
            let local = Mat4::from_scale_rotation_translation(scale, plume_rot, noz);
            instances[MeshId::ThrusterPlume as usize].push(InstanceData::with_emissive(
                root_mat * local,
                plume_tint,
                em_add,
                0.75 + jet * 0.35,
            ));
        }
    }
}
