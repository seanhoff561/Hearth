// Water surfaces (`water.rs`), shared by the full-detail terrain and the distant land
// (`common.wgsl` comes first): wind-driven waves from a tiling slope texture, the sky a mirror
// would show, and the sun's glitter.

struct WaterParams {
    // xy: the direction the wind blows toward (world x, z), z: wind speed (m/s), w: quality
    // (0 low, 1 medium, 2 high).
    wind: vec4<f32>,
    // xy: 1 / render size, z: camera near plane.
    screen: vec4<f32>,
};

@group(2) @binding(0) var<uniform> water: WaterParams;
// Tiling wave slopes (x, z) × 0.5 + 0.5.
@group(2) @binding(3) var wave_tex: texture_2d<f32>;
@group(2) @binding(4) var wave_samp: sampler;

// Reflectance of water seen straight on (index of refraction 1.33).
const WATER_F0: f32 = 0.02;

fn rotate(v: vec2<f32>, d: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(v.x * d.x + v.y * d.y, -v.x * d.y + v.y * d.x);
}

fn unrotate(v: vec2<f32>, d: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(v.x * d.x - v.y * d.y, v.x * d.y + v.y * d.x);
}

// How steep the wind makes the waves.
fn wave_steepness() -> f32 {
    return clamp(0.05 + 0.03 * water.wind.z, 0.05, 0.4);
}

// Wave slopes at a world point (blocks): two scales of the tiling wave field travelling with the
// wind, steeper in a stronger wind. `gx`, `gy` are the point's screen gradients (taken in uniform
// control flow) and choose the mip level, so distant water flattens.
fn wave_slope(p: vec2<f32>, gx: vec2<f32>, gy: vec2<f32>) -> vec2<f32> {
    let d = water.wind.xy;
    let drift = (0.5 + 0.2 * water.wind.z) * g.params.x;
    // In the wind's frame (x along the wind): long swell and a shorter chop at an angle.
    let q = rotate(p, d);
    let s1 = 1.0 / 40.0;
    let a = textureSampleGrad(wave_tex, wave_samp, (q + vec2<f32>(drift, 0.0)) * s1,
        rotate(gx, d) * s1, rotate(gy, d) * s1).xy * 2.0 - 1.0;
    let c = vec2<f32>(0.8, 0.6);
    let s2 = 1.0 / 11.0;
    let q2 = rotate(q, c) + vec2<f32>(drift * 1.4, 0.0);
    let b = textureSampleGrad(wave_tex, wave_samp, q2 * s2, rotate(rotate(gx, d), c) * s2,
        rotate(rotate(gy, d), c) * s2).xy * 2.0 - 1.0;
    return unrotate(a * 0.6 + unrotate(b, c) * 0.4, d) * wave_steepness();
}

// Wave slopes of distant water: the long swell alone (the chop is under a pixel there).
fn wave_slope_far(p: vec2<f32>, gx: vec2<f32>, gy: vec2<f32>) -> vec2<f32> {
    let d = water.wind.xy;
    let drift = (0.5 + 0.2 * water.wind.z) * g.params.x;
    let q = rotate(p, d);
    let s1 = 1.0 / 40.0;
    let a = textureSampleGrad(wave_tex, wave_samp, (q + vec2<f32>(drift, 0.0)) * s1,
        rotate(gx, d) * s1, rotate(gy, d) * s1).xy * 2.0 - 1.0;
    return unrotate(a * 0.6, d) * wave_steepness();
}

// Schlick's Fresnel for water seen at cos θ = `ndv`.
fn water_fresnel(ndv: f32) -> f32 {
    return WATER_F0 + (1.0 - WATER_F0) * pow(1.0 - clamp(ndv, 0.0, 1.0), 5.0);
}

// The sky a mirror would show in direction `r` (waves may tip it below the horizon: the sky
// just above it then), grey under a cloud deck.
fn water_sky(r: vec3<f32>) -> vec3<f32> {
    let clear = textureSampleLevel(skyview, sky_samp, skyview_uv(vec3<f32>(r.x, abs(r.y), r.z)), 0.0).rgb;
    return mix(clear, g.overcast.rgb, g.overcast.w);
}

// The sun's glitter on water of normal `n` seen from `view` at `dist` blocks: GGX on a surface as
// rough as the wind makes it, and rougher where the texture's waves average out with distance
// (their slopes still spread the light into a glitter path). `open`: how open it is to the sky.
fn water_glitter(n: vec3<f32>, view: vec3<f32>, dist: f32, open: f32) -> vec3<f32> {
    let rough = clamp(0.1 + 0.025 * water.wind.z, 0.1, 0.35) + clamp(dist / 2000.0, 0.0, 0.15);
    let a2 = rough * rough * rough * rough;
    let h = normalize(view + g.sun.xyz);
    let ndh = max(dot(n, h), 0.0);
    let dd = ndh * ndh * (a2 - 1.0) + 1.0;
    let ggx = a2 / (3.14159265 * dd * dd);
    let fh = water_fresnel(dot(h, view));
    let lit = select(0.0, 1.0, dot(n, g.sun.xyz) > 0.0) * open;
    return g.sun_light.rgb * ggx * fh * lit / (4.0 * max(dot(n, view), 0.1));
}
