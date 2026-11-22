use crate::{Aabb, Terrain};
use exo_core::damp;
use glam::{Vec2, Vec3};

/// Control inputs to the 6-DoF Exo-Suit controller.
#[derive(Clone, Copy, Debug, Default)]
pub struct ExoSuitInput {
    /// Local movement wish: x = right [-1..1], y = forward [-1..1]
    pub move_dir: Vec2,
    /// Facing yaw in radians (0 = -Z forward, +yaw turns left/CCW around Y)
    pub yaw: f32,
    /// Camera pitch in radians (used for 3D afterburner flight vectoring)
    pub pitch: f32,
    /// Jump / vertical jump-jets held
    pub jump_jet: bool,
    /// Jump button just pressed this step
    pub jump_pressed: bool,
    /// Afterburner thruster boost held
    pub boost: bool,
    /// Aim-down-sights hover stabilizer held
    pub hover_ads: bool,
    /// Dive-slam triggered while airborne
    pub dive_slam: bool,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct ExoSuitStepEvents {
    pub jumped: bool,
    pub landed_speed: f32,
    pub dive_slam_impact: bool,
    pub boost_burst: bool,
}

/// Complete physical & animation-telemetry state of an Exo-Suit.
#[derive(Clone, Debug)]
pub struct ExoSuitState {
    pub position: Vec3,
    pub velocity: Vec3,
    pub yaw: f32,
    pub grounded: bool,
    pub coyote_timer: f32,
    pub altitude_agl: f32,
    /// Thruster capacitor [0..100]
    pub energy: f32,
    pub overheated: bool,
    pub jetting: bool,
    pub boosting: bool,
    pub hovering: bool,
    pub dive_slamming: bool,
    /// Visual telemetry for procedural character animation
    pub thruster_intensity: f32,
    pub afterburner_intensity: f32,
    pub lean_pitch: f32,
    pub lean_roll: f32,
    pub gait_phase: f32,
    pub boost_cooldown: f32,
}

impl ExoSuitState {
    pub fn new(spawn_pos: Vec3, yaw: f32) -> Self {
        Self {
            position: spawn_pos,
            velocity: Vec3::ZERO,
            yaw,
            grounded: true,
            coyote_timer: 0.0,
            altitude_agl: 0.0,
            energy: 100.0,
            overheated: false,
            jetting: false,
            boosting: false,
            hovering: false,
            dive_slamming: false,
            thruster_intensity: 0.0,
            afterburner_intensity: 0.0,
            lean_pitch: 0.0,
            lean_roll: 0.0,
            gait_phase: 0.0,
            boost_cooldown: 0.0,
        }
    }

    pub fn forward_xz(&self) -> Vec3 {
        Vec3::new(-self.yaw.sin(), 0.0, -self.yaw.cos())
    }

    pub fn right_xz(&self) -> Vec3 {
        Vec3::new(self.yaw.cos(), 0.0, -self.yaw.sin())
    }

    pub fn speed(&self) -> f32 {
        self.velocity.length()
    }

    pub fn step(
        &mut self,
        input: &ExoSuitInput,
        terrain: &Terrain,
        colliders: &[Aabb],
        gravity: f32,
        dt: f32,
    ) -> ExoSuitStepEvents {
        let mut events = ExoSuitStepEvents::default();
        self.yaw = input.yaw;
        self.boost_cooldown = (self.boost_cooldown - dt).max(0.0);

        let forward = self.forward_xz();
        let right = self.right_xz();
        let wish_raw = forward * input.move_dir.y + right * input.move_dir.x;
        let wish_dir = if wish_raw.length_squared() > 1e-4 {
            wish_raw.normalize()
        } else {
            Vec3::ZERO
        };

        // Coyote time window for crisp jump feel
        if self.grounded {
            self.coyote_timer = 0.14;
        } else {
            self.coyote_timer = (self.coyote_timer - dt).max(0.0);
        }

        // Initial jump impulse
        if input.jump_pressed && self.coyote_timer > 0.0 {
            self.velocity.y = 11.2;
            self.grounded = false;
            self.coyote_timer = 0.0;
            events.jumped = true;
        }

        // Dive-slam activation when airborne
        if input.dive_slam && !self.grounded && self.altitude_agl > 3.5 && !self.dive_slamming {
            self.dive_slamming = true;
            self.velocity.y = -34.0;
            self.velocity.x *= 0.45;
            self.velocity.z *= 0.45;
        }

        // Energy & overheat recovery
        if self.overheated && self.energy > 24.0 {
            self.overheated = false;
        }

        let can_thrust = !self.overheated && self.energy > 0.5;
        self.jetting = false;
        self.boosting = false;
        self.hovering = false;

        // 1. Afterburner Boost (ground power-sprint or supersonic aerial flight!)
        if input.boost && can_thrust && !self.dive_slamming {
            let boost_vec = if !self.grounded {
                // 3D flight vector follows look pitch gently when moving forward
                let aim_3d = Vec3::new(
                    -input.yaw.sin() * input.pitch.cos(),
                    input.pitch.sin() * 0.65,
                    -input.yaw.cos() * input.pitch.cos(),
                )
                .normalize();
                if input.move_dir.y > 0.15 {
                    (aim_3d + right * input.move_dir.x * 0.45).normalize()
                } else if wish_dir.length_squared() > 0.01 {
                    wish_dir
                } else {
                    aim_3d
                }
            } else if wish_dir.length_squared() > 0.01 {
                wish_dir
            } else {
                forward
            };

            if self.boost_cooldown <= 0.0 {
                // Crisp initial thruster dash burst!
                self.velocity += boost_vec * 9.5;
                self.boost_cooldown = 0.42;
                events.boost_burst = true;
            }

            let target_speed = if self.grounded { 24.0 } else { 36.0 };
            let accel = if self.grounded { 52.0 } else { 44.0 };
            let desired = boost_vec * target_speed;
            self.velocity.x += (desired.x - self.velocity.x) * (accel * dt).min(1.0);
            self.velocity.z += (desired.z - self.velocity.z) * (accel * dt).min(1.0);
            if !self.grounded {
                // Sustain altitude with lift while boosting!
                let target_vy = desired.y.max(-1.2);
                self.velocity.y += (target_vy - self.velocity.y) * (6.5 * dt).min(1.0);
            }

            self.boosting = true;
            self.energy -= 27.0 * dt;
        } else if self.grounded {
            // Crisp ground running acceleration & friction
            let max_run = 14.5;
            let desired = wish_dir * max_run;
            let accel = if wish_dir.length_squared() > 0.01 {
                14.0
            } else {
                11.0
            };
            self.velocity.x = damp(self.velocity.x, desired.x, accel, dt);
            self.velocity.z = damp(self.velocity.z, desired.z, accel, dt);
        } else {
            // Responsive aerial strafe-jet control
            let max_air = if input.hover_ads { 14.0 } else { 20.0 };
            let air_accel = if input.hover_ads { 6.5 } else { 3.8 };
            if wish_dir.length_squared() > 0.01 {
                let desired = wish_dir * max_air;
                self.velocity.x = damp(self.velocity.x, desired.x, air_accel, dt);
                self.velocity.z = damp(self.velocity.z, desired.z, air_accel, dt);
            } else {
                // Mild aerodynamic drag in air
                let drag = if input.hover_ads { 3.5 } else { 0.7 };
                self.velocity.x = damp(self.velocity.x, 0.0, drag, dt);
                self.velocity.z = damp(self.velocity.z, 0.0, drag, dt);
            }
        }

        // 2. Vertical Jump-Jets (Space)
        if input.jump_jet && can_thrust && !self.dive_slamming {
            let climb_thrust = gravity + 24.0;
            self.velocity.y += climb_thrust * dt;
            if self.velocity.y > 19.5 {
                self.velocity.y = damp(self.velocity.y, 19.5, 6.0, dt);
            }
            self.grounded = false;
            self.jetting = true;
            self.energy -= 22.0 * dt;
        } else if input.hover_ads && !self.grounded && can_thrust && !self.dive_slamming {
            // Precision Hover-ADS inertial damper for mid-air sniping!
            self.velocity.y = damp(self.velocity.y, 0.0, 9.0, dt);
            self.hovering = true;
            self.energy -= 12.0 * dt;
        }

        // Energy recharge & clamp
        if !self.jetting && !self.boosting && !self.hovering {
            let regen = if self.grounded { 42.0 } else { 11.0 };
            self.energy = (self.energy + regen * dt).min(100.0);
        }
        if self.energy <= 0.0 {
            self.energy = 0.0;
            self.overheated = true;
        }

        // 3. Gravity
        if !self.grounded && !self.hovering {
            let g_mult = if self.dive_slamming {
                2.6
            } else if self.boosting {
                0.28
            } else {
                1.0
            };
            self.velocity.y -= gravity * g_mult * dt;
            self.velocity.y = self.velocity.y.max(-52.0);
        }

        // 4. Integrate position
        self.position += self.velocity * dt;

        // Soft arena boundary containment
        let max_r = terrain.playable_radius();
        let horiz = Vec2::new(self.position.x, self.position.z);
        let dist = horiz.length();
        if dist > max_r {
            let n = horiz / dist;
            self.position.x = n.x * max_r;
            self.position.z = n.y * max_r;
            let outward = self.velocity.x * n.x + self.velocity.z * n.y;
            if outward > 0.0 {
                self.velocity.x -= n.x * outward;
                self.velocity.z -= n.y * outward;
            }
        }

        // Ceiling cap so player stays within planetary atmosphere
        let max_alt = 135.0;
        if self.position.y > max_alt {
            self.position.y = max_alt;
            if self.velocity.y > 0.0 {
                self.velocity.y = 0.0;
            }
        }

        // 5. Resolve static AABB colliders (bunkers, landing pads, towers)
        let cap_radius = 0.65;
        let cap_height = 2.05;
        let mut landed_on_collider = false;
        for aabb in colliders {
            if let Some((push, on_top)) = aabb.resolve_capsule(self.position, cap_radius, cap_height)
            {
                self.position += push;
                if on_top && self.velocity.y <= 0.0 {
                    landed_on_collider = true;
                } else if !on_top {
                    // Kill velocity into wall
                    let n = push.normalize_or_zero();
                    let vn = self.velocity.dot(n);
                    if vn < 0.0 {
                        self.velocity -= n * vn;
                    }
                }
            }
        }

        // 6. Terrain ground resolution
        let ground_h = terrain.sample_height(self.position.x, self.position.z);
        let was_grounded = self.grounded;
        let pre_impact_vy = self.velocity.y;

        if self.position.y <= ground_h + 0.04 && self.velocity.y <= 0.5 {
            self.position.y = ground_h;
            self.velocity.y = 0.0;
            self.grounded = true;
        } else if landed_on_collider {
            self.velocity.y = 0.0;
            self.grounded = true;
        } else {
            self.grounded = false;
        }

        if !was_grounded && self.grounded {
            events.landed_speed = (-pre_impact_vy).max(0.0);
            if self.dive_slamming {
                events.dive_slam_impact = true;
                self.dive_slamming = false;
            }
        }

        self.altitude_agl = (self.position.y - ground_h).max(0.0);

        // 7. Procedural animation telemetry
        let target_thrust = if self.jetting {
            1.0
        } else if self.hovering {
            0.55
        } else if self.boosting {
            0.85
        } else {
            0.0
        };
        let target_afterburner = if self.boosting { 1.0 } else { 0.0 };
        self.thruster_intensity = damp(self.thruster_intensity, target_thrust, 14.0, dt);
        self.afterburner_intensity = damp(self.afterburner_intensity, target_afterburner, 12.0, dt);

        let local_fwd = self.velocity.dot(forward);
        let local_right = self.velocity.dot(right);
        let target_pitch = if self.grounded {
            (local_fwd * 0.015).clamp(-0.18, 0.24)
        } else if self.boosting {
            0.55
        } else {
            (local_fwd * 0.022).clamp(-0.35, 0.45)
        };
        let target_roll = (-local_right * 0.022).clamp(-0.42, 0.42);
        self.lean_pitch = damp(self.lean_pitch, target_pitch, 9.0, dt);
        self.lean_roll = damp(self.lean_roll, target_roll, 9.0, dt);

        if self.grounded {
            let horiz_speed = Vec2::new(self.velocity.x, self.velocity.z).length();
            self.gait_phase += horiz_speed * dt * 0.85;
        }

        events
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use exo_core::{BiomeId, FIXED_DT};

    #[test]
    fn jetpack_lifts_suit_and_consumes_energy() {
        let terrain = Terrain::generate(BiomeId::CrimsonMesa);
        let h0 = terrain.sample_height(0.0, 75.0);
        let mut suit = ExoSuitState::new(Vec3::new(0.0, h0, 75.0), 0.0);
        let input = ExoSuitInput {
            jump_jet: true,
            jump_pressed: true,
            ..Default::default()
        };
        for i in 0..120 {
            let mut step_in = input;
            step_in.jump_pressed = i == 0;
            suit.step(&step_in, &terrain, &[], 17.5, FIXED_DT);
        }
        assert!(!suit.grounded);
        assert!(suit.altitude_agl > 8.0, "expected climb, got {}", suit.altitude_agl);
        assert!(suit.energy < 90.0);
    }
}
