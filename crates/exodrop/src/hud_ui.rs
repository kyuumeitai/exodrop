use crate::entities::EnemyKind;
use crate::game::{GameMode, GameState, LockTarget};
use exo_core::{BiomeId, SquadOrder, WeaponKind};
use exo_gfx::{Camera, HudDrawList};
use glam::{Vec2, Vec3, Vec4};

const CYAN: Vec4 = Vec4::new(0.18, 0.92, 1.0, 0.95);
const AMBER: Vec4 = Vec4::new(1.0, 0.66, 0.16, 0.95);
const RED: Vec4 = Vec4::new(1.0, 0.22, 0.24, 0.95);
const GREEN: Vec4 = Vec4::new(0.25, 1.0, 0.58, 0.95);
const WHITE: Vec4 = Vec4::new(0.95, 0.97, 1.0, 0.96);
const MUTED: Vec4 = Vec4::new(0.65, 0.74, 0.84, 0.80);
const PANEL_BG: Vec4 = Vec4::new(0.03, 0.05, 0.09, 0.64);
const PANEL_EDGE: Vec4 = Vec4::new(0.20, 0.78, 0.95, 0.55);

pub fn build_hud(game: &GameState, cam: &Camera, width: f32, height: f32, touch_ui: bool, hud: &mut HudDrawList) {
    hud.clear();
    match game.mode {
        GameMode::Title => build_title_screen(game, width, height, touch_ui, hud),
        GameMode::Playing => build_combat_hud(game, cam, width, height, touch_ui, hud),
        GameMode::Paused => {
            build_combat_hud(game, cam, width, height, touch_ui, hud);
            build_overlay_box(
                width,
                height,
                "MISSION PAUSED",
                if touch_ui {
                    &["[ TAP ] RESUME FLIGHT"]
                } else {
                    &["[ESC / ENTER / TAP] RESUME FLIGHT", "[R] REDEPLOY SECTOR", "[N] NEXT PLANETARY SECTOR"]
                },
                CYAN,
                hud,
            );
        }
        GameMode::SectorCleared => {
            build_combat_hud(game, cam, width, height, touch_ui, hud);
            build_overlay_box(
                width,
                height,
                "SECTOR SECURED!",
                &[
                    &format!("FINAL SCORE: {}   HOSTILES ELIMINATED: {}", game.score, game.kills),
                    "[ENTER / SPACE / TAP] ORBITAL DROP TO NEXT SECTOR",
                ],
                GREEN,
                hud,
            );
        }
        GameMode::GameOver => {
            build_combat_hud(game, cam, width, height, touch_ui, hud);
            build_overlay_box(
                width,
                height,
                "EXO-SUIT CRITICAL FAILURE",
                &[
                    &format!("SCORE: {}   HOSTILES ELIMINATED: {}", game.score, game.kills),
                    "[ENTER / R / TAP] RELAUNCH DROP POD",
                ],
                RED,
                hud,
            );
        }
    }
}

fn build_title_screen(game: &GameState, w: f32, h: f32, touch_ui: bool, hud: &mut HudDrawList) {
    // Sleek cinematic letterbox tint bands
    hud.rect(0.0, 0.0, w, 72.0, Vec4::new(0.02, 0.04, 0.08, 0.72));
    hud.rect(0.0, h - 94.0, w, 94.0, Vec4::new(0.02, 0.04, 0.08, 0.80));
    hud.rect(0.0, 72.0, w, 2.0, PANEL_EDGE);
    hud.rect(0.0, h - 96.0, w, 2.0, PANEL_EDGE);

    // Top telemetry strip
    hud.text_shadow("HELIOS ENGINE // WGPU 26 HDR PBR", 24.0, 28.0, 2.0, MUTED);
    hud.text_right("2026 ORBITAL EDITION", w - 24.0, 28.0, 2.0, CYAN);

    // Hero Logo Center-Left
    let title_x = 64.0;
    let title_y = h * 0.22;
    hud.slanted_rect(title_x - 16.0, title_y - 18.0, 490.0, 134.0, 18.0, Vec4::new(0.02, 0.04, 0.08, 0.70));
    hud.slanted_rect(title_x - 16.0, title_y - 18.0, 8.0, 134.0, 18.0, CYAN);
    hud.text_shadow("EXODROP", title_x + 12.0, title_y, 9.5, WHITE);
    hud.text_shadow("ORBITAL EXO-SUIT TACTICAL ACTION", title_x + 14.0, title_y + 78.0, 2.3, AMBER);

    // Sector Selector Cards
    let card_y = h * 0.49;
    hud.text_shadow("SELECT DROP ZONE [KEYS 1-3]:", title_x, card_y - 26.0, 2.0, MUTED);
    for (idx, b) in BiomeId::ALL.iter().enumerate() {
        let cfg = b.config();
        let selected = *b == game.biome;
        let cy = card_y + idx as f32 * 52.0;
        let bg = if selected {
            Vec4::new(0.06, 0.16, 0.24, 0.85)
        } else {
            PANEL_BG
        };
        let border = if selected { CYAN } else { Vec4::new(0.3, 0.4, 0.5, 0.4) };
        hud.slanted_rect(title_x, cy, 430.0, 42.0, 10.0, bg);
        hud.slanted_rect(title_x, cy, 6.0, 42.0, 10.0, border);
        hud.text_shadow(
            &format!("[{}] {} - {}", idx + 1, cfg.code, cfg.name),
            title_x + 22.0,
            cy + 8.0,
            2.0,
            if selected { WHITE } else { MUTED },
        );
        hud.text(cfg.subtitle, title_x + 24.0, cy + 26.0, 1.4, if selected { CYAN } else { MUTED });
    }

    // Controls Card on Right
    let cx = w - 426.0;
    let cy = h * 0.44;
    hud.rect(cx, cy, 384.0, 224.0, PANEL_BG);
    hud.rect_outline(cx, cy, 384.0, 224.0, 1.5, PANEL_EDGE);
    hud.text_shadow("FLIGHT & COMBAT CONTROLS", cx + 16.0, cy + 14.0, 2.0, CYAN);
    let key_lines = [
        "WASD        MOVE / AIR STRAFE",
        "SPACE       JUMP / VERTICAL JETS",
        "SHIFT       AFTERBURNER FLIGHT BOOST",
        "C / CTRL    AERIAL SEISMIC DIVE-SLAM",
        "LMB / RMB   FIRE / HOVER-ZOOM ADS",
        "1-4 / WHEEL SWITCH WEAPONS (3=LOCK)",
        "F1 F2 F3    SQUAD FOLLOW/ATTACK/HOLD",
        "V / N       1ST-3RD VIEW / NEXT WORLD",
    ];
    let touch_lines = [
        "LEFT STICK  MOVE / AIR STRAFE",
        "RIGHT DRAG  AIM / FLIGHT VECTOR",
        "JETS        JUMP / VERTICAL JETS",
        "BOOST       AFTERBURNER FLIGHT BOOST",
        "SLAM        AERIAL SEISMIC DIVE-SLAM",
        "FIRE        HOLD SWARM POD TO LOCK",
        "WPN / SQD   SWITCH WEAPON / SQUAD ORDER",
        "CAM / II    1ST-3RD VIEW / PAUSE",
    ];
    let lines = if touch_ui { touch_lines } else { key_lines };
    for (i, line) in lines.iter().enumerate() {
        hud.text(line, cx + 16.0, cy + 44.0 + i as f32 * 21.0, 1.55, WHITE);
    }

    // Pulsing Start Prompt
    let pulse = 0.65 + 0.35 * (game.time * 4.0).sin();
    let prompt_col = Vec4::new(0.22, 0.96, 1.0, pulse);
    hud.text_centered_shadow(
        if touch_ui {
            "[ TAP A SECTOR, THEN TAP TO INITIATE ORBITAL DROP ]"
        } else {
            "[ PRESS ENTER OR CLICK TO INITIATE ORBITAL DROP ]"
        },
        w * 0.5,
        h - 58.0,
        2.6,
        prompt_col,
    );
}

fn build_combat_hud(game: &GameState, cam: &Camera, w: f32, h: f32, touch_ui: bool, hud: &mut HudDrawList) {
    let cx = w * 0.5;
    let cy = h * 0.5;

    // --- 1. 3D WORLD-SPACE PROJECTIONS (SPIRES, ENEMIES, LOCK-ONS, WINGMEN) ---
    for spire in &game.spires {
        if !spire.alive { continue; }
        let dist = spire.pos.distance(game.player.position) as i32;
        if let Some((sp, on_screen)) = cam.project_to_screen(spire.core_pos() + Vec3::Y * 5.0, w, h) {
            if on_screen {
                let locked = game.lock_targets.contains(&LockTarget::Spire(spire.id));
                let col = if locked { AMBER } else { Vec4::new(1.0, 0.32, 0.52, 0.92) };
                hud.diamond(Vec2::new(sp.x, sp.y), if locked { 20.0 } else { 11.0 }, 2.0, col);
                hud.text_centered_shadow(
                    &format!("{} {}M", spire.label, dist),
                    sp.x,
                    sp.y - 26.0,
                    1.45,
                    col,
                );
                // Health bar
                let frac = (spire.hp / spire.max_hp).clamp(0.0, 1.0);
                hud.rect(sp.x - 32.0, sp.y + 16.0, 64.0, 5.0, Vec4::new(0.05, 0.05, 0.08, 0.75));
                hud.rect(sp.x - 32.0, sp.y + 16.0, 64.0 * frac, 5.0, col);
            }
        }
    }

    for wing in &game.wingmen {
        if let Some((sp, on_screen)) = cam.project_to_screen(wing.pos + Vec3::Y * 2.5, w, h) {
            if on_screen {
                hud.diamond(Vec2::new(sp.x, sp.y), 6.0, 1.5, CYAN);
                hud.text_centered_shadow(wing.callsign, sp.x, sp.y - 18.0, 1.35, CYAN);
            }
        }
    }

    for e in &game.enemies {
        if !e.alive { continue; }
        let locked = game.lock_targets.contains(&LockTarget::Enemy(e.id));
        if let Some((sp, on_screen)) = cam.project_to_screen(e.center(), w, h) {
            if on_screen {
                if locked {
                    hud.diamond(Vec2::new(sp.x, sp.y), 22.0, 2.2, AMBER);
                    hud.corner_brackets(Vec2::new(sp.x, sp.y), Vec2::splat(26.0), 8.0, 2.0, RED);
                    hud.text_centered_shadow("LOCKED", sp.x, sp.y - 36.0, 1.4, AMBER);
                }
                let near_crosshair = Vec2::new(sp.x - cx, sp.y - cy).length() < 110.0;
                if e.hp < e.max_hp || near_crosshair || e.kind == EnemyKind::Goliath {
                    let bw = if e.kind == EnemyKind::Goliath { 84.0 } else { 44.0 };
                    let frac = (e.hp / e.max_hp).clamp(0.0, 1.0);
                    let hy = sp.y - 24.0;
                    hud.rect(sp.x - bw * 0.5, hy, bw, 4.5, Vec4::new(0.05, 0.05, 0.08, 0.80));
                    hud.rect(sp.x - bw * 0.5, hy, bw * frac, 4.5, RED);
                }
            }
        }
    }

    // --- 2. DYNAMIC WEAPON RETICLE & HITMARKER ---
    let w_col = game.weapon.accent_color().extend(0.92);
    let spread = 10.0 + game.weapon_recoil * 26.0 + game.player.afterburner_intensity * 8.0;
    match game.weapon {
        WeaponKind::PulseCarbine => {
            hud.rect(cx - 1.5, cy - 1.5, 3.0, 3.0, WHITE);
            hud.rect(cx - spread - 10.0, cy - 1.0, 10.0, 2.0, w_col);
            hud.rect(cx + spread, cy - 1.0, 10.0, 2.0, w_col);
            hud.rect(cx - 1.0, cy - spread - 10.0, 2.0, 10.0, w_col);
            hud.rect(cx - 1.0, cy + spread, 2.0, 10.0, w_col);
        }
        WeaponKind::RailLance => {
            hud.circle_outline(Vec2::new(cx, cy), 18.0 + game.weapon_recoil * 16.0, 1.8, 24, w_col);
            hud.rect(cx - 2.0, cy - 2.0, 4.0, 4.0, WHITE);
            hud.line(Vec2::new(cx - 32.0, cy), Vec2::new(cx - 20.0, cy), 1.8, w_col);
            hud.line(Vec2::new(cx + 20.0, cy), Vec2::new(cx + 32.0, cy), 1.8, w_col);
        }
        WeaponKind::MicroMissiles => {
            hud.circle_outline(Vec2::new(cx, cy), 34.0, 1.6, 28, Vec4::new(1.0, 0.45, 0.16, 0.55));
            hud.diamond(Vec2::new(cx, cy), 8.0, 1.8, w_col);
            let lock_txt = format!("LOCKS: {}/4", game.lock_targets.len());
            hud.text_centered_shadow(&lock_txt, cx, cy + 44.0, 1.6, if game.lock_targets.is_empty() { MUTED } else { AMBER });
        }
        WeaponKind::ArcMortar => {
            hud.circle_outline(Vec2::new(cx, cy), 14.0, 2.0, 20, w_col);
            hud.line(Vec2::new(cx - 16.0, cy + 20.0), Vec2::new(cx + 16.0, cy + 20.0), 1.8, w_col);
            hud.line(Vec2::new(cx - 10.0, cy + 29.0), Vec2::new(cx + 10.0, cy + 29.0), 1.8, w_col);
        }
    }

    if game.hitmarker_timer > 0.0 {
        let hm_col = if game.hitmarker_kill {
            RED
        } else if game.hitmarker_crit {
            AMBER
        } else {
            WHITE
        };
        let d0 = 7.0;
        let d1 = 15.0;
        hud.line(Vec2::new(cx - d1, cy - d1), Vec2::new(cx - d0, cy - d0), 2.4, hm_col);
        hud.line(Vec2::new(cx + d1, cy - d1), Vec2::new(cx + d0, cy - d0), 2.4, hm_col);
        hud.line(Vec2::new(cx - d1, cy + d1), Vec2::new(cx - d0, cy + d0), 2.4, hm_col);
        hud.line(Vec2::new(cx + d1, cy + d1), Vec2::new(cx + d0, cy + d0), 2.4, hm_col);
    }

    // --- 3. TOP-LEFT MISSION OBJECTIVES & SCORE ---
    hud.rect(20.0, 18.0, 380.0, 96.0, PANEL_BG);
    hud.rect(20.0, 18.0, 4.0, 96.0, CYAN);
    hud.text_shadow(&format!("{} // {}", game.biome_cfg.code, game.biome_cfg.name), 34.0, 26.0, 1.7, CYAN);
    let alive_spires = game.spires.iter().filter(|s| s.alive).count();
    let obj_line = if alive_spires > 0 {
        format!("DESTROY ALIEN SPIRES: {}/3", 3 - alive_spires)
    } else {
        "ELIMINATE DREADNOUGHT GOLIATH!".to_string()
    };
    hud.text_shadow(&obj_line, 34.0, 50.0, 1.7, WHITE);
    let score_line = if game.combo > 1 {
        format!("SCORE {:06}   KILLS {}   {}X COMBO!", game.score, game.kills, game.combo)
    } else {
        format!("SCORE {:06}   KILLS {}", game.score, game.kills)
    };
    hud.text_shadow(&score_line, 34.0, 78.0, 1.65, AMBER);

    // Boss Health Bar Top-Center when Dreadnought Goliath is active!
    if let Some(boss) = game.enemies.iter().find(|e| e.alive && e.kind == EnemyKind::Goliath) {
        let bw = 420.0;
        let bx = cx - bw * 0.5;
        hud.rect(bx - 10.0, 18.0, bw + 20.0, 42.0, PANEL_BG);
        hud.text_centered_shadow("DREADNOUGHT GOLIATH", cx, 23.0, 1.75, RED);
        let frac = (boss.hp / boss.max_hp).clamp(0.0, 1.0);
        hud.rect(bx, 42.0, bw, 10.0, Vec4::new(0.15, 0.04, 0.05, 0.9));
        hud.rect(bx, 42.0, bw * frac, 10.0, RED);
    }

    // --- 4. TOP-RIGHT 360° TACTICAL RADAR ---
    let rad_r = 64.0;
    let rad_c = Vec2::new(w - rad_r - 24.0, rad_r + 22.0);
    hud.circle_filled(rad_c, rad_r, 32, Vec4::new(0.02, 0.05, 0.09, 0.72));
    hud.circle_outline(rad_c, rad_r, 2.0, 32, PANEL_EDGE);
    hud.circle_outline(rad_c, rad_r * 0.5, 1.0, 24, Vec4::new(0.18, 0.72, 0.90, 0.25));
    hud.line(rad_c - Vec2::new(rad_r, 0.0), rad_c + Vec2::new(rad_r, 0.0), 1.0, Vec4::new(0.18, 0.72, 0.9, 0.2));
    hud.line(rad_c - Vec2::new(0.0, rad_r), rad_c + Vec2::new(0.0, rad_r), 1.0, Vec4::new(0.18, 0.72, 0.9, 0.2));

    let fwd = game.player.forward_xz();
    let right = game.player.right_xz();
    let radar_range = 165.0;
    let map_blip = |world_p: Vec3| -> Vec2 {
        let rel = world_p - game.player.position;
        let rx = rel.dot(right) / radar_range;
        let ry = -rel.dot(fwd) / radar_range;
        let v = Vec2::new(rx, ry);
        let clamped = if v.length() > 0.92 { v.normalize() * 0.92 } else { v };
        rad_c + clamped * rad_r
    };

    for s in &game.spires {
        if !s.alive { continue; }
        hud.diamond(map_blip(s.pos), 5.5, 1.8, Vec4::new(1.0, 0.28, 0.58, 1.0));
    }
    for e in &game.enemies {
        if !e.alive { continue; }
        let (r, col) = match e.kind {
            EnemyKind::Goliath => (5.2, AMBER),
            EnemyKind::Interceptor => (3.4, Vec4::new(1.0, 0.55, 0.15, 1.0)),
            _ => (2.8, RED),
        };
        hud.circle_filled(map_blip(e.pos), r, 8, col);
    }
    for wing in &game.wingmen {
        hud.circle_filled(map_blip(wing.pos), 3.4, 8, CYAN);
    }
    // Player center chevron
    hud.tri(
        rad_c + Vec2::new(0.0, -5.5),
        rad_c + Vec2::new(4.2, 4.5),
        rad_c + Vec2::new(-4.2, 4.5),
        WHITE,
    );

    // --- 5. BOTTOM-LEFT EXO-SUIT VITALS & FLIGHT TELEMETRY ---
    let bx = 24.0;
    let by = h - 122.0;
    hud.slanted_rect(bx, by, 330.0, 102.0, 12.0, PANEL_BG);
    hud.slanted_rect(bx, by, 5.0, 102.0, 12.0, CYAN);

    // Shield bar
    hud.text_shadow(&format!("SHIELD {:3}", game.shield as i32), bx + 20.0, by + 10.0, 1.65, CYAN);
    hud.slanted_rect(bx + 126.0, by + 11.0, 185.0, 13.0, 6.0, Vec4::new(0.05, 0.12, 0.18, 0.85));
    hud.slanted_rect(bx + 126.0, by + 11.0, 185.0 * (game.shield / game.max_shield), 13.0, 6.0, CYAN);

    // Hull bar
    let hull_col = if game.hull < 35.0 { RED } else { GREEN };
    hud.text_shadow(&format!("HULL   {:3}", game.hull as i32), bx + 20.0, by + 34.0, 1.65, hull_col);
    hud.slanted_rect(bx + 126.0, by + 35.0, 185.0, 13.0, 6.0, Vec4::new(0.08, 0.12, 0.08, 0.85));
    hud.slanted_rect(bx + 126.0, by + 35.0, 185.0 * (game.hull / game.max_hull), 13.0, 6.0, hull_col);

    // Thruster Capacitor bar
    let jet_col = if game.player.overheated { RED } else { AMBER };
    let jet_lbl = if game.player.overheated { "OVERHEAT!" } else { "THRUST" };
    hud.text_shadow(jet_lbl, bx + 20.0, by + 58.0, 1.65, jet_col);
    hud.slanted_rect(bx + 126.0, by + 59.0, 185.0, 13.0, 6.0, Vec4::new(0.16, 0.10, 0.04, 0.85));
    hud.slanted_rect(bx + 126.0, by + 59.0, 185.0 * (game.player.energy / 100.0), 13.0, 6.0, jet_col);

    // Altitude & Speed telemetry
    let state_tag = if game.player.dive_slamming {
        "DIVE-SLAM"
    } else if game.player.boosting {
        "AFTERBURNER"
    } else if game.player.hovering {
        "HOVER-ADS"
    } else if !game.player.grounded {
        "AIRBORNE"
    } else {
        "GROUNDED"
    };
    hud.text(
        &format!("ALT {:02.0}M  SPD {:02.0}M/S  {}", game.player.altitude_agl, game.player.speed(), state_tag),
        bx + 18.0,
        by + 82.0,
        1.45,
        MUTED,
    );

    // --- 6. BOTTOM-RIGHT ARSENAL & SQUAD ORDER (shifts center-bottom when mobile touch buttons are visible) ---
    let wx = if touch_ui { w * 0.5 - 163.0 } else { w - 350.0 };
    let wy = h - 118.0;
    hud.slanted_rect(wx, wy, 326.0, 102.0, -10.0, PANEL_BG);
    hud.text_shadow(game.weapon.name(), wx + 18.0, wy + 12.0, 1.9, w_col);

    // 4 weapon quick-slots
    for (i, wk) in WeaponKind::ALL.iter().enumerate() {
        let sx = wx + 16.0 + i as f32 * 75.0;
        let active = *wk == game.weapon;
        let bg = if active {
            wk.accent_color().extend(0.28)
        } else {
            Vec4::new(0.06, 0.08, 0.12, 0.72)
        };
        hud.rect(sx, wy + 38.0, 69.0, 26.0, bg);
        if active {
            hud.rect_outline(sx, wy + 38.0, 69.0, 26.0, 1.5, wk.accent_color().extend(0.95));
        }
        hud.text(
            &format!("{}:{}", i + 1, wk.short_name()),
            sx + 5.0,
            wy + 46.0,
            1.35,
            if active { WHITE } else { MUTED },
        );
    }

    // Squad status line
    let order_col = match game.squad_order {
        SquadOrder::Follow => CYAN,
        SquadOrder::AttackTarget => AMBER,
        SquadOrder::HoldPosition => GREEN,
    };
    hud.text_shadow(
        &format!("SQUAD [F1-F3]: {}", game.squad_order.label()),
        wx + 18.0,
        wy + 76.0,
        1.55,
        order_col,
    );

    // --- 7. TACTICAL COMMS FEED ---
    let comms_y = 132.0;
    for (i, msg) in game.comms.iter().enumerate() {
        let fade = (msg.ttl / 1.2).clamp(0.0, 1.0);
        let col = Vec4::new(msg.color.x, msg.color.y, msg.color.z, msg.color.w * fade);
        hud.text_shadow(&msg.text, 24.0, comms_y + i as f32 * 22.0, 1.55, col);
    }
}

fn build_overlay_box(w: f32, h: f32, title: &str, lines: &[&str], accent: Vec4, hud: &mut HudDrawList) {
    hud.rect(0.0, 0.0, w, h, Vec4::new(0.01, 0.02, 0.05, 0.56));
    let bw = 560.0;
    let bh = 180.0;
    let bx = (w - bw) * 0.5;
    let by = (h - bh) * 0.5;
    hud.rect(bx, by, bw, bh, Vec4::new(0.03, 0.05, 0.10, 0.90));
    hud.rect_outline(bx, by, bw, bh, 2.0, accent);
    hud.text_centered_shadow(title, w * 0.5, by + 26.0, 3.2, accent);
    for (i, line) in lines.iter().enumerate() {
        hud.text_centered_shadow(line, w * 0.5, by + 84.0 + i as f32 * 30.0, 1.85, WHITE);
    }
}
