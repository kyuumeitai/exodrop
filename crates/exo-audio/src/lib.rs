use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::{Arc, Mutex};

#[derive(Clone, Copy, Debug)]
pub enum SoundEvent {
    PulseShot,
    RailShot,
    MissileLaunch,
    MortarShot,
    Explosion { intensity: f32 },
    HitConfirm { crit: bool },
    KillConfirm,
    LockBeep,
    ShieldHit,
    ShieldBreak,
    BoostBurst,
    DiveSlam,
    SquadAck,
}

#[derive(Clone, Copy)]
struct Voice {
    kind: VoiceKind,
    age: f32,
    duration: f32,
    gain: f32,
    phase: f32,
}

#[derive(Clone, Copy)]
enum VoiceKind {
    Pulse,
    Rail,
    Missile,
    Mortar,
    Explosion,
    HitTick,
    CritTick,
    KillChord,
    LockBeep,
    ShieldHit,
    ShieldBreak,
    BoostBurst,
    DiveSlam,
    SquadAck,
}

struct SynthState {
    sample_rate: f32,
    thruster: f32,
    boost: f32,
    airspeed: f32,
    muted: bool,
    turbine_phase: f32,
    pad_phase: f32,
    noise_seed: u32,
    lp_jet: f32,
    lp_wind: f32,
    voices: Vec<Voice>,
}

impl SynthState {
    fn new(sample_rate: f32) -> Self {
        Self {
            sample_rate,
            thruster: 0.0,
            boost: 0.0,
            airspeed: 0.0,
            muted: false,
            turbine_phase: 0.0,
            pad_phase: 0.0,
            noise_seed: 0x1234_5678,
            lp_jet: 0.0,
            lp_wind: 0.0,
            voices: Vec::with_capacity(32),
        }
    }

    #[inline]
    fn white_noise(&mut self) -> f32 {
        let mut x = self.noise_seed;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.noise_seed = x;
        ((x as i32) as f32) * (1.0 / 2_147_483_648.0)
    }

    fn trigger(&mut self, ev: SoundEvent) {
        if self.voices.len() >= 28 {
            self.voices.remove(0);
        }
        let (kind, duration, gain) = match ev {
            SoundEvent::PulseShot => (VoiceKind::Pulse, 0.11, 0.20),
            SoundEvent::RailShot => (VoiceKind::Rail, 0.42, 0.34),
            SoundEvent::MissileLaunch => (VoiceKind::Missile, 0.36, 0.24),
            SoundEvent::MortarShot => (VoiceKind::Mortar, 0.28, 0.28),
            SoundEvent::Explosion { intensity } => (VoiceKind::Explosion, 0.72, (0.36 * intensity).clamp(0.10, 0.48)),
            SoundEvent::HitConfirm { crit } => {
                if crit { (VoiceKind::CritTick, 0.14, 0.24) } else { (VoiceKind::HitTick, 0.07, 0.18) }
            }
            SoundEvent::KillConfirm => (VoiceKind::KillChord, 0.34, 0.24),
            SoundEvent::LockBeep => (VoiceKind::LockBeep, 0.08, 0.16),
            SoundEvent::ShieldHit => (VoiceKind::ShieldHit, 0.16, 0.22),
            SoundEvent::ShieldBreak => (VoiceKind::ShieldBreak, 0.55, 0.30),
            SoundEvent::BoostBurst => (VoiceKind::BoostBurst, 0.30, 0.26),
            SoundEvent::DiveSlam => (VoiceKind::DiveSlam, 0.65, 0.42),
            SoundEvent::SquadAck => (VoiceKind::SquadAck, 0.18, 0.16),
        };
        self.voices.push(Voice { kind, age: 0.0, duration, gain, phase: 0.0 });
    }

    fn next_sample(&mut self) -> f32 {
        if self.muted {
            return 0.0;
        }
        let dt = 1.0 / self.sample_rate;
        let n = self.white_noise();

        // 1. Continuous jetpack turbine + afterburner roar
        let jet_lvl = (self.thruster + self.boost * 1.35).clamp(0.0, 1.8);
        let cutoff = (0.04 + jet_lvl * 0.16).clamp(0.02, 0.45);
        self.lp_jet += (n - self.lp_jet) * cutoff;
        let turbine_hz = 115.0 + self.thruster * 140.0 + self.boost * 210.0;
        self.turbine_phase = (self.turbine_phase + turbine_hz * dt).fract();
        let whine = (self.turbine_phase * std::f32::consts::TAU).sin() * 0.28;
        let jet_sig = (self.lp_jet * 0.75 + whine) * (jet_lvl * 0.16).min(0.26);

        // 2. Subtle high-altitude rushing wind + sci-fi space pad
        self.lp_wind += (n - self.lp_wind) * 0.015;
        let wind_gain = (self.airspeed / 36.0).clamp(0.0, 1.0) * 0.12 + 0.018;
        self.pad_phase = (self.pad_phase + 55.0 * dt).fract();
        let pad = ((self.pad_phase * std::f32::consts::TAU).sin()
            + (self.pad_phase * 1.5 * std::f32::consts::TAU).sin() * 0.5)
            * 0.014;

        let mut mix = jet_sig + self.lp_wind * wind_gain + pad;

        // 3. Polyphonic one-shot DSP voices
        let mut i = 0;
        while i < self.voices.len() {
            let v = &mut self.voices[i];
            let t = v.age / v.duration;
            if t >= 1.0 {
                self.voices.swap_remove(i);
                continue;
            }
            let env = (1.0 - t) * (1.0 - t);
            let s = match v.kind {
                VoiceKind::Pulse => {
                    let hz = 920.0 * (1.0 - t * 0.72) + 180.0;
                    v.phase += hz * dt;
                    ((v.phase * std::f32::consts::TAU).sin() + n * 0.18 * (1.0 - t)) * env
                }
                VoiceKind::Rail => {
                    let hz = 240.0 * (1.0 - t * 0.6) + 90.0;
                    v.phase += hz * dt;
                    let crack = n * (1.0 - t).powi(3) * 1.4;
                    let ring = (v.phase * std::f32::consts::TAU).sin()
                        + (v.phase * 4.5 * std::f32::consts::TAU).sin() * 0.45;
                    (crack + ring * 0.6) * env
                }
                VoiceKind::Missile => {
                    let hz = 320.0 + t * 520.0;
                    v.phase += hz * dt;
                    (n * 0.75 + (v.phase * std::f32::consts::TAU).sin() * 0.35) * env
                }
                VoiceKind::Mortar => {
                    let hz = 160.0 * (1.0 - t * 0.55);
                    v.phase += hz * dt;
                    ((v.phase * std::f32::consts::TAU).sin() * 1.2 + n * 0.25) * env
                }
                VoiceKind::Explosion => {
                    let hz = 95.0 * (1.0 - t * 0.75);
                    v.phase += hz * dt;
                    let sub = (v.phase * std::f32::consts::TAU).sin() * 0.9;
                    let rumble = (n * 0.85) * (1.0 - t);
                    (sub + rumble) * env
                }
                VoiceKind::HitTick => {
                    v.phase += 1750.0 * dt;
                    (v.phase * std::f32::consts::TAU).sin() * env
                }
                VoiceKind::CritTick => {
                    v.phase += (2100.0 + t * 400.0) * dt;
                    ((v.phase * std::f32::consts::TAU).sin() + (v.phase * 1.5 * std::f32::consts::TAU).sin() * 0.5) * env
                }
                VoiceKind::KillChord => {
                    v.phase += 660.0 * dt;
                    let a = (v.phase * std::f32::consts::TAU).sin();
                    let b = (v.phase * 1.25 * std::f32::consts::TAU).sin();
                    let c = (v.phase * 1.5 * std::f32::consts::TAU).sin();
                    (a + b * 0.7 + c * 0.6) * 0.45 * env
                }
                VoiceKind::LockBeep => {
                    v.phase += 1480.0 * dt;
                    (v.phase * std::f32::consts::TAU).sin() * env
                }
                VoiceKind::ShieldHit => {
                    v.phase += (480.0 - t * 220.0) * dt;
                    ((v.phase * std::f32::consts::TAU).sin() * 0.6 + n * 0.4) * env
                }
                VoiceKind::ShieldBreak => {
                    let hz = 760.0 * (1.0 - t * 0.72);
                    v.phase += hz * dt;
                    let sq = if (v.phase.fract()) > 0.5 { 0.6 } else { -0.6 };
                    (sq + n * 0.3) * env
                }
                VoiceKind::BoostBurst => {
                    let hz = 180.0 + t * 420.0;
                    v.phase += hz * dt;
                    (n * 0.65 + (v.phase * std::f32::consts::TAU).sin() * 0.5) * env
                }
                VoiceKind::DiveSlam => {
                    let hz = 120.0 * (1.0 - t * 0.82);
                    v.phase += hz * dt;
                    ((v.phase * std::f32::consts::TAU).sin() * 1.3 + n * 0.75) * env
                }
                VoiceKind::SquadAck => {
                    let hz = if t < 0.5 { 880.0 } else { 1175.0 };
                    v.phase += hz * dt;
                    (v.phase * std::f32::consts::TAU).sin() * (1.0 - t)
                }
            };
            mix += s * v.gain;
            v.age += dt;
            i += 1;
        }

        // Soft limiter
        (mix * 1.15).tanh() * 0.78
    }
}

pub struct AudioEngine {
    state: Arc<Mutex<SynthState>>,
    _stream: Option<cpal::Stream>,
}

impl AudioEngine {
    pub fn silent() -> Self {
        Self {
            state: Arc::new(Mutex::new(SynthState::new(48_000.0))),
            _stream: None,
        }
    }

    pub fn new() -> Self {
        let host = cpal::default_host();
        let Some(device) = host.default_output_device() else {
            log::warn!("No audio output device found; running silent.");
            return Self::silent();
        };
        let Ok(config) = device.default_output_config() else {
            return Self::silent();
        };
        let sample_rate = config.sample_rate().0 as f32;
        let channels = config.channels() as usize;
        let state = Arc::new(Mutex::new(SynthState::new(sample_rate)));
        let cb_state = Arc::clone(&state);

        let stream_res = match config.sample_format() {
            cpal::SampleFormat::F32 => device.build_output_stream(
                &config.into(),
                move |data: &mut [f32], _| {
                    if let Ok(mut s) = cb_state.lock() {
                        for frame in data.chunks_mut(channels) {
                            let v = s.next_sample();
                            for ch in frame.iter_mut() {
                                *ch = v;
                            }
                        }
                    }
                },
                |err| log::warn!("audio stream error: {err}"),
                None,
            ),
            cpal::SampleFormat::I16 => device.build_output_stream(
                &config.into(),
                move |data: &mut [i16], _| {
                    if let Ok(mut s) = cb_state.lock() {
                        for frame in data.chunks_mut(channels) {
                            let v = (s.next_sample() * 32767.0) as i16;
                            for ch in frame.iter_mut() {
                                *ch = v;
                            }
                        }
                    }
                },
                |err| log::warn!("audio stream error: {err}"),
                None,
            ),
            _ => return Self::silent(),
        };

        let stream = match stream_res {
            Ok(s) => {
                let _ = s.play();
                Some(s)
            }
            Err(e) => {
                log::warn!("Audio init failed ({e}); running silent.");
                None
            }
        };

        Self {
            state,
            _stream: stream,
        }
    }

    pub fn update_telemetry(&self, thruster: f32, boost: f32, airspeed: f32) {
        if let Ok(mut s) = self.state.try_lock() {
            s.thruster = thruster;
            s.boost = boost;
            s.airspeed = airspeed;
        }
    }

    pub fn play(&self, event: SoundEvent) {
        if let Ok(mut s) = self.state.try_lock() {
            s.trigger(event);
        }
    }
}

impl Default for AudioEngine {
    fn default() -> Self {
        Self::new()
    }
}
