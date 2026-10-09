// The mask of the thing looked at (Amendment T §2.3; `common.wgsl` is prepended): its own
// shape drawn into a two-channel target the size of the scene — red where the thing itself is
// seen (its boxes, or the quads of its block, alpha-tested as drawn), green over the patch of
// ground a dig there takes or the water about the point looked at. `outline_glow.wgsl` lays a
// soft glow over the finished frame along it.
//
// The thing is drawn with the unjittered camera (the glow is laid after the temporal
// anti-aliasing, and must not shimmer with its jitter), tested against the scene's depth, and
// pulled a little toward the eye (its place on screen unchanged) so its own surface does not
// hide it: only what of it is seen is marked.

struct Outline {
    // Clip space from camera-relative space, without the jitter.
    view_proj: mat4x4<f32>,
    // The scene's (jittered) clip space back to camera-relative space: its depth read back.
    inv_view_proj: mat4x4<f32>,
    // The block's corner, camera-relative; w 1 to mark only its fruit and flowers.
    origin: vec4<f32>,
    // The disc's middle (camera-relative) and radius (blocks).
    disc: vec4<f32>,
    // x the disc's kind (0 the ground, 1 water), y how far toward the eye the thing is drawn (a
    // scale on its distance), zw the mask's size in pixels.
    params: vec4<f32>,
    // x the climate code of the block's column (its foliage's season).
    climate: vec4<u32>,
};

@group(1) @binding(0) var<uniform> o: Outline;

// A box placed by an instance (`hearth_character::FigureInstance`), as `figure.wgsl` draws it.
struct Part {
    r0: vec4<f32>,
    r1: vec4<f32>,
    r2: vec4<f32>,
    color: u32,
    light: u32,
    coat_at: u32,
    coat_size: u32,
};

// A quad of the block (`outline::MaskQuad`): corners (block-local) with their texel u, each
// corner's texel v; layer:12 | frames−1:4 | frame time−1:4 | sway:2 (1 the upper corners, 2 all)
// | cutout:1; and the tint kind (4) with the face's direction (3, 7 none) above it.
struct MaskQuad {
    c: array<vec4<f32>, 4>,
    v: vec4<f32>,
    layer: u32,
    flags: u32,
    pad0: u32,
    pad1: u32,
};

@group(1) @binding(1) var<storage, read> parts: array<Part>;
@group(1) @binding(2) var<storage, read> mquads: array<MaskQuad>;

// The thing drawn toward the eye by `o.params.y` (its pixel unchanged), then projected.
fn project(world: vec3<f32>) -> vec4<f32> {
    return o.view_proj * vec4<f32>(world * o.params.y, 1.0);
}

// ---------------------------------------------------------------- boxes (things, animals)

fn box_axes(f: u32) -> mat3x3<f32> {
    switch f {
        case 0u: { return mat3x3<f32>(vec3(1.0, 0.0, 0.0), vec3(0.0, 0.0, -1.0), vec3(0.0, 1.0, 0.0)); }
        case 1u: { return mat3x3<f32>(vec3(-1.0, 0.0, 0.0), vec3(0.0, 0.0, 1.0), vec3(0.0, 1.0, 0.0)); }
        case 2u: { return mat3x3<f32>(vec3(0.0, 1.0, 0.0), vec3(1.0, 0.0, 0.0), vec3(0.0, 0.0, -1.0)); }
        case 3u: { return mat3x3<f32>(vec3(0.0, -1.0, 0.0), vec3(1.0, 0.0, 0.0), vec3(0.0, 0.0, 1.0)); }
        case 4u: { return mat3x3<f32>(vec3(0.0, 0.0, 1.0), vec3(1.0, 0.0, 0.0), vec3(0.0, 1.0, 0.0)); }
        default: { return mat3x3<f32>(vec3(0.0, 0.0, -1.0), vec3(-1.0, 0.0, 0.0), vec3(0.0, 1.0, 0.0)); }
    }
}

// Corner of the unit cube (−½..½) for a vertex of 36, as `figure.wgsl` has them.
fn box_corner(vi: u32) -> vec3<f32> {
    let ax = box_axes(vi / 6u);
    let corner = array<u32, 6>(0u, 1u, 2u, 0u, 2u, 3u)[vi % 6u];
    let cu = select(-1.0, 1.0, corner == 1u || corner == 2u);
    let cv = select(-1.0, 1.0, corner >= 2u);
    return 0.5 * (ax[0] + cu * ax[1] + cv * ax[2]);
}

@vertex
fn vs_box(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> @builtin(position) vec4<f32> {
    let p = parts[ii];
    let m = mat3x3<f32>(
        vec3<f32>(p.r0.x, p.r1.x, p.r2.x),
        vec3<f32>(p.r0.y, p.r1.y, p.r2.y),
        vec3<f32>(p.r0.z, p.r1.z, p.r2.z),
    );
    let world = m * box_corner(vi) + vec3<f32>(p.r0.w, p.r1.w, p.r2.w);
    return project(curve(world));
}

@fragment
fn fs_seen() -> @location(0) vec4<f32> {
    return vec4<f32>(1.0, 0.0, 0.0, 0.0);
}

// ---------------------------------------------------------------- the quads of a block

struct QuadOut {
    @builtin(position) pos: vec4<f32>,
    // Texels (0..16, wrapping where a face's texture is turned).
    @location(0) uv: vec2<f32>,
    // Camera-relative, as drawn (swayed and curved): where its foliage's leaves fall.
    @location(1) world: vec3<f32>,
    @location(2) @interpolate(flat) layer: u32,
    @location(3) @interpolate(flat) flags: u32,
    @location(4) @interpolate(flat) cutout: u32,
};

@vertex
fn vs_quad(@builtin(vertex_index) vi: u32) -> QuadOut {
    let q = mquads[vi / 6u];
    let c = array<u32, 6>(0u, 1u, 2u, 0u, 2u, 3u)[vi % 6u];
    let local = q.c[c].xyz;
    var world = o.origin.xyz + local;
    // The sway of plants and leaves in the wind, as the terrain moves them.
    switch (q.layer >> 20u) & 3u {
        case 1u: {
            let up = select(0.0, 1.0, fract(local.y) > 0.5 || c >= 2u);
            world += wind(world, up);
        }
        case 2u: { world += wind(world, 1.0); }
        default: {}
    }
    world = curve(world);
    var out: QuadOut;
    out.pos = project(world);
    out.uv = vec2<f32>(q.c[c].w, q.v[c]);
    out.world = world;
    out.layer = animated_layer(q.layer & 4095u, (q.layer >> 12u) & 15u, (q.layer >> 16u) & 15u);
    out.flags = q.flags;
    out.cutout = (q.layer >> 22u) & 1u;
    return out;
}

@fragment
fn fs_quad(in: QuadOut) -> @location(0) vec4<f32> {
    let fruit = o.origin.w > 0.5;
    if in.cutout == 1u || fruit {
        // The texel as the terrain draws it, crisp: its alpha exactly (a fruit's or flower's is
        // one step short of full, `hearth_texgen::PART_ALPHA`).
        let texel = vec2<i32>(floor(in.uv)) & vec2<i32>(15);
        let a = textureLoad(tex, texel, in.layer, 0).a;
        if a < 0.5 || (fruit && a > 0.998) {
            discard;
        }
        let normal = face_normal((in.flags >> 4u) & 7u);
        let leaf = tint_leaf(in.flags & 15u, o.climate.x);
        if leaf_fall(in.world, normal, leaf) == 2u {
            discard;
        }
    }
    return vec4<f32>(1.0, 0.0, 0.0, 0.0);
}

// ---------------------------------------------------------------- the ground's patch, water

@group(2) @binding(0) var scene_depth: texture_depth_2d;

// A triangle over the whole target (scissored to the disc's part of it).
@vertex
fn vs_full(@builtin(vertex_index) vi: u32) -> @builtin(position) vec4<f32> {
    let p = vec2<f32>(f32((vi << 1u) & 2u), f32(vi & 2u));
    return vec4<f32>(p * 2.0 - 1.0, 0.0, 1.0);
}

fn unproject(frag: vec2<f32>, depth: f32) -> vec3<f32> {
    let ndc = vec2<f32>(frag.x / o.params.z * 2.0 - 1.0, 1.0 - frag.y / o.params.w * 2.0);
    let h = o.inv_view_proj * vec4<f32>(ndc, depth, 1.0);
    return h.xyz / h.w;
}

@fragment
fn fs_disc(@builtin(position) frag: vec4<f32>) -> @location(0) vec4<f32> {
    let d = textureLoad(scene_depth, vec2<i32>(frag.xy), 0);
    // What the scene shows there (nothing drawn: the sky, infinitely far).
    let seen = select(unproject(frag.xy, d), vec3<f32>(1e9), d <= 0.0);
    var p = seen;
    var dist = 0.0;
    if o.params.x < 0.5 {
        // The ground: what is seen there, within the bowl the dig takes.
        if d <= 0.0 {
            discard;
        }
        dist = length(p - o.disc.xyz);
    } else {
        // Water: where the look meets its surface, if nothing stands before it there.
        let dir = normalize(unproject(frag.xy, 0.5));
        if abs(dir.y) < 1e-4 {
            discard;
        }
        let t = o.disc.y / dir.y;
        if t <= 0.0 || t > length(seen) {
            discard;
        }
        p = dir * t;
        dist = length(p.xz - o.disc.xz);
    }
    // Full within, fading over its last few centimetres.
    let r = o.disc.w;
    let v = 1.0 - smoothstep(r - 0.05, r, dist);
    if v <= 0.0 {
        discard;
    }
    return vec4<f32>(0.0, v, 0.0, 0.0);
}
