# EXODROP — Helios Engine Architecture

**EXODROP** is a 2026 spiritual successor to classic orbital jump-suit action games, built on a brand-new, from-scratch modular 3D game engine written in Rust (`wgpu` + `winit` + `glam` + `cpal`).

## Design Pillars

1. **Vertical Mobility First**: Every system—from terrain mesa plateaus and dynamic drop shadows to 6-DoF thruster physics, hover-ADS stabilization, afterburner boost-gliding, and aerial dive-slams—is built around effortless, exhilarating 3D flight combat.
2. **2026 Modern GPU Pipeline**: Native Metal (macOS), Vulkan (Linux/Windows), and DX12 (Windows) via `wgpu 26`:
   - 2048×2048 directional shadow mapping with 3×3 PCF soft filtering
   - Linear HDR (`Rgba16Float`) physically-based Cook-Torrance GGX shading + world-space procedural micro-detail
   - Dynamic point lights (explosions, plasma bolts, thruster flares, spire cores)
   - Procedural atmospheric sky dome (Rayleigh/Mie scattering, ringed gas giant, orbital station, starfield, planetary aurora)
   - Additive energy/particle/beam pass + multi-pass HDR Kawase/Gaussian Bloom + ACES filmic tone mapping
   - Crisp 2D vector/bitmap HUD overlay with 3D-projected target lock-on brackets and live radar
3. **Zero External Asset Dependencies**: Every articulated 3D mesh (Exo-Suits, alien Swarmers, Spitters, Interceptors, Dreadnought Goliaths, Drop Pods, Spore Spires, Outpost buildings), planetary terrain biome, and real-time DSP sound effect is synthesized procedurally in code.
4. **Autonomous Squad Wingmen**: Two AI jetpack wingmen (`ECHO-2` and `ECHO-3`) fight alongside you with full flight navigation, flanking, and tactical commands (`F1` Follow, `F2` Focus Target, `F3` Hold Position).

## Crate Map

```
crates/
├── exo-core      Fixed-timestep simulation clock, deterministic RNG, biome configs, input, weapon/squad types
├── exo-physics   Procedural planetary heightfield, static AABB colliders, 6-DoF Exo-Suit thruster controller, raycasts
├── exo-assets    Procedural PBR mesh builder (Exo-Suit rig, 4 alien classes, outposts, spires, 3 biome terrains)
├── exo-gfx       Custom wgpu HDR PBR renderer, shadow map, skybox, GPU particles, multi-pass bloom, ACES, 2D HUD, headless PNG export
├── exo-audio     Real-time cpal procedural DSP synthesizer (jetpack turbine, weapons, explosions, shield alarms, wind)
└── exodrop       Playable game binary: 3 planetary sectors, camera rig, 4 weapons + multi-lock missiles, AI squad, enemy waves & bosses, HUD/menus
```

## Simulation & Rendering Loop

- Physics runs on a deterministic `120 Hz` fixed-step accumulator (`SimClock`), preventing framerate-dependent jump heights or tunneling.
- Rendering runs uncapped/VSync-driven with smooth camera spring dynamics, FOV boost widening, banking roll, and hit-shake trauma decay.
- Headless capture mode (`--screenshot <path>`) drives the exact same `exo-gfx` pipeline offscreen into a PNG for automated visual verification.
