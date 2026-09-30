// Terrain rendering with vertex pulling.
//
// Two quad formats share this shader:
//  * packed quads (16 bytes): full-cube faces, greedily merged;
//  * general quads (64 bytes): models, fluids and translucent surfaces.
// Positions are camera-relative: instance origins are (cube min − camera) in f32.

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
    // x: aerial extinction per metre, y: haze, z,w: unused.
    fog: vec4<f32>,
    // x: seconds, y: animation ticks, z: render-edge fog start, w: fog end.
    params: vec4<f32>,
    // xyz: direction to the dominant light; w: wind strength.
    sun: vec4<f32>,
    // xyz: camera world position modulo 4096 (wind phase); w: year fraction (seasons).
    camera: vec4<f32>,
    // rgb: grey of an overcast sky; w: how far haze and fog take it instead of the clear sky.
    overcast: vec4<f32>,
};

@group(0) @binding(0) var<uniform> g: Globals;
@group(0) @binding(1) var tex: texture_2d_array<f32>;
@group(0) @binding(2) var samp: sampler;
@group(0) @binding(3) var skyview: texture_2d<f32>;
@group(0) @binding(4) var sky_samp: sampler;

struct PackedQuad { a: u32, b: u32, c: u32, d: u32 };
// 64 bytes, matching the Rust layout: four corners of three words each (a vec3<u32> member
// would be 16-byte aligned in WGSL and break the stride).
struct GeneralQuad {
    c: array<u32, 12>,
    layer: u32, tint: u32, overlay: u32, pad: u32,
};
struct Instance { origin: vec4<f32> };

@group(1) @binding(0) var<storage, read> quads: array<PackedQuad>;
@group(1) @binding(1) var<storage, read> gquads: array<GeneralQuad>;
@group(1) @binding(2) var<storage, read> instances: array<Instance>;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) @interpolate(flat) layers: vec2<u32>,
    @location(2) tint: vec3<f32>,
    @location(3) light: vec2<f32>,
    @location(4) ao: f32,
    // Face normal (zero for plants and other double-sided models).
    @location(5) normal: vec3<f32>,
    // Camera-relative position.
    @location(6) world: vec3<f32>,
    @location(7) @interpolate(flat) flags: u32,
    // Leaf cover of deciduous foliage for the date (1 for everything else).
    @location(8) @interpolate(flat) leaf: f32,
};

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

// Corner of a (w × h) face of block (0,0,0), corner order shared with the mesher.
fn face_corner(face: u32, c: u32, w: f32, h: f32) -> vec3<f32> {
    switch face {
        case 0u: { // down
            let t = array<vec3<f32>, 4>(vec3(0.0, 0.0, 0.0), vec3(w, 0.0, 0.0), vec3(w, 0.0, h), vec3(0.0, 0.0, h));
            return t[c];
        }
        case 1u: { // up
            let t = array<vec3<f32>, 4>(vec3(0.0, 1.0, h), vec3(w, 1.0, h), vec3(w, 1.0, 0.0), vec3(0.0, 1.0, 0.0));
            return t[c];
        }
        case 2u: { // north (-z)
            let t = array<vec3<f32>, 4>(vec3(w, 0.0, 0.0), vec3(0.0, 0.0, 0.0), vec3(0.0, h, 0.0), vec3(w, h, 0.0));
            return t[c];
        }
        case 3u: { // south (+z)
            let t = array<vec3<f32>, 4>(vec3(0.0, 0.0, 1.0), vec3(w, 0.0, 1.0), vec3(w, h, 1.0), vec3(0.0, h, 1.0));
            return t[c];
        }
        case 4u: { // west (-x)
            let t = array<vec3<f32>, 4>(vec3(0.0, 0.0, 0.0), vec3(0.0, 0.0, w), vec3(0.0, h, w), vec3(0.0, h, 0.0));
            return t[c];
        }
        default: { // east (+x)
            let t = array<vec3<f32>, 4>(vec3(1.0, 0.0, w), vec3(1.0, 0.0, 0.0), vec3(1.0, h, 0.0), vec3(1.0, h, w));
            return t[c];
        }
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
    let s = sin(t * 1.7 + p.x * 0.35 + p.z * 0.21) + 0.5 * sin(t * 2.9 + p.z * 0.5);
    return vec3<f32>(s, 0.0, s * 0.6) * 0.04 * amount * g.sun.w;
}

@vertex
fn vs_packed(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> VsOut {
    let q = quads[vi / 4u];
    let flip = (q.a >> 23u) & 1u;
    let c = ((vi % 4u) + flip) % 4u;
    let bx = f32(q.a & 15u);
    let by = f32((q.a >> 4u) & 15u);
    let bz = f32((q.a >> 8u) & 15u);
    let w = f32(((q.a >> 12u) & 15u) + 1u);
    let h = f32(((q.a >> 16u) & 15u) + 1u);
    let face = (q.a >> 20u) & 7u;
    var local = vec3<f32>(bx, by, bz) + face_corner(face, c, w, h);
    let fluid = (q.a >> 24u) & 1u;
    if fluid == 1u && face == 1u {
        local.y -= 0.111;
    }
    let origin = instances[ii].origin.xyz;
    var world = origin + local;
    if ((q.a >> 25u) & 1u) == 1u {
        world += wind(world, 1.0);
    }
    var out: VsOut;
    out.pos = g.view_proj * vec4<f32>(world, 1.0);
    // UVs in tiles: corners (0,h),(w,h),(w,0),(0,0), rotated in quarter turns.
    let uvs = array<vec2<f32>, 4>(vec2(0.0, h), vec2(w, h), vec2(w, 0.0), vec2(0.0, 0.0));
    var uv = uvs[c];
    let rot = (q.a >> 26u) & 3u;
    if rot == 1u { uv = vec2(uv.y, -uv.x); }
    if rot == 2u { uv = -uv; }
    if rot == 3u { uv = vec2(-uv.y, uv.x); }
    out.uv = uv;
    let frames_m1 = (q.a >> 28u) & 15u;
    let frame_time_m1 = (q.b >> 24u) & 15u;
    let base = animated_layer(q.b & 4095u, frames_m1, frame_time_m1);
    out.layers = vec2<u32>(base, (q.b >> 12u) & 4095u);
    out.tint = resolve_tint((q.b >> 28u) & 15u, q.d & 0xffffffu);
    out.leaf = tint_leaf((q.b >> 28u) & 15u, q.d & 0xffffffu);
    let l = (q.c >> (c * 8u)) & 255u;
    out.light = vec2<f32>(f32(l >> 4u), f32(l & 15u)) / 15.0;
    out.ao = f32((q.d >> (24u + c * 2u)) & 3u) / 3.0;
    out.normal = face_normal(face);
    out.world = world;
    out.flags = 0u;
    return out;
}

fn gcorner(q: GeneralQuad, c: u32) -> vec3<u32> {
    return vec3<u32>(q.c[c * 3u], q.c[c * 3u + 1u], q.c[c * 3u + 2u]);
}

fn s16(v: u32) -> f32 {
    return f32(bitcast<i32>(v << 16u) >> 16u) / 256.0;
}

@vertex
fn vs_general(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> VsOut {
    let q = gquads[vi / 4u];
    let c = vi % 4u;
    let e = gcorner(q, c);
    let local = vec3<f32>(s16(e.x & 0xffffu), s16(e.x >> 16u), s16(e.y & 0xffffu));
    let origin = instances[ii].origin.xyz;
    var world = origin + local;
    let waving = (q.layer >> 23u) & 1u;
    if waving == 1u {
        // Only the upper corners of plants move.
        let up = select(0.0, 1.0, fract(local.y) > 0.5 || c >= 2u);
        world += wind(world, up);
    }
    let fluid = (q.layer >> 24u) & 1u;
    var out: VsOut;
    out.pos = g.view_proj * vec4<f32>(world, 1.0);
    out.uv = vec2<f32>(f32(e.z & 0xffffu), f32(e.z >> 16u)) / 256.0 / 16.0;
    let frames_m1 = (q.layer >> 12u) & 15u;
    let frame_time_m1 = (q.layer >> 16u) & 15u;
    out.layers = vec2<u32>(animated_layer(q.layer & 4095u, frames_m1, frame_time_m1), q.overlay & 4095u);
    out.tint = resolve_tint((q.tint >> 24u) & 15u, q.tint & 0xffffffu);
    out.leaf = tint_leaf((q.tint >> 24u) & 15u, q.tint & 0xffffffu);
    let l = (e.y >> 16u) & 255u;
    out.light = vec2<f32>(f32(l >> 4u), f32(l & 15u)) / 15.0;
    out.ao = f32((e.y >> 24u) & 3u) / 3.0;
    out.normal = face_normal((q.layer >> 20u) & 7u);
    out.world = world;
    out.flags = fluid;
    return out;
}

// Crisp texels up close, smooth filtering at distance ("pixel-art anti-aliasing").
fn sample_sharp(uv: vec2<f32>, layer: u32) -> vec4<f32> {
    let size = 16.0;
    let texel = uv * size;
    let seam = floor(texel + 0.5);
    let d = max(fwidth(texel), vec2<f32>(1e-4));
    let sharp = (seam + clamp((texel - seam) / d, vec2<f32>(-0.5), vec2<f32>(0.5))) / size;
    return textureSampleGrad(tex, samp, sharp, layer, dpdx(uv), dpdy(uv));
}

fn light_curve(l: f32) -> f32 {
    // Level 15 → 1, falling off steeply like light spreading through openings.
    return l / (4.0 - 3.0 * l);
}

// Sky-view lookup at or above the horizon (below it, the horizon row).
fn skyview_uv(dir: vec3<f32>) -> vec2<f32> {
    let az = atan2(dir.x, -dir.z);
    let u = fract(az / TAU + 1.0);
    let el = asin(clamp(dir.y, 0.0, 1.0));
    let t = el / (TAU / 4.0);
    return vec2<f32>(u, max(0.5 + 0.5 * sqrt(t), 0.5 + 1.0 / 128.0));
}

fn shade_color(albedo: vec3<f32>, in: VsOut) -> vec3<f32> {
    let sky_vis = light_curve(in.light.x);
    let n = in.normal;
    let omni = dot(n, n) < 0.5;
    // Sky light on a face: full on top, less on the sides, least underneath.
    let sky_dir = select(0.62 + 0.38 * n.y + 0.1 * (1.0 - abs(n.y)), 0.75, omni);
    let ambient = g.sky_light.rgb * sky_vis * max(sky_dir, 0.2) + vec3<f32>(g.sky_light.a);
    // Direct light only on faces fully open to the sky (no shadow maps yet).
    let open = smoothstep(0.8, 1.0, in.light.x);
    let lambert = select(max(dot(n, g.sun.xyz), 0.0), 0.5 * max(g.sun.y, 0.0) + 0.25, omni);
    let direct = g.sun_light.rgb * lambert * open;
    let fire = g.block_light.rgb * light_curve(in.light.y);
    let ao = mix(0.45, 1.0, in.ao);
    var c = albedo * (ambient * ao + direct * mix(0.75, 1.0, in.ao) + fire * ao) / 3.14159265;
    // Aerial perspective: extinction over distance toward the sky's colour in that direction.
    let dist = length(in.world);
    let view = in.world / max(dist, 1e-3);
    // The colour distance fades to: the clear sky in that direction, or the grey of cloud and
    // falling rain or snow.
    let clear = textureSampleLevel(skyview, sky_samp, skyview_uv(view), 0.0).rgb;
    let sky = mix(clear, g.overcast.rgb, g.overcast.w);
    let aerial = 1.0 - exp(-g.fog.x * dist);
    c = mix(c, sky, aerial);
    // Render-distance edge: blend into the sky near the horizon of the loaded area.
    let edge = clamp((dist - g.params.z) / max(g.params.w - g.params.z, 1.0), 0.0, 1.0);
    c = mix(c, sky, edge * edge);
    return c;
}

fn surface_color(in: VsOut) -> vec4<f32> {
    var base = sample_sharp(in.uv, in.layers.x);
    var tinted = base.rgb;
    if in.layers.y == 4095u {
        tinted = base.rgb * in.tint;
    } else {
        // Base untinted, overlay tinted on top.
        let o = sample_sharp(in.uv, in.layers.y);
        tinted = mix(base.rgb, o.rgb * in.tint, o.a);
    }
    return vec4<f32>(tinted, base.a);
}

@fragment
fn fs_opaque(in: VsOut) -> @location(0) vec4<f32> {
    let c = surface_color(in);
    return vec4<f32>(shade_color(c.rgb, in), 1.0);
}

// Integer hash of a texel position to 0..1.
fn hash_texel(p: vec3<f32>) -> f32 {
    let q = vec3<u32>(bitcast<u32>(i32(p.x)), bitcast<u32>(i32(p.y)), bitcast<u32>(i32(p.z)));
    var h = (q.x * 0x8da6b343u) ^ (q.y * 0xd8163841u) ^ (q.z * 0xcb1ab31fu);
    h = (h ^ (h >> 16u)) * 0x7feb352du;
    h = (h ^ (h >> 15u)) * 0x846ca68bu;
    h = h ^ (h >> 16u);
    return f32(h) / 4294967296.0;
}

// Bare twigs (linear albedo).
const TWIG: vec3<f32> = vec3<f32>(0.085, 0.07, 0.058);

@fragment
fn fs_cutout(in: VsOut) -> @location(0) vec4<f32> {
    let c = surface_color(in);
    if c.a < 0.5 {
        discard;
    }
    var albedo = c.rgb;
    if in.leaf < 0.999 {
        // Leaf fall: this share of the leaf texels stays (a pattern fixed in the world), and a
        // sparse network of twigs shows among the rest.
        let texel = floor((in.world + g.camera.xyz - in.normal * 0.03) * 16.0);
        if hash_texel(texel) >= in.leaf {
            if hash_texel(texel + vec3<f32>(19.0, 7.0, 3.0)) > 0.16 {
                discard;
            }
            albedo = TWIG;
        }
    }
    return vec4<f32>(shade_color(albedo, in), 1.0);
}

@fragment
fn fs_translucent(in: VsOut) -> @location(0) vec4<f32> {
    let c = surface_color(in);
    var alpha = c.a;
    if in.flags == 1u {
        // Water: slightly more opaque with distance so far oceans read solid.
        alpha = clamp(c.a + length(in.world) * 0.002, 0.55, 0.9);
    }
    return vec4<f32>(shade_color(c.rgb, in), alpha);
}
