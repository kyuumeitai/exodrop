mod entities;
mod game;
mod hud_ui;

use crate::game::{GameMode, GameState};
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
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{CursorGrabMode, Fullscreen, Window, WindowId};

struct CliArgs {
    sector: usize,
    screenshot: Option<PathBuf>,
    cam_preset: String,
    width: u32,
    height: u32,
}

fn parse_args() -> CliArgs {
    let mut args = std::env::args().skip(1);
    let mut cli = CliArgs {
        sector: 0,
        screenshot: None,
        cam_preset: "action".to_string(),
        width: 1280,
        height: 720,
    };
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
            _ => {}
        }
    }
    cli
}

fn main() {
    let cli = parse_args();
    let biome = BiomeId::from_index(cli.sector);

    // Headless screenshot render mode for visual validation & gallery capture!
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

        let cam = game.current_camera();
        game.build_render_batches(&cam, &mut instances, &mut lights, &mut fx_verts);
        hud_ui::build_hud(&game, &cam, cli.width as f32, cli.height as f32, &mut hud);

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
    let mut app = ExoDropApp::new(biome);
    event_loop.run_app(&mut app).expect("run app");
}

struct ExoDropApp {
    window: Option<Arc<Window>>,
    renderer: Option<Renderer>,
    audio: AudioEngine,
    game: GameState,
    clock: SimClock,
    input: InputState,
    last_frame: Instant,
    cursor_locked: bool,
    instances: Vec<Vec<InstanceData>>,
    lights: Vec<PointLight>,
    fx_verts: Vec<FxVertex>,
    hud: HudDrawList,
}

impl ExoDropApp {
    fn new(biome: BiomeId) -> Self {
        Self {
            window: None,
            renderer: None,
            audio: AudioEngine::new(),
            game: GameState::new(biome),
            clock: SimClock::default(),
            input: InputState::default(),
            last_frame: Instant::now(),
            cursor_locked: false,
            instances: vec![Vec::new(); MeshId::COUNT],
            lights: Vec::with_capacity(16),
            fx_verts: Vec::with_capacity(4096),
            hud: HudDrawList::new(),
        }
    }

    fn set_cursor_lock(&mut self, lock: bool) {
        if self.cursor_locked == lock {
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
        if self.window.is_some() {
            return;
        }
        let attrs = Window::default_attributes()
            .with_title("EXODROP — Orbital Exo-Suit Tactical Action")
            .with_inner_size(LogicalSize::new(1280.0, 720.0));
        let window = Arc::new(event_loop.create_window(attrs).expect("create window"));
        let phys = window.inner_size();
        let mut renderer = Renderer::new_windowed(Arc::clone(&window), phys.width, phys.height);
        let terrain_mesh = build_terrain_mesh(&self.game.terrain);
        renderer.set_terrain_mesh(&terrain_mesh);
        self.game.terrain_dirty = false;

        self.window = Some(window);
        self.renderer = Some(renderer);
        self.last_frame = Instant::now();
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
                if self.game.mode == GameMode::Playing {
                    self.game.mode = GameMode::Paused;
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
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

                self.game.handle_frame_input(&self.input, &self.audio);
                let should_lock = self.game.mode == GameMode::Playing;
                self.set_cursor_lock(should_lock);

                let steps = self.clock.advance(frame_dt);
                for _ in 0..steps {
                    self.game.fixed_update(FIXED_DT, &self.input, &self.audio);
                }
                self.input.end_frame();

                if let Some(renderer) = &mut self.renderer {
                    if self.game.terrain_dirty {
                        let mesh = build_terrain_mesh(&self.game.terrain);
                        renderer.set_terrain_mesh(&mesh);
                        self.game.terrain_dirty = false;
                    }
                    let cam = self.game.current_camera();
                    self.game.build_render_batches(&cam, &mut self.instances, &mut self.lights, &mut self.fx_verts);
                    hud_ui::build_hud(
                        &self.game,
                        &cam,
                        renderer.width as f32,
                        renderer.height as f32,
                        &mut self.hud,
                    );
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
                        Err(SurfaceError::Lost) => renderer.resize(renderer.width, renderer.height),
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
