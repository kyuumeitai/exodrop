use crate::game::GameMode;
use exo_core::{InputState, Key};
use exo_gfx::HudDrawList;
use glam::{Vec2, Vec4};
use winit::event::{Touch, TouchPhase};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TouchButtonId {
    Fire,
    Jets,
    Boost,
    Slam,
    WeaponNext,
    SquadNext,
    ViewToggle,
    Pause,
}

#[derive(Clone, Copy, Debug)]
struct TouchButton {
    id: TouchButtonId,
    label: &'static str,
    center: Vec2,
    radius: f32,
    color: Vec4,
}

#[derive(Clone, Copy, Debug)]
enum FingerRole {
    MoveStick { origin: Vec2, current: Vec2 },
    AimDrag { last: Vec2 },
    Button(TouchButtonId),
    MenuTap,
}

#[derive(Clone, Copy, Debug)]
struct ActiveFinger {
    id: u64,
    role: FingerRole,
}

/// Multi-touch virtual twin-stick + tactical action button controller for iOS & Android.
pub struct TouchController {
    pub enabled: bool,
    fingers: Vec<ActiveFinger>,
    fire_held_prev: bool,
    jets_held_prev: bool,
}

impl TouchController {
    pub fn new(enabled: bool) -> Self {
        Self {
            enabled,
            fingers: Vec::with_capacity(8),
            fire_held_prev: false,
            jets_held_prev: false,
        }
    }

    fn layout_buttons(w: f32, h: f32) -> [TouchButton; 8] {
        let s = (h / 720.0).clamp(0.75, 1.5);
        let fire_c = Vec2::new(w - 108.0 * s, h - 118.0 * s);
        let jets_c = Vec2::new(w - 92.0 * s, h - 242.0 * s);
        let boost_c = Vec2::new(w - 224.0 * s, h - 104.0 * s);
        let slam_c = Vec2::new(w - 208.0 * s, h - 214.0 * s);
        let wpn_c = Vec2::new(w - 238.0 * s, 54.0 * s);
        let sqd_c = Vec2::new(w - 318.0 * s, 54.0 * s);
        let view_c = Vec2::new(w - 392.0 * s, 54.0 * s);
        let pause_c = Vec2::new(424.0 * s, 90.0 * s);

        [
            TouchButton {
                id: TouchButtonId::Fire,
                label: "FIRE",
                center: fire_c,
                radius: 56.0 * s,
                color: Vec4::new(1.0, 0.34, 0.20, 0.92),
            },
            TouchButton {
                id: TouchButtonId::Jets,
                label: "JETS",
                center: jets_c,
                radius: 46.0 * s,
                color: Vec4::new(0.18, 0.92, 1.0, 0.92),
            },
            TouchButton {
                id: TouchButtonId::Boost,
                label: "BOOST",
                center: boost_c,
                radius: 42.0 * s,
                color: Vec4::new(1.0, 0.68, 0.16, 0.92),
            },
            TouchButton {
                id: TouchButtonId::Slam,
                label: "SLAM",
                center: slam_c,
                radius: 34.0 * s,
                color: Vec4::new(0.72, 0.42, 1.0, 0.90),
            },
            TouchButton {
                id: TouchButtonId::WeaponNext,
                label: "WPN",
                center: wpn_c,
                radius: 30.0 * s,
                color: Vec4::new(0.25, 0.92, 1.0, 0.88),
            },
            TouchButton {
                id: TouchButtonId::SquadNext,
                label: "SQD",
                center: sqd_c,
                radius: 30.0 * s,
                color: Vec4::new(0.30, 1.0, 0.60, 0.88),
            },
            TouchButton {
                id: TouchButtonId::ViewToggle,
                label: "CAM",
                center: view_c,
                radius: 26.0 * s,
                color: Vec4::new(0.85, 0.90, 0.98, 0.80),
            },
            TouchButton {
                id: TouchButtonId::Pause,
                label: "II",
                center: pause_c,
                radius: 24.0 * s,
                color: Vec4::new(0.85, 0.90, 0.98, 0.80),
            },
        ]
    }

    pub fn on_touch(
        &mut self,
        touch: Touch,
        mode: GameMode,
        width: f32,
        height: f32,
        input: &mut InputState,
    ) {
        self.enabled = true;
        let pos = Vec2::new(touch.location.x as f32, touch.location.y as f32);

        match touch.phase {
            TouchPhase::Started => {
                // A new pointer can't share an id with a live one: drop any stale entry (a lost Ended/Cancelled)
                // so it can't keep the stick or a button held forever.
                if let Some(idx) = self.fingers.iter().position(|f| f.id == touch.id) {
                    let stale = self.fingers.remove(idx);
                    if let FingerRole::Button(b) = stale.role {
                        Self::release_button_key(b, input);
                    }
                }
                if mode != GameMode::Playing {
                    self.handle_menu_tap(pos, mode, width, height, input);
                    self.fingers.push(ActiveFinger {
                        id: touch.id,
                        role: FingerRole::MenuTap,
                    });
                    return;
                }

                // Check tactical action buttons first
                let buttons = Self::layout_buttons(width, height);
                for btn in &buttons {
                    if pos.distance(btn.center) <= btn.radius * 1.18 {
                        match btn.id {
                            TouchButtonId::WeaponNext => input.on_key(Key::Tab, true),
                            TouchButtonId::SquadNext => input.on_key(Key::Q, true),
                            TouchButtonId::ViewToggle => input.on_key(Key::V, true),
                            TouchButtonId::Pause => input.on_key(Key::Escape, true),
                            TouchButtonId::Slam => input.on_key(Key::C, true),
                            _ => {}
                        }
                        self.fingers.push(ActiveFinger {
                            id: touch.id,
                            role: FingerRole::Button(btn.id),
                        });
                        return;
                    }
                }

                // Left 42% of screen -> Floating virtual movement thumbstick
                let has_stick = self
                    .fingers
                    .iter()
                    .any(|f| matches!(f.role, FingerRole::MoveStick { .. }));
                if pos.x < width * 0.42 && !has_stick {
                    self.fingers.push(ActiveFinger {
                        id: touch.id,
                        role: FingerRole::MoveStick {
                            origin: pos,
                            current: pos,
                        },
                    });
                } else {
                    // Right side -> Drag-to-look aiming
                    self.fingers.push(ActiveFinger {
                        id: touch.id,
                        role: FingerRole::AimDrag { last: pos },
                    });
                }
            }
            TouchPhase::Moved => {
                for f in &mut self.fingers {
                    if f.id != touch.id {
                        continue;
                    }
                    match &mut f.role {
                        FingerRole::MoveStick { current, .. } => {
                            *current = pos;
                        }
                        FingerRole::AimDrag { last } => {
                            let delta = pos - *last;
                            *last = pos;
                            // Smooth touch sensitivity scaled for mobile screens
                            input.mouse_delta += delta * 2.1;
                        }
                        _ => {}
                    }
                }
            }
            TouchPhase::Ended | TouchPhase::Cancelled => {
                if let Some(idx) = self.fingers.iter().position(|f| f.id == touch.id) {
                    let removed = self.fingers.remove(idx);
                    if let FingerRole::Button(b) = removed.role {
                        Self::release_button_key(b, input);
                    }
                }
            }
        }
    }

    fn release_button_key(b: TouchButtonId, input: &mut InputState) {
        match b {
            TouchButtonId::WeaponNext => input.on_key(Key::Tab, false),
            TouchButtonId::SquadNext => input.on_key(Key::Q, false),
            TouchButtonId::ViewToggle => input.on_key(Key::V, false),
            TouchButtonId::Pause => input.on_key(Key::Escape, false),
            TouchButtonId::Slam => input.on_key(Key::C, false),
            _ => {}
        }
    }

    /// Forget every finger (app suspended or lost focus: Android won't send their Ended events).
    /// The next `apply_to_input` then releases fire / jets / boost and centres the stick.
    pub fn reset(&mut self, input: &mut InputState) {
        for f in self.fingers.drain(..) {
            if let FingerRole::Button(b) = f.role {
                Self::release_button_key(b, input);
            }
        }
    }

    fn handle_menu_tap(
        &self,
        pos: Vec2,
        mode: GameMode,
        _w: f32,
        h: f32,
        input: &mut InputState,
    ) {
        if mode == GameMode::Title {
            // Sector selector cards sit on left side (x: 64..494, y: card_y + idx*52)
            let card_y = h * 0.49;
            if pos.x >= 50.0 && pos.x <= 510.0 {
                for (idx, k) in [Key::Digit1, Key::Digit2, Key::Digit3].iter().enumerate() {
                    let cy = card_y + idx as f32 * 52.0;
                    if pos.y >= cy - 4.0 && pos.y <= cy + 46.0 {
                        input.on_key(*k, true);
                        input.on_key(*k, false);
                        return;
                    }
                }
            }
        }
        // Any other tap starts / resumes / advances / redeploys!
        input.on_key(Key::Enter, true);
        input.on_key(Key::Enter, false);
    }

    /// Sync continuous touch state (virtual stick, fire/jets/boost holds) into `InputState`.
    pub fn apply_to_input(&mut self, height: f32, input: &mut InputState) {
        if !self.enabled {
            return;
        }
        let max_r = (height * 0.115).clamp(52.0, 96.0);
        let mut stick_vec = Vec2::ZERO;
        let mut fire_held = false;
        let mut jets_held = false;
        let mut boost_held = false;

        for f in &self.fingers {
            match f.role {
                FingerRole::MoveStick { origin, current } => {
                    let raw = (current - origin) / max_r;
                    let clamped = if raw.length() > 1.0 { raw.normalize() } else { raw };
                    // Screen Y is down, game move Y is forward (+up)
                    stick_vec = Vec2::new(clamped.x, -clamped.y);
                }
                FingerRole::Button(TouchButtonId::Fire) => fire_held = true,
                FingerRole::Button(TouchButtonId::Jets) => jets_held = true,
                FingerRole::Button(TouchButtonId::Boost) => boost_held = true,
                _ => {}
            }
        }

        input.analog_move = stick_vec;

        // Fire button mirrors LMB with clean pressed/released edges (supports Swarm Pod multi-lock!)
        if fire_held != self.fire_held_prev {
            input.on_mouse_button(true, fire_held);
            self.fire_held_prev = fire_held;
        }
        // Jets mirrors Space (jump + vertical thrusters)
        if jets_held != self.jets_held_prev {
            input.on_key(Key::Space, jets_held);
            self.jets_held_prev = jets_held;
        }
        // Boost mirrors Shift
        if boost_held {
            input.keys_down.insert(Key::Shift);
        } else if self.fingers.iter().all(|f| !matches!(f.role, FingerRole::Button(TouchButtonId::Boost))) {
            // Only clear Shift if touch is driving controls (on mobile there is no keyboard)
            #[cfg(any(target_os = "android", target_os = "ios"))]
            input.keys_down.remove(&Key::Shift);
        }
    }

    /// Render sleek translucent sci-fi virtual twin-stick and action buttons onto the 2D HUD.
    pub fn draw_overlay(&self, mode: GameMode, w: f32, h: f32, hud: &mut HudDrawList) {
        if !self.enabled || mode != GameMode::Playing {
            return;
        }
        let s = (h / 720.0).clamp(0.75, 1.5);
        let max_r = (h * 0.115).clamp(52.0, 96.0);

        // 1. Left Virtual Thumbstick (floating at active touch, or resting guide ring bottom-left)
        let default_origin = Vec2::new(118.0 * s, h - 228.0 * s);
        let (origin, knob, active) = self
            .fingers
            .iter()
            .find_map(|f| match f.role {
                FingerRole::MoveStick { origin, current } => {
                    let delta = current - origin;
                    let clamped = if delta.length() > max_r {
                        delta.normalize() * max_r
                    } else {
                        delta
                    };
                    Some((origin, origin + clamped, true))
                }
                _ => None,
            })
            .unwrap_or((default_origin, default_origin, false));

        let ring_a = if active { 0.72 } else { 0.34 };
        hud.circle_filled(origin, max_r, 28, Vec4::new(0.03, 0.08, 0.14, ring_a * 0.45));
        hud.circle_outline(origin, max_r, 2.2, 28, Vec4::new(0.20, 0.90, 1.0, ring_a));
        hud.circle_filled(knob, 26.0 * s, 20, Vec4::new(0.20, 0.92, 1.0, if active { 0.62 } else { 0.30 }));
        hud.circle_outline(knob, 26.0 * s, 2.0, 20, Vec4::new(0.90, 0.98, 1.0, ring_a + 0.18));
        if !active {
            hud.text_centered_shadow("MOVE", origin.x, origin.y + max_r + 8.0 * s, 1.35 * s, Vec4::new(0.65, 0.85, 0.95, 0.60));
        }

        // 2. Right Tactical Action Buttons
        let buttons = Self::layout_buttons(w, h);
        for btn in &buttons {
            let pressed = self
                .fingers
                .iter()
                .any(|f| matches!(f.role, FingerRole::Button(id) if id == btn.id));
            let fill_a = if pressed { 0.50 } else { 0.20 };
            let edge_a = if pressed { 1.0 } else { 0.72 };
            let fill = Vec4::new(btn.color.x, btn.color.y, btn.color.z, fill_a);
            let edge = Vec4::new(btn.color.x, btn.color.y, btn.color.z, edge_a);

            hud.circle_filled(btn.center, btn.radius, 24, Vec4::new(0.02, 0.05, 0.09, 0.52));
            hud.circle_filled(btn.center, btn.radius * 0.92, 24, fill);
            hud.circle_outline(btn.center, btn.radius, if pressed { 3.2 } else { 2.0 }, 24, edge);
            let scale = if btn.radius > 44.0 * s { 1.85 * s } else { 1.45 * s };
            hud.text_centered_shadow(btn.label, btn.center.x, btn.center.y - 3.5 * scale, scale, Vec4::new(0.96, 0.98, 1.0, 0.95));
        }
    }
}
