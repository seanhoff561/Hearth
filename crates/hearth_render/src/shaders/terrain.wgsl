// Terrain rendering with vertex pulling (`common.wgsl` is prepended).
//
// Two quad formats share this shader:
//  * packed quads (16 bytes): full-cube faces, greedily merged;
//  * general quads (64 bytes): models, fluids and translucent surfaces.
// Positions are camera-relative: instance origins are (cube min − camera) in f32.

struct PackedQuad { a: u32, b: u32, c: u32, d: u32 };
// 64 bytes, matching the Rust layout: four corners of three words each (a vec3<u32> member
// would be 16-byte aligned in WGSL and break the stride).
struct GeneralQuad {
    c: array<u32, 12>,
    layer: u32, tint: u32, overlay: u32, pad: u32,
};
struct Instance { origin: vec4<f32> };
// The smooth ground's vertex (24 bytes, `smooth::SmoothVertex`): position (u16 × 3, (m + 1) ×
// 2048 in the cube), octahedral normal (i8 × 2), four material slots and their weights (u8 ×
// 4 each), light, AO, sharpness, and the column's climate code.
struct SmoothV { a: u32, b: u32, c: u32, d: u32, e: u32, f: u32 };
// A ground material (`terrain::GroundMaterial`).
struct GroundMat { color: vec4<f32>, color2: vec4<f32>, tint: u32, relief: f32, pad0: f32, pad1: f32 };

@group(1) @binding(0) var<storage, read> quads: array<PackedQuad>;
@group(1) @binding(1) var<storage, read> gquads: array<GeneralQuad>;
@group(1) @binding(2) var<storage, read> instances: array<Instance>;
@group(1) @binding(3) var<storage, read> svert: array<SmoothV>;
@group(1) @binding(4) var<storage, read> gmats: array<GroundMat>;

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
    // How far below the water surface the point lies (blocks; negative above water), from the
    // map of the surfaces around the camera.
    @location(9) water_depth: f32,
    // The smooth ground: its four material slots (one byte each, the triangle's), their
    // weights, the column's climate and how crisp the surface is.
    @location(10) @interpolate(flat) mats: u32,
    @location(11) weights: vec4<f32>,
    @location(12) @interpolate(flat) climate: u32,
    @location(13) sharp: f32,
};

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

// How far below the water surface a camera-relative point lies (blocks), from the map of the
// surfaces around the camera; negative above water and outside the map.
fn depth_under_water(world: vec3<f32>) -> f32 {
    if g.water_map.w < 0.5 {
        return -1.0;
    }
    let c = vec2<i32>(floor(world.xz - g.water_map.xy));
    if c.x < 0 || c.y < 0 || c.x >= 256 || c.y >= 256 {
        return -1.0;
    }
    return textureLoad(water_heights, c, 0).r - (world.y + g.water_map.z);
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
    let water_depth = depth_under_water(world);
    world = curve(world);
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
    out.water_depth = water_depth;
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
    let water_depth = depth_under_water(world);
    world = curve(world);
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
    out.water_depth = water_depth;
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


// The sun's light on a floor `depth_m` metres under the waves, relative to its mean: two
// drifting layers of the caustic map, smoothing out with depth; the mip level from the distance
// (about 0.05 texels per block of distance at 1080p).
fn caustic_light(p: vec2<f32>, dist: f32, depth_m: f32) -> f32 {
    let t = g.params.x;
    let lod = log2(max(dist * 0.05, 1.0));
    let a = textureSampleLevel(caustics, samp, p / 7.0 + vec2<f32>(t * 0.031, t * 0.017), lod).r * 4.0;
    let q = vec2<f32>(-p.y, p.x);
    let b = textureSampleLevel(caustics, samp, q / 5.3 + vec2<f32>(-t * 0.023, t * 0.029), lod).r * 4.0;
    return mix(1.0, a * b, exp(-depth_m / 6.0));
}

fn shade_color(albedo: vec3<f32>, in: VsOut) -> vec3<f32> {
    let sky_vis = light_curve(in.light.x);
    let n = in.normal;
    let omni = dot(n, n) < 0.5;
    // Sky light on a face: full on top, less on the sides, least underneath.
    let sky_dir = select(0.62 + 0.38 * n.y + 0.1 * (1.0 - abs(n.y)), 0.75, omni);
    var ambient = g.sky_light.rgb * sky_vis * max(sky_dir, 0.2) + vec3<f32>(g.sky_light.a);
    // Direct light only on faces fully open to the sky (no shadow maps yet).
    let open = smoothstep(0.8, 1.0, in.light.x);
    let lambert = select(max(dot(n, g.sun.xyz), 0.0), 0.5 * max(g.sun.y, 0.0) + 0.25, omni);
    var direct = g.sun_light.rgb * lambert * open;
    let depth = in.water_depth;
    if depth > 0.0 && in.light.x > 0.0 {
        // Under water open to the sky: the light that reaches this depth, falling off
        // exponentially (red first, blue deepest), the sun's refracted toward the vertical and
        // gathered into caustics by the waves above.
        let depth_m = depth * g.fog.z;
        let reach = exp(-vec3<f32>(0.35, 0.07, 0.045) * depth_m);
        ambient = (g.sky_light.rgb * max(sky_dir, 0.2) + vec3<f32>(g.sky_light.a)) * reach;
        let h = g.sun.xz / 1.333;
        let sun_w = vec3<f32>(h.x, sqrt(max(1.0 - dot(h, h), 0.0)), h.y);
        let lam = select(max(dot(n, sun_w), 0.0), 0.5 * sun_w.y + 0.25, omni);
        let p = in.world.xz + g.camera.xz;
        direct = g.sun_light.rgb * lam * reach * select(0.0, 1.0, g.sun.y > 0.0)
            * caustic_light(p, length(in.world), depth_m);
    }
    let fire = g.block_light.rgb * light_curve(in.light.y);
    let ao = mix(0.45, 1.0, in.ao);
    var c = albedo * (ambient * ao + direct * mix(0.75, 1.0, in.ao) + fire * ao) / 3.14159265;
    return aerial(c, in.world);
}

// Full-detail terrain gives way to the LOD across the band at the edge of the loaded area.
fn handoff(in: VsOut) {
    if near_weight(in.world.xz) <= dither(in.pos.xy) {
        discard;
    }
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
    handoff(in);
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
    handoff(in);
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

// ---------------------------------------------------------------- the smooth ground (Amendment S)

fn oct_decode(e: vec2<f32>) -> vec3<f32> {
    var n = vec3<f32>(e.x, 1.0 - abs(e.x) - abs(e.y), e.y);
    if n.y < 0.0 {
        let x = (1.0 - abs(n.z)) * select(-1.0, 1.0, n.x >= 0.0);
        let z = (1.0 - abs(n.x)) * select(-1.0, 1.0, n.z >= 0.0);
        n = vec3<f32>(x, n.y, z);
    }
    return normalize(n);
}

fn s8(v: u32) -> f32 {
    return f32(bitcast<i32>(v << 24u) >> 24u) / 127.0;
}

@vertex
fn vs_smooth(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> VsOut {
    let v = svert[vi];
    let local = vec3<f32>(f32(v.a & 0xffffu), f32(v.a >> 16u), f32(v.b & 0xffffu)) / 2048.0 - 1.0;
    let origin = instances[ii].origin.xyz;
    var world = origin + local;
    let water_depth = depth_under_water(world);
    world = curve(world);
    var out: VsOut;
    out.pos = g.view_proj * vec4<f32>(world, 1.0);
    out.normal = oct_decode(vec2<f32>(s8(v.b >> 16u), s8(v.b >> 24u)));
    out.mats = v.c;
    out.weights = unpack4x8unorm(v.d);
    let l = v.e & 255u;
    out.light = vec2<f32>(f32(l >> 4u), f32(l & 15u)) / 15.0;
    out.ao = f32((v.e >> 8u) & 255u) / 255.0;
    out.sharp = f32((v.e >> 16u) & 255u) / 255.0;
    out.climate = v.f;
    out.world = world;
    out.water_depth = water_depth;
    out.layers = vec2<u32>(0u, 4095u);
    out.leaf = 1.0;
    return out;
}

fn hash31(p: vec3<f32>) -> f32 {
    let q = vec3<u32>(bitcast<u32>(i32(p.x)), bitcast<u32>(i32(p.y)), bitcast<u32>(i32(p.z)));
    var h = (q.x * 0x8da6b343u) ^ (q.y * 0xd8163841u) ^ (q.z * 0xcb1ab31fu);
    h = (h ^ (h >> 16u)) * 0x7feb352du;
    h = (h ^ (h >> 15u)) * 0x846ca68bu;
    return f32(h ^ (h >> 16u)) / 4294967296.0;
}

// Value noise in 3D (0..1), smooth between lattice points.
fn vnoise(p: vec3<f32>) -> f32 {
    let i = floor(p);
    let f = p - i;
    let u = f * f * (3.0 - 2.0 * f);
    let a = mix(hash31(i), hash31(i + vec3<f32>(1.0, 0.0, 0.0)), u.x);
    let b = mix(hash31(i + vec3<f32>(0.0, 1.0, 0.0)), hash31(i + vec3<f32>(1.0, 1.0, 0.0)), u.x);
    let c = mix(hash31(i + vec3<f32>(0.0, 0.0, 1.0)), hash31(i + vec3<f32>(1.0, 0.0, 1.0)), u.x);
    let d = mix(hash31(i + vec3<f32>(0.0, 1.0, 1.0)), hash31(i + vec3<f32>(1.0, 1.0, 1.0)), u.x);
    return mix(mix(a, b, u.y), mix(c, d, u.y), u.z);
}

// Three octaves, finer octaves fading out with the distance (they would only shimmer).
fn fbm3(p: vec3<f32>, dist: f32, grain: f32) -> f32 {
    var sum = 0.0;
    var amp = 0.5;
    var freq = 1.0 / max(grain, 0.02);
    var norm = 0.0;
    for (var o = 0; o < 3; o++) {
        // An octave whose cells are smaller than a few pixels is left out.
        let fade = 1.0 - smoothstep(0.5, 2.0, dist * freq * 0.004);
        sum += amp * fade * vnoise(p * freq + vec3<f32>(f32(o) * 17.3));
        norm += amp * fade;
        amp *= 0.5;
        freq *= 2.3;
    }
    return select(0.5, sum / norm, norm > 1e-3);
}

// A ground material at a point: its colour (linear) and its relief for height blending.
fn ground_sample(slot: u32, p: vec3<f32>, dist: f32, climate: u32) -> vec4<f32> {
    let m = gmats[slot];
    let n = fbm3(p, dist, m.color2.w);
    var col = mix(m.color.rgb, m.color2.rgb, smoothstep(0.25, 0.75, n));
    if m.tint == 1u {
        // Grass: its colour by place and season, varied a little.
        col = resolve_tint(1u, climate) * mix(0.8, 1.15, n);
    }
    return vec4<f32>(col, n * m.relief);
}

@fragment
fn fs_smooth(in: VsOut) -> @location(0) vec4<f32> {
    handoff(in);
    let p = in.world + g.camera.xyz;
    let dist = length(in.world);
    // Height blending (S §4.1): each material's weight raised by its relief there; the
    // highest wins within a narrow band, so stones stand through sand and turf into joints.
    var best = -1.0;
    var h: array<f32, 4>;
    var c: array<vec3<f32>, 4>;
    for (var k = 0u; k < 4u; k++) {
        let w = in.weights[k];
        let slot = (in.mats >> (8u * k)) & 255u;
        let s = ground_sample(slot, p, dist, in.climate);
        c[k] = s.rgb;
        h[k] = select(-1.0, w + s.a, w > 0.004);
        best = max(best, h[k]);
    }
    var sum = vec3<f32>(0.0);
    var total = 0.0;
    for (var k = 0u; k < 4u; k++) {
        let a = max(h[k] - (best - 0.2), 0.0);
        sum += c[k] * a;
        total += a;
    }
    let albedo = sum / max(total, 1e-4);
    // Crisp materials shade with their faces, soft ones with the smooth normal.
    var shaded = in;
    let face = normalize(cross(dpdx(in.world), dpdy(in.world)));
    let facing = select(-face, face, dot(face, in.normal) >= 0.0);
    shaded.normal = normalize(mix(in.normal, facing, in.sharp * 0.6));
    return vec4<f32>(shade_color(albedo, shaded), 1.0);
}

// ---------------------------------------------------------------- water (v1 §9.3, water.rs)
// `water.wgsl` (the waves, the sky's reflection, the sun's glitter) comes before this file.

// The scene drawn before the translucent pass, and the depth buffer (read-only).
@group(2) @binding(1) var scene_color: texture_2d<f32>;
@group(2) @binding(2) var scene_depth: texture_depth_2d;

// The light reaching water at a block: the sky by its sky-light level, the sun where it is open
// to the sky, firelight.
fn water_light(in: VsOut) -> vec3<f32> {
    let open = smoothstep(0.8, 1.0, in.light.x);
    return g.sky_light.rgb * light_curve(in.light.x) + vec3<f32>(g.sky_light.a)
        + g.sun_light.rgb * max(g.sun.y, 0.0) * open
        + g.block_light.rgb * light_curve(in.light.y);
}

// Screen-space reflection: the reflected ray from `world` along `r`, marched across the depth
// buffer in `steps` steps (denser near the start; reverse-Z depth is linear in screen space) and
// refined where it passes behind a surface; the scene there (rgb) and how far to trust it (a,
// fading toward the screen's edges, the ray's end, and where the ray passes well behind what it
// met), or nothing where the ray leaves the screen or meets only sky.
fn water_ssr(world: vec3<f32>, r: vec3<f32>, steps: u32) -> vec4<f32> {
    let near = water.screen.z;
    let size = vec2<f32>(textureDimensions(scene_depth));
    var len = 600.0;
    let c0 = g.view_proj * vec4<f32>(world, 1.0);
    var c1 = g.view_proj * vec4<f32>(world + r * len, 1.0);
    // A ray heading toward the camera stops short of the near plane.
    if c1.w < near * 4.0 {
        len *= clamp((c0.w - near * 4.0) / max(c0.w - c1.w, 1e-4), 0.0, 1.0);
        c1 = g.view_proj * vec4<f32>(world + r * len, 1.0);
    }
    let s0 = (c0.xy / c0.w * vec2<f32>(0.5, -0.5) + 0.5) * size;
    let s1 = (c1.xy / c1.w * vec2<f32>(0.5, -0.5) + 0.5) * size;
    let z0 = c0.z / c0.w;
    let z1 = c1.z / c1.w;
    var prev = 0.0;
    for (var i = 1u; i <= steps; i++) {
        let t = pow(f32(i) / f32(steps), 1.8);
        let s = mix(s0, s1, t);
        if any(s < vec2<f32>(0.0)) || any(s >= size) {
            return vec4<f32>(0.0);
        }
        let z = mix(z0, z1, t);
        if z < textureLoad(scene_depth, vec2<i32>(s), 0) {
            var lo = prev;
            var hi = t;
            for (var j = 0; j < 4; j++) {
                let m = 0.5 * (lo + hi);
                if mix(z0, z1, m) < textureLoad(scene_depth, vec2<i32>(mix(s0, s1, m)), 0) {
                    hi = m;
                } else {
                    lo = m;
                }
            }
            let sh = mix(s0, s1, hi);
            let w_ray = near / max(mix(z0, z1, hi), 1e-9);
            let w_hit = near / max(textureLoad(scene_depth, vec2<i32>(sh), 0), 1e-9);
            // Things lying on the water right by the reflection (lily pads) are not reflected.
            if abs(w_hit - c0.w) < 2.0 {
                return vec4<f32>(0.0);
            }
            let thick = 1.0 + 0.1 * w_hit;
            let edge = min(min(sh.x, size.x - sh.x), min(sh.y, size.y - sh.y)) / (0.08 * size.y);
            let fade = clamp(edge, 0.0, 1.0) * (1.0 - smoothstep(0.7, 1.0, hi))
                * (1.0 - smoothstep(thick, 3.0 * thick, w_ray - w_hit));
            return vec4<f32>(textureLoad(scene_color, vec2<i32>(sh), 0).rgb, fade);
        }
        prev = t;
    }
    return vec4<f32>(0.0);
}

// Water: Fresnel reflection of the sky and the sun's glitter on wind-driven waves over the
// scene behind (refracted, absorbed by the water's depth) at Medium and High; a translucent
// surface over it at Low; foam where the water thins against the shore. From below: the world
// above through Snell's window.
fn water_shade(in: VsOut, gx: vec3<f32>, gy: vec3<f32>) -> vec4<f32> {
    let world = in.world;
    let dist = length(world);
    let view = -world / max(dist, 1e-3);
    let p = world.xz + g.camera.xz;
    var n = in.normal;
    var slope = vec2<f32>(0.0);
    if n.y > 0.5 {
        slope = wave_slope(p, gx.xz, gy.xz);
        n = normalize(vec3<f32>(-slope.x, 1.0, -slope.y));
    }
    let below = dot(in.normal, view) < 0.0;
    if below {
        n = -n;
    }
    let ndv = clamp(dot(n, view), 0.0, 1.0);
    let fresnel = water_fresnel(ndv);
    let light = water_light(in);
    // The water's own colour: light scattered back from inside it (dark, the water's tint).
    let deep = in.tint * light * 0.06 / 3.14159265;
    // Clear blue water lets light far; green and brown water (silt, algae, peat) less.
    let turbid = clamp(1.0 - in.tint.b / max(max(in.tint.g, in.tint.r), 1e-3), 0.0, 1.0);
    let sigma = vec3<f32>(0.45, 0.07, 0.035) + vec3<f32>(0.25, 0.35, 0.45) * turbid;
    let tier = u32(water.wind.w + 0.5);
    let near = water.screen.z;
    let size = vec2<f32>(textureDimensions(scene_color));
    if below {
        // From under the surface: the world above through Snell's window, and outside it (past
        // 48.6° from the vertical) the water below, reflected entirely.
        let sin_t = 1.333 * sqrt(max(1.0 - ndv * ndv, 0.0));
        if sin_t >= 1.0 || tier == 0u {
            return vec4<f32>(deep, select(1.0, 0.85, tier == 0u));
        }
        let cos_t = sqrt(1.0 - sin_t * sin_t);
        let q = clamp(in.pos.xy + slope * 40.0, vec2<f32>(0.0), size - 1.0);
        let above = textureLoad(scene_color, vec2<i32>(q), 0).rgb;
        return vec4<f32>(mix(above, deep, water_fresnel(cos_t)), 1.0);
    }
    var r = reflect(-view, n);
    r.y = abs(r.y);
    var sky = water_sky(r);
    if tier >= 2u && fresnel > 0.04 {
        // What the screen shows where the reflection goes: the shore, the trees, the hills,
        // marched with a calmer surface than the ripples (they blur it on real water).
        var rc = reflect(-view, normalize(mix(n, vec3<f32>(0.0, 1.0, 0.0), 0.85)));
        rc.y = abs(rc.y);
        let found = water_ssr(world, rc, 20u);
        sky = mix(sky, found.rgb, found.a);
    }
    let spec = water_glitter(n, view, dist, smoothstep(0.8, 1.0, in.light.x));
    if tier == 0u {
        let alpha = clamp(0.65 + 0.35 * fresnel + dist * 0.002, 0.65, 0.95);
        return vec4<f32>(aerial(mix(deep, sky, fresnel) + spec, world), alpha);
    }
    // The scene behind, displaced by the waves where the water is deep enough to bend it.
    let w_surf = near / max(in.pos.z, 1e-9);
    let px = vec2<i32>(in.pos.xy);
    let raw0 = textureLoad(scene_depth, px, 0);
    let thick0 = max(near / max(raw0, 1e-9) - w_surf, 0.0) * dist / max(w_surf, 1e-3);
    var q = clamp(in.pos.xy + slope * min(thick0, 6.0) * 120.0 / max(w_surf, 1.0),
        vec2<f32>(0.0), size - 1.0);
    var raw = textureLoad(scene_depth, vec2<i32>(q), 0);
    if near / max(raw, 1e-9) < w_surf {
        // Something in front of the water there: look straight through.
        q = in.pos.xy;
        raw = raw0;
    }
    let behind = textureLoad(scene_color, vec2<i32>(q), 0).rgb;
    let thick = max(near / max(raw, 1e-9) - w_surf, 0.0) * dist / max(w_surf, 1e-3);
    let trans = exp(-sigma * min(thick, 1000.0));
    var body = behind * trans + deep * (1.0 - trans);
    // Foam where the water thins out against the shore, in patches that drift with the waves.
    let shore = 1.0 - smoothstep(0.05, 0.7, thick);
    if shore > 0.0 {
        let foam = length(wave_slope(p * 3.1 + vec2<f32>(17.0, 5.0), gx.xz * 3.1, gy.xz * 3.1));
        let patches = smoothstep(0.25, 0.75, foam / wave_steepness());
        body = mix(body, light * 0.7 / 3.14159265, shore * mix(0.35, 0.8, patches));
    }
    return vec4<f32>(aerial(mix(body, sky, fresnel * (1.0 - shore * 0.5)) + spec, world), 1.0);
}

@fragment
fn fs_translucent(in: VsOut) -> @location(0) vec4<f32> {
    handoff(in);
    // Screen gradients for the water's waves, taken where control flow is uniform.
    let gx = dpdx(in.world);
    let gy = dpdy(in.world);
    let c = surface_color(in);
    if in.flags == 1u {
        return water_shade(in, gx, gy);
    }
    // Ice (the one translucent solid): its texture's colour and opacity under a Fresnel
    // reflection of the sky (1.8 % head-on) and the sun's sharp glint on its smooth face; more
    // opaque where it reflects more.
    let dist = length(in.world);
    let view = -in.world / max(dist, 1e-3);
    let n = select(in.normal, vec3<f32>(0.0, 1.0, 0.0), dot(in.normal, in.normal) < 0.5);
    let ndv = abs(dot(n, view));
    let f = 0.018 + 0.982 * pow(1.0 - ndv, 5.0);
    let open = smoothstep(0.8, 1.0, in.light.x);
    let rn = select(n, -n, dot(n, view) < 0.0);
    var sky = water_sky(reflect(-view, rn)) * mix(0.35, 1.0, open);
    let rough = 0.06;
    let a2 = rough * rough * rough * rough;
    let h = normalize(view + g.sun.xyz);
    let dd = max(dot(rn, h), 0.0) * max(dot(rn, h), 0.0) * (a2 - 1.0) + 1.0;
    let glint = g.sun_light.rgb * a2 / (3.14159265 * dd * dd) * f
        * select(0.0, 1.0, dot(rn, g.sun.xyz) > 0.0) * open / (4.0 * max(ndv, 0.1));
    let body = shade_color(c.rgb, in);
    let reflected = aerial(sky + glint, in.world);
    return vec4<f32>(mix(body, reflected, f), max(c.a, f));
}
