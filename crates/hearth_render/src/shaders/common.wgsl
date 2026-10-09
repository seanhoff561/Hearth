// Shared by the terrain and LOD shaders: per-frame globals, seasonal tints, the sky-view
// lookup, aerial perspective, planet curvature and the handoff between full-detail and LOD
// terrain.

// The camera's place reaches the shaders modulo WRAP blocks (reduced in f64: precise anywhere on
// the planet), and every planet's circumference is a multiple of it: a pattern fixed in the
// world repeats every WRAP blocks, or it would jump each time the camera crosses a multiple of it
// (E4.1 §4.5).
const WRAP: f32 = 4096.0;

// A frequency (cycles per block) snapped so WRAP blocks hold a whole number of its cycles; that
// number in y.
fn wrap_freq(f: f32) -> vec2<f32> {
    let cycles = max(round(WRAP * f), 1.0);
    return vec2<f32>(cycles / WRAP, cycles);
}

// An angular frequency (radians per block) snapped to whole turns over WRAP blocks.
fn wrap_rad(k: f32) -> f32 {
    return round(k * WRAP / TAU) * TAU / WRAP;
}

// A direction scaled to `f` cycles per block, snapped so WRAP blocks hold whole cycles along x and
// along z: a tiling pattern turned to it repeats every WRAP blocks.
fn wrap_dir(d: vec2<f32>, f: f32) -> vec2<f32> {
    return round(d * f * WRAP) / WRAP;
}

// Lighting is physically based and pre-exposed (lux × exposure): direct light from the sun (or
// the moon at night) by face orientation, diffuse sky light scaled by the voxel sky-light
// level, and firelight from the block-light level.
struct Globals {
    view_proj: mat4x4<f32>,
    // rgb: direct illuminance of the dominant light (sun or moon) on a facing surface.
    sun_light: vec4<f32>,
    // rgb: sky irradiance on an upward surface; a: floor ambient (starlight, airglow).
    sky_light: vec4<f32>,
    // rgb: firelight illuminance at block-light level 15; w: how wet the ground is from rain.
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
    // The sun's shadow maps (`shadow.rs`): each cascade's view-projection from camera-relative
    // points into the map as it was drawn.
    shadow_vp: array<mat4x4<f32>, 5>,
    // x: 1 when the maps hold the sun's shadows this frame; y: their size (texels); z: how far
    // toward the sun the far cascade takes its casters (blocks); w: 1 while a cascade is drawn.
    shadow: vec4<f32>,
    // Each cascade's texel (blocks): the four near ones, then the far one in the second's x.
    shadow_texel: array<vec4<f32>, 2>,
    // xyz: the direction toward the light the maps are drawn for.
    shadow_light: vec4<f32>,
    // How far toward the sun each near cascade takes its casters (blocks).
    shadow_reach: vec4<f32>,
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
// The sun's shadow maps, compared as they are read (lit where a point is no deeper from the sun
// than what the map holds).
@group(0) @binding(7) var shadow_map: texture_depth_2d_array;
@group(0) @binding(8) var shadow_samp: sampler_comparison;
// The maps' depths themselves, for finding what casts a shadow.
@group(0) @binding(9) var shadow_depth: sampler;

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

fn deciduous_tint(v: u32, variant: u32, yf: f32) -> vec3<f32> {
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
    if variant == 1u {
        // Yellow in autumn (birch).
        base = vec3<f32>(0.50, 0.65, 0.33);
        autumn = vec3<f32>(0.88, 0.72, 0.22);
    } else if variant == 2u {
        // Red in autumn (maples, cherry, hawthorn).
        autumn = vec3<f32>(0.80, 0.24, 0.10);
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
        case 2u: { return srgb_to_linear(deciduous_tint(v, (kind >> 2u) & 3u, yf)); }
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

// ---------------------------------------------------------------- the sun's shadows (R1a)
// Four cascades about the camera hold what the full-detail world casts, a fifth what the distant
// terrain beyond it casts (`shadow.rs`).

// A cascade's texel (blocks).
fn shadow_texel(i: u32) -> f32 {
    if i < 4u {
        return g.shadow_texel[0][i];
    }
    return g.shadow_texel[1].x;
}

// Where a camera-relative point falls in cascade `i`: its place in the map (0..1, y down) and its
// depth from the sun's side (0..1).
fn shadow_coord(i: u32, p: vec3<f32>) -> vec3<f32> {
    let c = g.shadow_vp[i] * vec4<f32>(p, 1.0);
    return vec3<f32>(c.x * 0.5 + 0.5, 0.5 - c.y * 0.5, c.z);
}

// How far inside a cascade's map a point lies: 1 a tenth of the way in or more, falling to 0 at
// its edge; negative outside it, and above or below its depth.
fn shadow_inside(s: vec3<f32>) -> f32 {
    if s.z <= 0.0 || s.z >= 1.0 {
        return -1.0;
    }
    let edge = min(min(s.x, 1.0 - s.x), min(s.y, 1.0 - s.y));
    return min(edge / 0.1, 1.0);
}

// Blocks of depth cascade `i`'s map spans: its width and how far it reaches toward the sun.
fn shadow_span(i: u32) -> f32 {
    return shadow_texel(i) * g.shadow.y + select(g.shadow.z, g.shadow_reach[i], i < 4u);
}

// How the depth of the surface through a point changes across cascade `i`'s map, per texel
// across (u, v), from its normal `n` (zero for plants: none): comparisons off the point are made
// against the surface's own plane there, not the point's depth, or a surface tilted from the sun
// would shade itself a few texels off.
fn shadow_slope(i: u32, n: vec3<f32>) -> vec2<f32> {
    if dot(n, n) < 0.5 {
        return vec2<f32>(0.0);
    }
    let back = g.shadow_light.xyz;
    let hint = select(vec3<f32>(0.0, 1.0, 0.0), vec3<f32>(0.0, 0.0, 1.0), abs(back.y) > 0.99);
    let right = normalize(cross(hint, back));
    let up = cross(back, right);
    // Grazing faces held to a slope of five.
    let facing = max(dot(n, back), 0.2);
    return vec2<f32>(dot(n, right), -dot(n, up)) / facing * shadow_texel(i) / shadow_span(i);
}

// Nine filtered comparisons `spread` texels apart about a point of cascade `i`, each against the
// surface's plane there (`slope`).
fn shadow_taps(i: u32, s: vec3<f32>, slope: vec2<f32>, spread: f32) -> f32 {
    let t = 1.0 / g.shadow.y;
    var lit = 0.0;
    for (var y = -1; y <= 1; y++) {
        for (var x = -1; x <= 1; x++) {
            let o = vec2<f32>(f32(x), f32(y)) * spread;
            let at = s.xy + o * t;
            lit += textureSampleCompareLevel(shadow_map, shadow_samp, at, i32(i), s.z + dot(slope, o));
        }
    }
    return lit / 9.0;
}

// The sun's angular radius (its disc is 0.53° across): a shadow's edge is as wide as twice this
// times the distance from what casts it, a centimetre for every metre.
const SUN_RADIUS: f32 = 0.00465;
// Blockers are looked for as far about a point as the penumbra of something this far above it
// reaches (blocks), and no penumbra is wider: softer would wash out the sharp shadows of things
// near the ground where a canopy far above casts too (the blockers' depths are averaged).
const PENUMBRA_REACH: f32 = 8.0;

// The share of the sun a cascade lets through about a point on a surface sloping across the map
// by `slope`, its edge as soft as the sun's disc makes it (percentage-closer soft shadows): where
// the map is fine enough to show it, the blockers about the point are found (five gathers of four
// texels), and sixteen comparisons spread over the penumbra their mean height above it gives; a
// texel or so soft otherwise.
fn shadow_pcf(i: u32, s: vec3<f32>, slope: vec2<f32>) -> f32 {
    let texel = shadow_texel(i);
    let search = SUN_RADIUS * PENUMBRA_REACH / texel;
    if search < 1.5 {
        return shadow_taps(i, s, slope, 1.0);
    }
    let t = 1.0 / g.shadow.y;
    // Half a texel's slope either way within a gather's four texels.
    let within = 0.5 * (abs(slope.x) + abs(slope.y));
    var sum = 0.0;
    var n = 0.0;
    for (var k = 0; k < 5; k++) {
        var o = vec2<f32>(0.0);
        if k > 0 {
            o = vec2<f32>(f32((k - 1) & 1) - 0.5, f32((k - 1) >> 1) - 0.5) * search;
        }
        let d = textureGather(shadow_map, shadow_depth, s.xy + o * t, i32(i));
        let plane = s.z + dot(slope, o) - within;
        for (var j = 0; j < 4; j++) {
            if d[j] < plane {
                sum += plane - d[j];
                n += 1.0;
            }
        }
    }
    if n < 0.5 {
        return 1.0;
    }
    if n > 19.5 {
        return 0.0;
    }
    let above = sum / n * shadow_span(i);
    let radius = clamp(SUN_RADIUS * above / texel, 1.0, search);
    // On a spiral over the penumbra, turned by a hash of the map's texel (a pattern fixed in the
    // world).
    let cell = floor(s.xy * g.shadow.y);
    let turn = fract(sin(dot(cell, vec2<f32>(12.9898, 78.233))) * 43758.5453) * TAU;
    var lit = 0.0;
    for (var k = 0; k < 16; k++) {
        let a = f32(k) * 2.39996 + turn;
        let o = vec2<f32>(cos(a), sin(a)) * sqrt((f32(k) + 0.5) / 16.0) * radius;
        let at = s.xy + o * t;
        lit += textureSampleCompareLevel(shadow_map, shadow_samp, at, i32(i), s.z + dot(slope, o));
    }
    return lit / 16.0;
}

// The sun's light reaching a camera-relative point on a surface facing `n` (zero for plants and
// such, lit from every side): 1 in the open, 0 in a cast shadow, between at a shadow's soft edge;
// -1 where no map holds the sun's shadows (shadows off, the sun down, the point beyond every
// cascade). The point is looked up a texel and a half off its surface (toward the sun for
// plants), so a surface does not shade itself. Its near cascade, blended into the next across the
// outer tenth of its map, and the far one: the darker. The far one holds the distant terrain,
// coarse; for a point of the full-detail world (`lod` false) only what lies beyond its near
// cascade's reach toward the sun counts there, the nearer casters being the near cascade's own.
fn sun_shadow_at(world: vec3<f32>, n: vec3<f32>, lod: bool) -> f32 {
    if g.shadow.x < 0.5 {
        return -1.0;
    }
    let l = g.shadow_light.xyz;
    let off = select(n, l, dot(n, n) < 0.5) * 1.5;
    var near = -1.0;
    var reach = 0.0;
    for (var i = 0u; i < 4u; i++) {
        let s = shadow_coord(i, world + off * shadow_texel(i));
        let inside = shadow_inside(s);
        if inside <= 0.0 {
            continue;
        }
        near = shadow_pcf(i, s, shadow_slope(i, n));
        reach = g.shadow_reach[i];
        if inside < 1.0 {
            if i == 3u {
                // The last near cascade gives way to the far one alone.
                near = mix(1.0, near, inside);
                reach *= inside;
            } else {
                let s2 = shadow_coord(i + 1u, world + off * shadow_texel(i + 1u));
                if shadow_inside(s2) > 0.0 {
                    near = mix(shadow_pcf(i + 1u, s2, shadow_slope(i + 1u, n)), near, inside);
                }
            }
        }
        break;
    }
    var far = -1.0;
    let skip = select(reach, 0.0, lod);
    let sf = shadow_coord(4u, world + off * shadow_texel(4u) + l * skip);
    let inside = shadow_inside(sf);
    if inside > 0.0 {
        far = mix(1.0, shadow_pcf(4u, sf, shadow_slope(4u, n)), inside);
    }
    if near < 0.0 {
        return far;
    }
    if far < 0.0 {
        return near;
    }
    return min(near, far);
}

fn sun_shadow(world: vec3<f32>, n: vec3<f32>) -> f32 {
    return sun_shadow_at(world, n, false);
}

// The share of the sun reaching a point whose sky-light level is `sky` (0..1): by the shadow maps
// where they hold it, else only where it is fully open to the sky; never deep in enclosed places,
// whose roofs may lie beyond the loaded world.
fn sun_open(world: vec3<f32>, n: vec3<f32>, sky: f32) -> f32 {
    let s = sun_shadow(world, n);
    if s >= 0.0 {
        return s * smoothstep(0.25, 0.55, sky);
    }
    return smoothstep(0.8, 1.0, sky);
}

// ---------------------------------------------------------------- blocks as drawn
// Shared by the terrain and the highlight's mask of a block (`outline.wgsl`), which must move
// and thin exactly as the block on screen does.

fn face_normal(face: u32) -> vec3<f32> {
    switch face {
        case 0u: { return vec3<f32>(0.0, -1.0, 0.0); }
        case 1u: { return vec3<f32>(0.0, 1.0, 0.0); }
        case 2u: { return vec3<f32>(0.0, 0.0, -1.0); }
        case 3u: { return vec3<f32>(0.0, 0.0, 1.0); }
        case 4u: { return vec3<f32>(-1.0, 0.0, 0.0); }
        case 5u: { return vec3<f32>(1.0, 0.0, 0.0); }
        default: { return vec3<f32>(0.0); }
    }
}

fn animated_layer(layer: u32, frames_m1: u32, frame_time_m1: u32) -> u32 {
    if frames_m1 == 0u {
        return layer;
    }
    let frame = u32(g.params.y / f32(frame_time_m1 + 1u)) % (frames_m1 + 1u);
    return layer + frame;
}

fn wind(world: vec3<f32>, amount: f32) -> vec3<f32> {
    let t = g.params.x;
    let p = world + g.camera.xyz;
    // Whole turns over WRAP blocks (the sway holds as the camera crosses a multiple of it).
    let s = sin(t * 1.7 + p.x * wrap_rad(0.35) + p.z * wrap_rad(0.21))
        + 0.5 * sin(t * 2.9 + p.z * wrap_rad(0.5));
    return vec3<f32>(s, 0.0, s * 0.6) * 0.04 * amount * g.sun.w;
}

// Integer hash of a texel position (texels of 1/16 block) to 0..1, the texel's place wrapped at
// WRAP blocks (65536 texels: the pattern holds as the camera crosses a multiple of it).
fn hash_texel(p: vec3<f32>) -> f32 {
    let q = vec3<u32>(bitcast<u32>(i32(p.x)), bitcast<u32>(i32(p.y)), bitcast<u32>(i32(p.z)))
        & vec3<u32>(0xffffu);
    var h = (q.x * 0x8da6b343u) ^ (q.y * 0xd8163841u) ^ (q.z * 0xcb1ab31fu);
    h = (h ^ (h >> 16u)) * 0x7feb352du;
    h = (h ^ (h >> 15u)) * 0x846ca68bu;
    h = h ^ (h >> 16u);
    return f32(h) / 4294967296.0;
}

// Leaf fall on a texel of foliage drawn at `world` (camera-relative, as drawn) facing `normal`,
// its cover `leaf` (1 in full leaf): 0 the leaf stays; 1 a twig shows instead (a sparse network
// among the fallen); 2 nothing. The share that stays is a pattern fixed in the world.
fn leaf_fall(world: vec3<f32>, normal: vec3<f32>, leaf: f32) -> u32 {
    if leaf >= 0.999 {
        return 0u;
    }
    let texel = floor((world + g.camera.xyz - normal * 0.03) * 16.0);
    if hash_texel(texel) < leaf {
        return 0u;
    }
    if hash_texel(texel + vec3<f32>(19.0, 7.0, 3.0)) > 0.16 {
        return 2u;
    }
    return 1u;
}
