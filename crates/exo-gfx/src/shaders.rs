pub const SHADOW_WGSL: &str = r#"
struct ShadowUniform {
    light_view_proj: mat4x4<f32>,
};

@group(0) @binding(0) var<uniform> shadow_u: ShadowUniform;

struct VertexIn {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) color: vec3<f32>,
    @location(3) metallic: f32,
    @location(4) emissive: vec3<f32>,
    @location(5) roughness: f32,
};

struct InstanceIn {
    @location(6) m0: vec4<f32>,
    @location(7) m1: vec4<f32>,
    @location(8) m2: vec4<f32>,
    @location(9) m3: vec4<f32>,
    @location(10) tint: vec4<f32>,
    @location(11) emissive_boost: vec4<f32>,
};

@vertex
fn vs_main(v: VertexIn, inst: InstanceIn) -> @builtin(position) vec4<f32> {
    let model = mat4x4<f32>(inst.m0, inst.m1, inst.m2, inst.m3);
    let wp = model * vec4<f32>(v.position, 1.0);
    return shadow_u.light_view_proj * wp;
}
"#;

pub const SKY_WGSL: &str = r#"
struct PointLightGpu {
    pos_radius: vec4<f32>,
    color_intensity: vec4<f32>,
};

struct FrameUniform {
    view_proj: mat4x4<f32>,
    inv_view_proj: mat4x4<f32>,
    light_view_proj: mat4x4<f32>,
    camera_pos: vec4<f32>,
    sun_dir: vec4<f32>,
    sun_color: vec4<f32>,
    sky_zenith: vec4<f32>,
    sky_horizon: vec4<f32>,
    ground_bounce: vec4<f32>,
    fog_params: vec4<f32>,      // rgb = color, a = density
    planet_color: vec4<f32>,    // rgb = planet, a = aurora_strength
    ring_color: vec4<f32>,      // rgb = ring, a = time
    post_params: vec4<f32>,     // x = boost_fx, y = damage_fx, z = num_lights, w = aspect
    lights: array<PointLightGpu, 16>,
};

@group(0) @binding(0) var<uniform> frame: FrameUniform;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) ndc: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vi: u32) -> VsOut {
    var pts = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(3.0, -1.0),
        vec2<f32>(-1.0, 3.0)
    );
    let p = pts[vi];
    var out: VsOut;
    out.pos = vec4<f32>(p, 0.9999, 1.0);
    out.ndc = p;
    return out;
}

fn hash31(p: vec3<f32>) -> f32 {
    var q = fract(p * 0.1031);
    q += dot(q, q.yzx + 33.33);
    return fract((q.x + q.y) * q.z);
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let far_clip = vec4<f32>(in.ndc, 1.0, 1.0);
    let world_h = frame.inv_view_proj * far_clip;
    let world_p = world_h.xyz / world_h.w;
    let dir = normalize(world_p - frame.camera_pos.xyz);

    let y = clamp(dir.y, -0.2, 1.0);
    let horizon_t = pow(1.0 - max(y, 0.0), 3.2);
    var col = mix(frame.sky_zenith.rgb, frame.sky_horizon.rgb, horizon_t);

    // Lower hemisphere atmospheric ground scatter
    if (dir.y < 0.0) {
        col = mix(frame.sky_horizon.rgb, frame.ground_bounce.rgb * 0.65, clamp(-dir.y * 4.0, 0.0, 1.0));
    }

    // Crisp Starfield at high elevation
    let star_mask = smoothstep(0.08, 0.55, dir.y);
    let cell = floor(dir * 260.0);
    let h = hash31(cell);
    if (h > 0.9945 && star_mask > 0.01) {
        let twinkle = 0.72 + 0.28 * sin(frame.ring_color.a * 2.4 + h * 140.0);
        let brightness = smoothstep(0.9945, 0.9995, h) * 2.2 * twinkle * star_mask;
        col += vec3<f32>(0.85, 0.92, 1.0) * brightness;
    }

    // --- RINGED GAS GIANT IN ALIEN SKY ---
    let planet_dir = normalize(vec3<f32>(-0.42, 0.36, -0.83));
    let sun_dir = normalize(frame.sun_dir.xyz);
    let cos_ang = dot(dir, planet_dir);
    let p_rad = 0.145;
    let ang = acos(clamp(cos_ang, -1.0, 1.0));

    // Planet disk
    var hit_planet = false;
    if (ang < p_rad && dir.y > -0.05) {
        hit_planet = true;
        let up_ref = vec3<f32>(0.0, 1.0, 0.0);
        let right = normalize(cross(up_ref, planet_dir));
        let up = normalize(cross(planet_dir, right));
        let rel = dir - planet_dir * cos_ang;
        let uv = vec2<f32>(dot(rel, right), dot(rel, up)) / p_rad;
        let r2 = dot(uv, uv);
        let z = sqrt(max(1.0 - r2, 0.0));
        let sphere_n = normalize(right * uv.x + up * uv.y - planet_dir * z);

        // Banded gas-giant atmosphere
        let bands = 0.5 + 0.5 * sin(uv.y * 18.0 + sin(uv.x * 4.0) * 0.7);
        let p_albedo = mix(frame.planet_color.rgb * 0.65, frame.planet_color.rgb * 1.15, bands);
        let ndl = smoothstep(-0.18, 0.65, dot(sphere_n, sun_dir));
        let rim = pow(1.0 - z, 3.0) * 0.45 * max(dot(sphere_n, sun_dir) + 0.3, 0.0);
        let planet_lit = p_albedo * (0.04 + ndl * 1.15) + frame.sky_horizon.rgb * rim;
        let edge_aa = smoothstep(p_rad, p_rad - 0.003, ang);
        let atmo_fade = smoothstep(-0.02, 0.14, dir.y);
        col = mix(col, planet_lit + frame.sky_horizon.rgb * 0.18, edge_aa * atmo_fade);
    }

    // Ringed halo around gas giant
    let ring_normal = normalize(vec3<f32>(0.24, 0.92, -0.30));
    let denom = dot(dir, ring_normal);
    if (abs(denom) > 0.001 && dir.y > 0.0) {
        let t_plane = dot(planet_dir, ring_normal) / denom;
        if (t_plane > 0.0) {
            let hit = dir * t_plane - planet_dir;
            let dist = length(hit);
            if (dist > p_rad * 1.32 && dist < p_rad * 2.55) {
                let in_front = t_plane < 1.0;
                if (!hit_planet || in_front) {
                    let band = 0.55 + 0.45 * sin(dist * 170.0) * cos(dist * 55.0);
                    let fade = smoothstep(p_rad * 1.32, p_rad * 1.48, dist)
                             * (1.0 - smoothstep(p_rad * 2.25, p_rad * 2.55, dist));
                    let ring_alpha = fade * band * 0.46 * smoothstep(0.0, 0.16, dir.y);
                    col = mix(col, frame.ring_color.rgb * 1.25, ring_alpha);
                }
            }
        }
    }

    // --- PLANETARY AURORA CURTAINS (Europa / high energy sectors) ---
    let aurora_str = frame.planet_color.a;
    if (aurora_str > 0.01 && dir.y > 0.05) {
        let t = frame.ring_color.a;
        let a_uv = dir.xz / (dir.y + 0.25);
        let wave1 = sin(a_uv.x * 2.8 + t * 0.35 + sin(a_uv.y * 2.1 - t * 0.25));
        let wave2 = sin(a_uv.x * 5.2 - a_uv.y * 3.0 + t * 0.5);
        let ribbon = pow(clamp(1.0 - abs(wave1 * 0.65 + wave2 * 0.35), 0.0, 1.0), 4.0);
        let height_mask = smoothstep(0.06, 0.26, dir.y) * (1.0 - smoothstep(0.55, 0.90, dir.y));
        let aurora_col = mix(vec3<f32>(0.10, 1.35, 0.85), vec3<f32>(0.45, 0.35, 1.5), smoothstep(0.15, 0.55, dir.y));
        col += aurora_col * ribbon * height_mask * aurora_str * 0.85;
    }

    // --- SUN DISK & MIE SCATTER HALO ---
    let sun_dot = max(dot(dir, sun_dir), 0.0);
    let mie = pow(sun_dot, 10.0) * 0.42 + pow(sun_dot, 96.0) * 1.15;
    let disk = smoothstep(0.9988, 0.9996, sun_dot) * 7.0;
    col += frame.sun_color.rgb * (mie * 0.35 + disk);

    // Horizon atmospheric bloom haze
    let haze = exp(-abs(dir.y) * 9.5) * 0.35;
    col += frame.fog_params.rgb * haze;

    return vec4<f32>(col, 1.0);
}
"#;

pub const PBR_WGSL: &str = r#"
struct PointLightGpu {
    pos_radius: vec4<f32>,
    color_intensity: vec4<f32>,
};

struct FrameUniform {
    view_proj: mat4x4<f32>,
    inv_view_proj: mat4x4<f32>,
    light_view_proj: mat4x4<f32>,
    camera_pos: vec4<f32>,
    sun_dir: vec4<f32>,
    sun_color: vec4<f32>,
    sky_zenith: vec4<f32>,
    sky_horizon: vec4<f32>,
    ground_bounce: vec4<f32>,
    fog_params: vec4<f32>,
    planet_color: vec4<f32>,
    ring_color: vec4<f32>,
    post_params: vec4<f32>,
    lights: array<PointLightGpu, 16>,
};

@group(0) @binding(0) var<uniform> frame: FrameUniform;
@group(1) @binding(0) var shadow_tex: texture_depth_2d;
@group(1) @binding(1) var shadow_sampler: sampler_comparison;

struct VertexIn {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) color: vec3<f32>,
    @location(3) metallic: f32,
    @location(4) emissive: vec3<f32>,
    @location(5) roughness: f32,
};

struct InstanceIn {
    @location(6) m0: vec4<f32>,
    @location(7) m1: vec4<f32>,
    @location(8) m2: vec4<f32>,
    @location(9) m3: vec4<f32>,
    @location(10) tint: vec4<f32>,
    @location(11) emissive_boost: vec4<f32>,
};

struct VsOut {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) world_pos: vec3<f32>,
    @location(1) world_normal: vec3<f32>,
    @location(2) color: vec3<f32>,
    @location(3) emissive: vec3<f32>,
    @location(4) metallic: f32,
    @location(5) roughness: f32,
};

@vertex
fn vs_main(v: VertexIn, inst: InstanceIn) -> VsOut {
    let model = mat4x4<f32>(inst.m0, inst.m1, inst.m2, inst.m3);
    let wp = model * vec4<f32>(v.position, 1.0);
    let nmat = mat3x3<f32>(inst.m0.xyz, inst.m1.xyz, inst.m2.xyz);
    let wn = normalize(nmat * v.normal);

    var out: VsOut;
    out.clip_pos = frame.view_proj * wp;
    out.world_pos = wp.xyz;
    out.world_normal = wn;
    out.color = v.color * inst.tint.rgb;
    out.emissive = v.emissive * inst.emissive_boost.a + inst.emissive_boost.rgb;
    out.metallic = v.metallic;
    out.roughness = v.roughness;
    return out;
}

const PI: f32 = 3.14159265;

fn hash3(p: vec3<f32>) -> f32 {
    var q = fract(p * 0.1031);
    q += dot(q, q.yzx + 33.33);
    return fract((q.x + q.y) * q.z);
}

fn value_noise3(p: vec3<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    let n000 = hash3(i + vec3<f32>(0.0, 0.0, 0.0));
    let n100 = hash3(i + vec3<f32>(1.0, 0.0, 0.0));
    let n010 = hash3(i + vec3<f32>(0.0, 1.0, 0.0));
    let n110 = hash3(i + vec3<f32>(1.0, 1.0, 0.0));
    let n001 = hash3(i + vec3<f32>(0.0, 0.0, 1.0));
    let n101 = hash3(i + vec3<f32>(1.0, 0.0, 1.0));
    let n011 = hash3(i + vec3<f32>(0.0, 1.0, 1.0));
    let n111 = hash3(i + vec3<f32>(1.0, 1.0, 1.0));
    let nx00 = mix(n000, n100, u.x);
    let nx10 = mix(n010, n110, u.x);
    let nx01 = mix(n001, n101, u.x);
    let nx11 = mix(n011, n111, u.x);
    let nxy0 = mix(nx00, nx10, u.y);
    let nxy1 = mix(nx01, nx11, u.y);
    return mix(nxy0, nxy1, u.z);
}

fn ggx_d(ndh: f32, rough: f32) -> f32 {
    let a = rough * rough;
    let a2 = a * a;
    let denom = ndh * ndh * (a2 - 1.0) + 1.0;
    return a2 / max(PI * denom * denom, 0.0001);
}

fn geom_schlick(ndv: f32, rough: f32) -> f32 {
    let r = rough + 1.0;
    let k = (r * r) / 8.0;
    return ndv / max(ndv * (1.0 - k) + k, 0.0001);
}

fn pbr_light(n: vec3<f32>, v: vec3<f32>, l: vec3<f32>, albedo: vec3<f32>, metallic: f32, rough: f32, radiance: vec3<f32>) -> vec3<f32> {
    let ndl = max(dot(n, l), 0.0);
    if (ndl <= 0.0) {
        return vec3<f32>(0.0);
    }
    let h = normalize(v + l);
    let ndv = max(dot(n, v), 0.001);
    let ndh = max(dot(n, h), 0.0);
    let hdv = max(dot(h, v), 0.0);

    let f0 = mix(vec3<f32>(0.04), albedo, metallic);
    let fresnel = f0 + (vec3<f32>(1.0) - f0) * pow(1.0 - hdv, 5.0);
    let ndf = ggx_d(ndh, rough);
    let g = geom_schlick(ndv, rough) * geom_schlick(ndl, rough);
    let spec = (ndf * g * fresnel) / max(4.0 * ndv * ndl, 0.001);
    let kd = (vec3<f32>(1.0) - fresnel) * (1.0 - metallic);
    return (kd * albedo / PI + spec) * radiance * ndl;
}

fn sample_shadow(world_pos: vec3<f32>, world_normal: vec3<f32>) -> f32 {
    let sun_dir = normalize(frame.sun_dir.xyz);
    let bias_pos = world_pos + world_normal * 0.28 + sun_dir * 0.16;
    let lp = frame.light_view_proj * vec4<f32>(bias_pos, 1.0);
    let ndc = lp.xyz / lp.w;
    let uv = vec2<f32>(ndc.x * 0.5 + 0.5, -ndc.y * 0.5 + 0.5);
    if (uv.x < 0.01 || uv.x > 0.99 || uv.y < 0.01 || uv.y > 0.99 || ndc.z < 0.0 || ndc.z > 1.0) {
        return 1.0;
    }
    let texel = 1.0 / 2048.0;
    var sum = 0.0;
    for (var dy = -1; dy <= 1; dy++) {
        for (var dx = -1; dx <= 1; dx++) {
            let offset = vec2<f32>(f32(dx), f32(dy)) * texel * 1.4;
            sum += textureSampleCompare(shadow_tex, shadow_sampler, uv + offset, ndc.z);
        }
    }
    return sum / 9.0;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let n = normalize(in.world_normal);
    let v_vec = frame.camera_pos.xyz - in.world_pos;
    let dist = length(v_vec);
    let v = v_vec / max(dist, 0.0001);

    // Subtle procedural world-space micro-detail so armor & terrain feel crisp & tactile!
    let micro1 = value_noise3(in.world_pos * 0.45);
    let micro2 = value_noise3(in.world_pos * 2.1);
    let grain = (micro1 * 0.65 + micro2 * 0.35) - 0.5;
    let albedo = clamp(in.color * (1.0 + grain * 0.16), vec3<f32>(0.01), vec3<f32>(1.0));
    let rough = clamp(in.roughness + grain * 0.08, 0.08, 0.98);

    // Directional Sun + 3x3 PCF Soft Shadow
    let sun_dir = normalize(frame.sun_dir.xyz);
    let shadow = sample_shadow(in.world_pos, n);
    var lo = pbr_light(n, v, sun_dir, albedo, in.metallic, rough, frame.sun_color.rgb) * shadow;

    // Dynamic HDR Point Lights (plasma bolts, explosions, thrusters, beacons)
    let num_lights = min(u32(frame.post_params.z), 16u);
    for (var i = 0u; i < num_lights; i++) {
        let pl = frame.lights[i];
        let to_l = pl.pos_radius.xyz - in.world_pos;
        let d = length(to_l);
        let r = pl.pos_radius.w;
        if (d < r && d > 0.001) {
            let l_dir = to_l / d;
            let atten = pow(clamp(1.0 - d / r, 0.0, 1.0), 2.0) / (1.0 + d * d * 0.04);
            lo += pbr_light(n, v, l_dir, albedo, in.metallic, rough, pl.color_intensity.rgb * atten);
        }
    }

    // Hemisphere Sky + Ground Bounce IBL Approximation
    let sky_t = n.y * 0.5 + 0.5;
    let hemi = mix(frame.ground_bounce.rgb, mix(frame.sky_horizon.rgb, frame.sky_zenith.rgb, 0.55), sky_t);
    let refl = reflect(-v, n);
    let env_spec = mix(frame.ground_bounce.rgb, frame.sky_horizon.rgb * 1.15, clamp(refl.y * 0.5 + 0.5, 0.0, 1.0));
    let f0 = mix(vec3<f32>(0.04), albedo, in.metallic);
    let fresnel = f0 + (vec3<f32>(1.0) - f0) * pow(1.0 - max(dot(n, v), 0.0), 4.0);
    let ambient = albedo * (1.0 - in.metallic * 0.7) * hemi * 0.42 + env_spec * fresnel * (1.0 - rough * 0.75) * 0.55;

    // Subtle heroic atmospheric rim highlight
    let rim = pow(1.0 - max(dot(n, v), 0.0), 3.5) * 0.16 * (0.4 + 0.6 * max(dot(n, sun_dir), 0.0));
    var col = lo + ambient + frame.sky_horizon.rgb * rim + in.emissive;

    // Exponential height-aware atmospheric fog
    let height_factor = exp(-max(in.world_pos.y - 6.0, 0.0) * 0.015);
    let fog_amount = 1.0 - exp(-dist * frame.fog_params.a * (0.55 + 0.45 * height_factor));
    let sun_scatter = pow(max(dot(-v, sun_dir), 0.0), 6.0);
    let fog_col = mix(frame.fog_params.rgb, frame.sun_color.rgb * 0.48, sun_scatter * 0.38);
    col = mix(col, fog_col, clamp(fog_amount, 0.0, 0.92));

    return vec4<f32>(col, 1.0);
}
"#;

pub const FX_WGSL: &str = r#"
struct FrameUniform {
    view_proj: mat4x4<f32>,
};

@group(0) @binding(0) var<uniform> frame: FrameUniform;

struct FxIn {
    @location(0) position: vec3<f32>,
    @location(1) color: vec4<f32>,
};

struct FxOut {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) color: vec4<f32>,
};

@vertex
fn vs_main(v: FxIn) -> FxOut {
    var out: FxOut;
    out.clip_pos = frame.view_proj * vec4<f32>(v.position, 1.0);
    out.color = v.color;
    return out;
}

@fragment
fn fs_main(in: FxOut) -> @location(0) vec4<f32> {
    return in.color;
}
"#;

pub const BLOOM_WGSL: &str = r#"
@group(0) @binding(0) var src_tex: texture_2d<f32>;
@group(0) @binding(1) var src_samp: sampler;

struct BloomParams {
    dir: vec2<f32>,      // (1/w, 0) for horiz, (0, 1/h) for vert, (0,0) for bright extract
    threshold: f32,
    _pad: f32,
};
@group(0) @binding(2) var<uniform> params: BloomParams;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vi: u32) -> VsOut {
    var pts = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(3.0, -1.0),
        vec2<f32>(-1.0, 3.0)
    );
    let p = pts[vi];
    var out: VsOut;
    out.pos = vec4<f32>(p, 0.0, 1.0);
    out.uv = vec2<f32>(p.x * 0.5 + 0.5, 0.5 - p.y * 0.5);
    return out;
}

@fragment
fn fs_bright(in: VsOut) -> @location(0) vec4<f32> {
    let c = textureSample(src_tex, src_samp, in.uv).rgb;
    let luma = dot(c, vec3<f32>(0.2126, 0.7152, 0.0722));
    let soft = clamp((luma - params.threshold) / max(luma, 0.001), 0.0, 1.0);
    let contrib = max(luma - params.threshold, 0.0) * 0.55 + soft * 0.25;
    return vec4<f32>(c * clamp(contrib, 0.0, 4.0), 1.0);
}

@fragment
fn fs_blur(in: VsOut) -> @location(0) vec4<f32> {
    var weights = array<f32, 5>(0.227027, 0.1945946, 0.1216216, 0.054054, 0.016216);
    var acc = textureSample(src_tex, src_samp, in.uv).rgb * weights[0];
    for (var i = 1; i < 5; i++) {
        let off = params.dir * f32(i) * 1.65;
        acc += textureSample(src_tex, src_samp, in.uv + off).rgb * weights[i];
        acc += textureSample(src_tex, src_samp, in.uv - off).rgb * weights[i];
    }
    return vec4<f32>(acc, 1.0);
}
"#;

pub const TONEMAP_WGSL: &str = r#"
@group(0) @binding(0) var hdr_tex: texture_2d<f32>;
@group(0) @binding(1) var bloom_tex: texture_2d<f32>;
@group(0) @binding(2) var lin_samp: sampler;

struct PostUniform {
    boost_fx: f32,
    damage_fx: f32,
    time: f32,
    exposure: f32,
};
@group(0) @binding(3) var<uniform> post: PostUniform;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vi: u32) -> VsOut {
    var pts = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(3.0, -1.0),
        vec2<f32>(-1.0, 3.0)
    );
    let p = pts[vi];
    var out: VsOut;
    out.pos = vec4<f32>(p, 0.0, 1.0);
    out.uv = vec2<f32>(p.x * 0.5 + 0.5, 0.5 - p.y * 0.5);
    return out;
}

fn aces_filmic(x: vec3<f32>) -> vec3<f32> {
    let a = 2.51;
    let b = 0.03;
    let c = 2.43;
    let d = 0.59;
    let e = 0.14;
    return clamp((x * (a * x + b)) / (x * (c * x + d) + e), vec3<f32>(0.0), vec3<f32>(1.0));
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let centered = in.uv - vec2<f32>(0.5);
    let r = length(centered);

    // Subtle radial chromatic kick during supersonic afterburner boost!
    let ca = post.boost_fx * r * 0.0065;
    let dir = centered / max(r, 0.0001);
    let hdr_r = textureSample(hdr_tex, lin_samp, in.uv + dir * ca).r;
    let hdr_g = textureSample(hdr_tex, lin_samp, in.uv).g;
    let hdr_b = textureSample(hdr_tex, lin_samp, in.uv - dir * ca).b;
    var col = vec3<f32>(hdr_r, hdr_g, hdr_b);

    let bloom = textureSample(bloom_tex, lin_samp, in.uv).rgb;
    col += bloom * 0.58;

    // Speed radial streaks on boost
    if (post.boost_fx > 0.02) {
        let ang = atan2(centered.y, centered.x);
        let streaks = pow(abs(sin(ang * 24.0 + post.time * 18.0)), 14.0);
        let edge_mask = smoothstep(0.32, 0.72, r);
        col += vec3<f32>(0.25, 0.85, 1.0) * streaks * edge_mask * post.boost_fx * 0.45;
    }

    // Exposure & ACES Filmic Tone Map
    col = aces_filmic(col * post.exposure);

    // Cinematic vignette + damage crimson pulse
    let vig = 1.0 - smoothstep(0.45, 0.92, r) * (0.28 + post.boost_fx * 0.14);
    col *= vig;
    let dmg_ring = smoothstep(0.25, 0.78, r) * post.damage_fx;
    col = mix(col, vec3<f32>(0.95, 0.12, 0.08), clamp(dmg_ring * 0.65, 0.0, 0.75));

    return vec4<f32>(col, 1.0);
}
"#;

pub const HUD_WGSL: &str = r#"
struct HudUniform {
    screen_size: vec2<f32>,
    _pad: vec2<f32>,
};
@group(0) @binding(0) var<uniform> hud: HudUniform;

struct HudIn {
    @location(0) pos: vec2<f32>,
    @location(1) color: vec4<f32>,
};

struct HudOut {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) color: vec4<f32>,
};

@vertex
fn vs_main(v: HudIn) -> HudOut {
    let nx = (v.pos.x / hud.screen_size.x) * 2.0 - 1.0;
    let ny = 1.0 - (v.pos.y / hud.screen_size.y) * 2.0;
    var out: HudOut;
    out.clip_pos = vec4<f32>(nx, ny, 0.0, 1.0);
    out.color = v.color;
    return out;
}

@fragment
fn fs_main(in: HudOut) -> @location(0) vec4<f32> {
    return in.color;
}
"#;
