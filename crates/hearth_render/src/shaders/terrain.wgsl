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

@fragment
fn fs_translucent(in: VsOut) -> @location(0) vec4<f32> {
    handoff(in);
    let c = surface_color(in);
    var alpha = c.a;
    if in.flags == 1u {
        // Water: slightly more opaque with distance so far oceans read solid.
        alpha = clamp(c.a + length(in.world) * 0.002, 0.55, 0.9);
    }
    return vec4<f32>(shade_color(c.rgb, in), alpha);
}
