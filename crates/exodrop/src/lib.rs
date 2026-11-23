pub mod entities;
pub mod game;
pub mod hud_ui;
pub mod touch;

use crate::game::{GameMode, GameState};
use crate::touch::TouchController;
use exo_assets::{build_terrain_mesh, MeshId};
use exo_audio::AudioEngine;
use exo_core::{BiomeId, InputState, Key, SimClock, FIXED_DT};
use exo_gfx::{FxVertex, HudDrawList, InstanceData, PointLight, Renderer, SceneFrame, SurfaceError};
use glam::Vec2;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{DeviceEvent, DeviceId, ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, NamedKey, PhysicalKey};
use winit::window::{CursorGrabMode, Fullscreen, Window, WindowId};

pub struct CliArgs {
    pub sector: usize,
    pub screenshot: Option<PathBuf>,
    pub cam_preset: String,
    pub width: u32,
    pub height: u32,
    pub touch_ui: bool,
    pub no_hud: bool,
}

impl Default for CliArgs {
    fn default() -> Self {
        Self {
            sector: 0,
            screenshot: None,
            cam_preset: "action".to_string(),
            width: 1280,
            height: 720,
            touch_ui: cfg!(any(target_os = "android", target_os = "ios")),
            no_hud: false,
        }
    }
}

pub fn parse_args() -> CliArgs {
    let mut args = std::env::args().skip(1);
    let mut cli = CliArgs::default();
    while let Some(a) = args.next() {
        match a.as_str() {
            "--sector" => {
                if let Some(v) = args.next() {
                    cli.sector = v.parse().unwrap_or(0);
                }
            }
            "--screenshot" => {
                if let Some(v) = args.next() {
                    cli.screenshot = Some(PathBuf::from(v));
                }
            }
            "--cam" => {
                if let Some(v) = args.next() {
                    cli.cam_preset = v;
                }
            }
            "--size" => {
                if let Some(v) = args.next() {
                    if let Some((w, h)) = v.split_once('x') {
                        cli.width = w.parse().unwrap_or(1280);
                        cli.height = h.parse().unwrap_or(720);
                    }
                }
            }
            "--touch-ui" => {
                cli.touch_ui = true;
            }
            "--no-hud" => {
                cli.no_hud = true;
            }
            _ => {}
        }
    }
    cli
}

/// Desktop entry point (macOS / Windows / Linux).
pub fn run_desktop() {
    let cli = parse_args();
    let biome = BiomeId::from_index(cli.sector);

    if let Some(path) = cli.screenshot {
        let mut renderer = Renderer::new_headless(cli.width, cli.height);
        let mut game = GameState::new(biome);
        game.setup_showcase(&cli.cam_preset);
        let terrain_mesh = build_terrain_mesh(&game.terrain);
        renderer.set_terrain_mesh(&terrain_mesh);

        let mut instances: Vec<Vec<InstanceData>> = vec![Vec::new(); MeshId::COUNT];
        let mut lights: Vec<PointLight> = Vec::new();
        let mut fx_verts: Vec<FxVertex> = Vec::new();
        let mut hud = HudDrawList::new();
        let touch = TouchController::new(cli.touch_ui);

        let cam = game.current_camera();
        game.build_render_batches(&cam, &mut instances, &mut lights, &mut fx_verts);
        // --touch-ui captures use the phone HUD layout (virtual 720 px high), like the mobile builds.
        let ui = if cli.touch_ui { (cli.width.min(cli.height) as f32 / 720.0).clamp(1.0, 2.0) } else { 1.0 };
        renderer.set_hud_scale(ui);
        let (uw, uh) = (cli.width as f32 / ui, cli.height as f32 / ui);
        if !cli.no_hud {
            hud_ui::build_hud(&game, &cam, uw, uh, cli.touch_ui, &mut hud);
            touch.draw_overlay(game.mode, uw, uh, &mut hud);
        }

        let frame = SceneFrame {
            camera: cam,
            shadow_focus: game.player.position,
            biome: &game.biome_cfg,
            time: game.time,
            boost_fx: game.boost_fx,
            damage_fx: game.damage_fx,
            instances: &instances,
            lights: &lights,
            fx_vertices: &fx_verts,
            hud: &hud,
        };
        if let Err(e) = renderer.render_to_png(&frame, &path) {
            eprintln!("Failed to render screenshot: {e}");
            std::process::exit(1);
        }
        println!("Rendered {} ({}x{})", path.display(), cli.width, cli.height);
        return;
    }

    let event_loop = EventLoop::new().expect("create event loop");
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = ExoDropApp::new(biome, cli.touch_ui);
    event_loop.run_app(&mut app).expect("run app");
}

/// Android NativeActivity entry point (compiled into `libexodrop.so`).
#[cfg(target_os = "android")]
#[no_mangle]
pub fn android_main(android_app: android_activity::AndroidApp) {
    use winit::platform::android::EventLoopBuilderExtAndroid;
    android_logger::init_once(
        android_logger::Config::default()
            .with_max_level(log::LevelFilter::Info)
            .with_tag("exodrop"),
    );
    // Android discards stderr, so a panic (including wgpu validation errors, which panic by default) would only
    // show up as a bare SIGABRT. Route the message and location to logcat.
    std::panic::set_hook(Box::new(|info| {
        log::error!("PANIC: {info}\n{}", std::backtrace::Backtrace::force_capture());
    }));
    let event_loop = EventLoop::builder()
        .with_android_app(android_app)
        .build()
        .expect("create Android event loop");
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = ExoDropApp::new(BiomeId::CrimsonMesa, true);
    event_loop.run_app(&mut app).expect("run Android app");
    // winit allows one event loop per process. If the activity is finished (Back on the title screen) and the
    // launcher starts it again while the process is cached, a second android_main would panic, so end the process.
    std::process::exit(0);
}

/// iOS staticlib entry point (called from `mobile/ios/ExoDropiOS/main.m`).
#[cfg(target_os = "ios")]
#[no_mangle]
pub extern "C" fn exodrop_ios_main() {
    let event_loop = EventLoop::new().expect("create iOS event loop");
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = ExoDropApp::new(BiomeId::CrimsonMesa, true);
    event_loop.run_app(&mut app).expect("run iOS app");
}

struct ExoDropApp {
    window: Option<Arc<Window>>,
    renderer: Option<Renderer>,
    audio: AudioEngine,
    game: GameState,
    touch: TouchController,
    clock: SimClock,
    input: InputState,
    last_frame: Instant,
    cursor_locked: bool,
    instances: Vec<Vec<InstanceData>>,
    lights: Vec<PointLight>,
    fx_verts: Vec<FxVertex>,
    hud: HudDrawList,
    fps_window_start: Instant,
    fps_frames: u32,
    /// Seconds in a row below the mobile FPS floor; triggers a render-scale step down.
    slow_secs: f32,
}

impl ExoDropApp {
    fn new(biome: BiomeId, touch_ui: bool) -> Self {
        Self {
            window: None,
            renderer: None,
            audio: AudioEngine::new(),
            game: GameState::new(biome),
            touch: TouchController::new(touch_ui),
            clock: SimClock::default(),
            input: InputState::default(),
            last_frame: Instant::now(),
            cursor_locked: false,
            instances: vec![Vec::new(); MeshId::COUNT],
            lights: Vec::with_capacity(16),
            fx_verts: Vec::with_capacity(4096),
            hud: HudDrawList::new(),
            fps_window_start: Instant::now(),
            fps_frames: 0,
            slow_secs: 0.0,
        }
    }

    /// Touch builds lay the HUD and touch controls out in a virtual space 720 px high (up to 2x), so text and
    /// buttons keep their physical size on 1080p+ phones. Desktop draws 1:1.
    fn ui_scale(&self) -> f32 {
        match &self.renderer {
            Some(r) if self.touch.enabled => (r.height.min(r.width) as f32 / 720.0).clamp(1.0, 2.0),
            _ => 1.0,
        }
    }

    fn ui_size(&self) -> (f32, f32) {
        let ui = self.ui_scale();
        self.renderer
            .as_ref()
            .map(|r| (r.width as f32 / ui, r.height as f32 / ui))
            .unwrap_or((1280.0, 720.0))
    }

    fn set_cursor_lock(&mut self, lock: bool) {
        if cfg!(any(target_os = "android", target_os = "ios")) || self.cursor_locked == lock {
            return;
        }
        if let Some(w) = &self.window {
            if lock {
                let _ = w
                    .set_cursor_grab(CursorGrabMode::Locked)
                    .or_else(|_| w.set_cursor_grab(CursorGrabMode::Confined));
                w.set_cursor_visible(false);
            } else {
                let _ = w.set_cursor_grab(CursorGrabMode::None);
                w.set_cursor_visible(true);
            }
            self.cursor_locked = lock;
        }
    }
}

impl ApplicationHandler for ExoDropApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        event_loop.set_control_flow(ControlFlow::Poll);
        self.audio.resume();
        let window = match &self.window {
            Some(w) => Arc::clone(w),
            None => {
                let attrs = Window::default_attributes()
                    .with_title("EXODROP — Orbital Exo-Suit Tactical Action")
                    .with_inner_size(LogicalSize::new(1280.0, 720.0));
                let w = Arc::new(event_loop.create_window(attrs).expect("create window"));
                self.window = Some(Arc::clone(&w));
                w
            }
        };

        let phys = window.inner_size();
        if let Some(renderer) = &mut self.renderer {
            // Mobile foreground resume: recreate swapchain surface without reloading GPU meshes!
            renderer.resume_surface(window, phys.width, phys.height);
        } else {
            let mut renderer = Renderer::new_windowed(window, phys.width, phys.height);
            let terrain_mesh = build_terrain_mesh(&self.game.terrain);
            renderer.set_terrain_mesh(&terrain_mesh);
            self.game.terrain_dirty = false;
            self.renderer = Some(renderer);
        }
        self.last_frame = Instant::now();
    }

    fn suspended(&mut self, event_loop: &ActiveEventLoop) {
        // Poll with no surface would spin a core at 100% while the app sits in the background.
        event_loop.set_control_flow(ControlFlow::Wait);
        // Mobile background suspend: release swapchain surface and pause audio cleanly!
        if let Some(renderer) = &mut self.renderer {
            renderer.suspend_surface();
        }
        self.audio.pause();
        self.touch.reset(&mut self.input);
        if self.game.mode == GameMode::Playing {
            self.game.mode = GameMode::Paused;
        }
    }

    fn device_event(&mut self, _event_loop: &ActiveEventLoop, _id: DeviceId, event: DeviceEvent) {
        if let DeviceEvent::MouseMotion { delta: (dx, dy) } = event {
            if self.game.mode == GameMode::Playing {
                self.input.mouse_delta += Vec2::new(dx as f32, dy as f32);
            }
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                if let Some(r) = &mut self.renderer {
                    r.resize(size.width, size.height);
                }
            }
            WindowEvent::Focused(false) => {
                self.set_cursor_lock(false);
                self.touch.reset(&mut self.input);
                if self.game.mode == GameMode::Playing {
                    self.game.mode = GameMode::Paused;
                }
            }
            WindowEvent::Touch(mut touch) => {
                let (w, h) = self.ui_size();
                let ui = self.ui_scale();
                touch.location.x /= ui as f64;
                touch.location.y /= ui as f64;
                self.touch.on_touch(touch, self.game.mode, w, h, &mut self.input);
            }
            WindowEvent::KeyboardInput { event, .. } => {
                // Android Back arrives as a logical key with no physical key code: pause in game, quit on title.
                if matches!(
                    event.logical_key,
                    winit::keyboard::Key::Named(NamedKey::BrowserBack | NamedKey::GoBack)
                ) {
                    if event.state == ElementState::Pressed && !event.repeat {
                        if self.game.mode == GameMode::Title {
                            event_loop.exit();
                        } else {
                            self.input.on_key(Key::Escape, true);
                        }
                    } else if event.state == ElementState::Released {
                        self.input.on_key(Key::Escape, false);
                    }
                    return;
                }
                if let PhysicalKey::Code(code) = event.physical_key {
                    let down = event.state == ElementState::Pressed;
                    if let Some(k) = map_key(code) {
                        if k == Key::F11 && down && !event.repeat {
                            if let Some(w) = &self.window {
                                if w.fullscreen().is_some() {
                                    w.set_fullscreen(None);
                                } else {
                                    w.set_fullscreen(Some(Fullscreen::Borderless(None)));
                                }
                            }
                        }
                        self.input.on_key(k, down);
                    }
                }
            }
            WindowEvent::MouseInput { state, button, .. } => {
                let down = state == ElementState::Pressed;
                match button {
                    MouseButton::Left => {
                        self.input.on_mouse_button(true, down);
                        if down && self.game.mode == GameMode::Playing {
                            self.set_cursor_lock(true);
                        }
                    }
                    MouseButton::Right => self.input.on_mouse_button(false, down),
                    _ => {}
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let dy = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y,
                    MouseScrollDelta::PixelDelta(p) => p.y as f32 * 0.05,
                };
                self.input.scroll_delta += dy;
            }
            WindowEvent::RedrawRequested => {
                let now = Instant::now();
                let frame_dt = (now - self.last_frame).as_secs_f32().clamp(0.0, 0.05);
                self.last_frame = now;
                self.fps_frames += 1;
                let window_secs = (now - self.fps_window_start).as_secs_f32();
                if window_secs >= 5.0 {
                    // One line every 5 s (logcat on Android, stderr-less no-op elsewhere without a logger).
                    let fps = self.fps_frames as f32 / window_secs;
                    let scale = self.renderer.as_ref().map(|r| r.render_scale()).unwrap_or(1.0);
                    log::info!("FPS {fps:.1} mode {:?} render_scale {scale:.2}", self.game.mode);
                    // Touch builds: step the 3D resolution down after ~10 s of play under 28 FPS (never back up,
                    // so it can't oscillate). Floor 0.4 of the mobile starting scale's window size.
                    if self.touch.enabled && self.game.mode == GameMode::Playing {
                        if fps < 28.0 {
                            self.slow_secs += window_secs;
                        } else {
                            self.slow_secs = 0.0;
                        }
                        if self.slow_secs >= 10.0 && scale > 0.4 {
                            if let Some(r) = &mut self.renderer {
                                r.set_render_scale((scale * 0.8).max(0.4));
                                log::info!("QUALITY render_scale {:.2} -> {:.2}", scale, r.render_scale());
                            }
                            self.slow_secs = 0.0;
                        }
                    }
                    self.fps_frames = 0;
                    self.fps_window_start = now;
                }

                let (_, ui_h) = self.ui_size();
                self.touch.apply_to_input(ui_h, &mut self.input);

                self.game.handle_frame_input(&self.input, &self.audio);
                let should_lock = self.game.mode == GameMode::Playing && !self.touch.enabled;
                self.set_cursor_lock(should_lock);

                let steps = self.clock.advance(frame_dt);
                for _ in 0..steps {
                    self.game.fixed_update(FIXED_DT, &self.input, &self.audio);
                }
                self.input.end_frame();

                let ui = self.ui_scale();
                let (rw, rh) = self.ui_size();
                if let Some(renderer) = &mut self.renderer {
                    renderer.set_hud_scale(ui);
                    if self.game.terrain_dirty {
                        let mesh = build_terrain_mesh(&self.game.terrain);
                        renderer.set_terrain_mesh(&mesh);
                        self.game.terrain_dirty = false;
                    }
                    let cam = self.game.current_camera();
                    self.game.build_render_batches(&cam, &mut self.instances, &mut self.lights, &mut self.fx_verts);
                    hud_ui::build_hud(&self.game, &cam, rw, rh, self.touch.enabled, &mut self.hud);
                    self.touch.draw_overlay(self.game.mode, rw, rh, &mut self.hud);
                    let scene = SceneFrame {
                        camera: cam,
                        shadow_focus: self.game.player.position,
                        biome: &self.game.biome_cfg,
                        time: self.game.time,
                        boost_fx: self.game.boost_fx,
                        damage_fx: self.game.damage_fx,
                        instances: &self.instances,
                        lights: &self.lights,
                        fx_vertices: &self.fx_verts,
                        hud: &self.hud,
                    };
                    match renderer.render_to_surface(&scene) {
                        Ok(()) => {}
                        Err(SurfaceError::Lost | SurfaceError::Outdated) => {
                            renderer.resize(renderer.width, renderer.height)
                        }
                        Err(SurfaceError::OutOfMemory) => event_loop.exit(),
                        Err(_) => {}
                    }
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }
}

fn map_key(code: KeyCode) -> Option<Key> {
    Some(match code {
        KeyCode::KeyW | KeyCode::ArrowUp => Key::W,
        KeyCode::KeyA | KeyCode::ArrowLeft => Key::A,
        KeyCode::KeyS | KeyCode::ArrowDown => Key::S,
        KeyCode::KeyD | KeyCode::ArrowRight => Key::D,
        KeyCode::Space => Key::Space,
        KeyCode::ShiftLeft | KeyCode::ShiftRight => Key::Shift,
        KeyCode::ControlLeft | KeyCode::ControlRight => Key::Ctrl,
        KeyCode::KeyC => Key::C,
        KeyCode::KeyE => Key::E,
        KeyCode::KeyR => Key::R,
        KeyCode::KeyV => Key::V,
        KeyCode::KeyQ => Key::Q,
        KeyCode::KeyF => Key::F,
        KeyCode::KeyN => Key::N,
        KeyCode::Tab => Key::Tab,
        KeyCode::Enter => Key::Enter,
        KeyCode::Escape => Key::Escape,
        KeyCode::Digit1 => Key::Digit1,
        KeyCode::Digit2 => Key::Digit2,
        KeyCode::Digit3 => Key::Digit3,
        KeyCode::Digit4 => Key::Digit4,
        KeyCode::F1 => Key::F1,
        KeyCode::F2 => Key::F2,
        KeyCode::F3 => Key::F3,
        KeyCode::F11 => Key::F11,
        _ => return None,
    })
}
