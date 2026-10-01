// Shared by the terrain and LOD shaders: per-frame globals, seasonal tints, the sky-view
// lookup, aerial perspective, planet curvature and the handoff between full-detail and LOD
// terrain.

// Lighting is physically based and pre-exposed (lux × exposure): direct light from the sun (or
// the moon at night) by face orientation, diffuse sky light scaled by the voxel sky-light
// level, and firelight from the block-light level.
struct Globals {
    view_proj: mat4x4<f32>,
    // rgb: direct illuminance of the dominant light (sun or moon) on a facing surface.
    sun_light: vec4<f32>,
    // rgb: sky irradiance on an upward surface; a: floor ambient (starlight, airglow).
    sky_light: vec4<f32>,
    // rgb: firelight illuminance at block-light level 15.
    block_light: vec4<f32>,
    // Aerial perspective. x: aerosol extinction at sea level per metre (haze and humidity),
    // y: grey extinction per metre of falling rain or snow, z: real metres per vertical block
    // (1 / vertical scale), w: camera altitude in real metres.
    fog: vec4<f32>,
    // x: seconds, y: animation ticks, z: planet curvature 1 / (2 R) per block, w: 1 when aerial
    // perspective is on.
    params: vec4<f32>,
    // xyz: direction to the dominant light; w: wind strength.
    sun: vec4<f32>,
    // xyz: camera world position modulo 4096 (wind phase); w: year fraction (seasons).
    camera: vec4<f32>,
    // rgb: grey of an overcast sky; w: how far haze and fog take it instead of the clear sky.
    overcast: vec4<f32>,
    // Full-detail terrain, camera-relative: min x, min z, max x, max z. The LOD takes over
    // beyond it through a dithered band.
    near: vec4<f32>,
    // The map of water surfaces around the camera (`water.rs`): xy its first column relative to
    // the camera, z the camera's height (world Y), w 1 when there is a map.
    water_map: vec4<f32>,
};

@group(0) @binding(0) var<uniform> g: Globals;
@group(0) @binding(1) var tex: texture_2d_array<f32>;
@group(0) @binding(2) var samp: sampler;
@group(0) @binding(3) var skyview: texture_2d<f32>;
@group(0) @binding(4) var sky_samp: sampler;
// The water surface (world Y) of each column around the camera, and the caustics the waves
// focus on floors below them (value / 4, mean 1 / 4).
@group(0) @binding(5) var water_heights: texture_2d<f32>;
@group(0) @binding(6) var caustics: texture_2d<f32>;

const TAU: f32 = 6.2831853;

// ---------------------------------------------------------------- seasonal tints
// Climate codes (24 bits) as written by `hearth_env::tint::encode`.
struct Climate {
    t_mean: f32,
    t_range: f32,
    precip: f32,
    dry: u32,
    south: bool,
};

fn decode_climate(v: u32) -> Climate {
    var c: Climate;
    c.t_mean = f32(v & 255u) / 2.5 - 50.0;
    c.t_range = f32((v >> 8u) & 31u) * 2.0;
    let p = f32((v >> 13u) & 255u) / 255.0;
    c.precip = p * p * 6000.0;
    c.dry = (v >> 21u) & 3u;
    c.south = ((v >> 23u) & 1u) == 1u;
    return c;
}

fn local_year(c: Climate, yf: f32) -> f32 {
    return select(yf, fract(yf + 0.5), c.south);
}

fn season_temp(c: Climate, yf: f32) -> f32 {
    let lag = 0.07 + 0.05 * (1.0 - smoothstep(8.0, 25.0, c.t_range));
    return c.t_mean + 0.5 * c.t_range * cos(TAU * (local_year(c, yf) - 0.25 - lag));
}

fn dry_season(c: Climate, yf: f32) -> bool {
    let summer = cos(TAU * (local_year(c, yf) - 0.3));
    return (c.dry == 1u && summer < -0.52) || (c.dry == 2u && summer > 0.78);
}

fn summer_color(c: Climate, foliage: bool) -> vec3<f32> {
    let warmth = clamp((c.t_mean + 10.0) / 38.0, 0.0, 1.0);
    let wet = clamp(c.precip / 2200.0, 0.0, 1.0);
    var cd = vec3<f32>(128.0, 180.0, 151.0);
    var cw = vec3<f32>(100.0, 170.0, 120.0);
    var hd = vec3<f32>(191.0, 183.0, 85.0);
    var hw = vec3<f32>(71.0, 205.0, 51.0);
    if foliage {
        cd = vec3<f32>(96.0, 161.0, 123.0);
        cw = vec3<f32>(72.0, 140.0, 110.0);
        hd = vec3<f32>(174.0, 164.0, 42.0);
        hw = vec3<f32>(26.0, 160.0, 24.0);
    }
    return mix(mix(cd, cw, wet), mix(hd, hw, wet), warmth) / 255.0;
}

const CURED: vec3<f32> = vec3<f32>(0.78, 0.68, 0.42);
const DEAD_LEAVES: vec3<f32> = vec3<f32>(0.47, 0.35, 0.22);

fn grass_tint(v: u32, yf: f32) -> vec3<f32> {
    let c = decode_climate(v);
    let t = season_temp(c, yf);
    var green = smoothstep(2.0, 8.0, t) * select(1.0, 0.15, dry_season(c, yf));
    if c.dry == 3u {
        green *= 0.35;
    }
    return mix(CURED, summer_color(c, false), green);
}

// Leaf cover of deciduous trees 0..1, as `hearth_env::phenology`: leaves come out as spring
// passes ~8 °C and fall around 5 °C in autumn. Trees of winter-dry (savanna, monsoon) climates
// drop most leaves in the dry season; mediterranean trees keep theirs through the summer drought.
fn leaf_cover(v: u32, yf: f32) -> f32 {
    let c = decode_climate(v);
    let t = season_temp(c, yf);
    var leaf = smoothstep(3.0, 7.0, t);
    if season_temp(c, yf + 0.02) > t {
        leaf = smoothstep(6.0, 10.0, t);
    }
    if c.dry == 1u && dry_season(c, yf) {
        leaf *= 0.35;
    }
    return leaf;
}

fn deciduous_tint(v: u32, birch: bool, yf: f32) -> vec3<f32> {
    let c = decode_climate(v);
    let t = season_temp(c, yf);
    let rising = season_temp(c, yf + 0.02) > t;
    let leaf = leaf_cover(v, yf);
    var green = smoothstep(5.0, 13.0, t);
    if rising {
        green = 1.0;
    }
    var base = summer_color(c, true);
    var autumn = vec3<f32>(0.70, 0.45, 0.16);
    if birch {
        base = vec3<f32>(0.50, 0.65, 0.33);
        autumn = vec3<f32>(0.88, 0.72, 0.22);
    }
    return mix(DEAD_LEAVES, mix(autumn, base, green), sqrt(clamp(leaf, 0.0, 1.0)));
}

fn evergreen_tint(v: u32, yf: f32) -> vec3<f32> {
    let c = decode_climate(v);
    let t = season_temp(c, yf);
    let greenness = 0.75 + 0.25 * smoothstep(-15.0, 5.0, t);
    return mix(vec3<f32>(0.30, 0.38, 0.28), vec3<f32>(0.38, 0.60, 0.38), greenness);
}

// Leaf cover for a tint kind: seasonal for deciduous foliage, 1 otherwise.
fn tint_leaf(kind: u32, v: u32) -> f32 {
    if (kind & 3u) == 2u {
        return leaf_cover(v, g.camera.w);
    }
    return 1.0;
}

// Tint kind (low 2 bits) with variant (next 2 bits) and a 24-bit value.
fn resolve_tint(kind: u32, v: u32) -> vec3<f32> {
    let yf = g.camera.w;
    switch kind & 3u {
        case 1u: { return srgb_to_linear(grass_tint(v, yf)); }
        case 2u: { return srgb_to_linear(deciduous_tint(v, (kind >> 2u) == 1u, yf)); }
        case 3u: { return srgb_to_linear(evergreen_tint(v, yf)); }
        default: { return unpack_rgb(v); }
    }
}

fn srgb_to_linear(c: vec3<f32>) -> vec3<f32> {
    return pow(c, vec3<f32>(2.2));
}

fn unpack_rgb(v: u32) -> vec3<f32> {
    return srgb_to_linear(vec3<f32>(f32(v & 255u), f32((v >> 8u) & 255u), f32((v >> 16u) & 255u)) / 255.0);
}

// Sky-view lookup at or above the horizon (below it, the horizon row).
fn skyview_uv(dir: vec3<f32>) -> vec2<f32> {
    let az = atan2(dir.x, -dir.z);
    let u = fract(az / TAU + 1.0);
    let el = asin(clamp(dir.y, 0.0, 1.0));
    let t = el / (TAU / 4.0);
    return vec2<f32>(u, max(0.5 + 0.5 * sqrt(t), 0.5 + 1.0 / 128.0));
}

// Rayleigh scattering of air at sea level (per metre, RGB) and the scale heights of air and
// aerosol (metres), as in `atmosphere.wgsl` and `hearth_env::sky`.
const RAYLEIGH: vec3<f32> = vec3<f32>(5.802e-6, 13.558e-6, 33.1e-6);
const RAYLEIGH_H: f32 = 8000.0;
const MIE_H: f32 = 1200.0;

// Air mass along a straight path of `len` metres from altitude h0 to h1 in an exponential
// atmosphere of scale height `sh`: the integral of exp(-h / sh) over the path.
fn air_mass(sh: f32, h0: f32, h1: f32, len: f32) -> f32 {
    let a = exp(-max(h0, 0.0) / sh);
    let dh = h1 - h0;
    if abs(dh) < 1.0 {
        return a * len;
    }
    let b = exp(-max(h1, 0.0) / sh);
    return len * sh * (a - b) / dh;
}

// Aerial perspective: the light of a surface `world` (camera-relative, blocks) dimmed by the
// air on the way and replaced by the light the air scatters toward the eye, which tends to the
// sky's colour in that direction (or the grey of a cloud deck and falling rain or snow).
// Horizontal distances are metres, heights blocks of `fog.z` metres; the air thins with real
// altitude. Nothing depends on how far the terrain is loaded.
fn aerial(c: vec3<f32>, world: vec3<f32>) -> vec3<f32> {
    if g.params.w < 0.5 {
        return c;
    }
    let rise = world.y * g.fog.z;
    let len = sqrt(dot(world.xz, world.xz) + rise * rise);
    let h0 = g.fog.w;
    let h1 = h0 + rise;
    let tau = RAYLEIGH * air_mass(RAYLEIGH_H, h0, h1, len)
        + vec3<f32>(g.fog.x * air_mass(MIE_H, h0, h1, len) + g.fog.y * len);
    let t = exp(-tau);
    let view = world / max(length(world), 1e-3);
    let clear = textureSampleLevel(skyview, sky_samp, skyview_uv(view), 0.0).rgb;
    let sky = mix(clear, g.overcast.rgb, g.overcast.w);
    return c * t + sky * (1.0 - t);
}

// Planet curvature: the ground drops by d² / 2R away from the camera (R is Earth's radius times
// the vertical scale, so horizons match the real view from the equivalent altitude).
fn curve(world: vec3<f32>) -> vec3<f32> {
    return world - vec3<f32>(0.0, dot(world.xz, world.xz) * g.params.z, 0.0);
}

// Width of the band where full-detail terrain hands over to the LOD (blocks).
const HANDOFF_BAND: f32 = 8.0;

// How far into the full-detail area a point lies: 1 well inside, 0 outside, a ramp across the
// band at its edge.
fn near_weight(p: vec2<f32>) -> f32 {
    let d = min(min(p.x - g.near.x, g.near.z - p.x), min(p.y - g.near.y, g.near.w - p.y));
    return clamp(d / HANDOFF_BAND, 0.0, 1.0);
}

// Ordered 4×4 dither threshold of a pixel (0..1): across the band the full-detail terrain keeps
// the pixels whose threshold is below its weight, and the LOD behind it fills the rest.
fn dither(frag: vec2<f32>) -> f32 {
    let m = array<f32, 16>(0.0, 8.0, 2.0, 10.0, 12.0, 4.0, 14.0, 6.0, 3.0, 11.0, 1.0, 9.0, 15.0, 7.0, 13.0, 5.0);
    let i = u32(frag.x) % 4u + (u32(frag.y) % 4u) * 4u;
    return (m[i] + 0.5) / 16.0;
}
