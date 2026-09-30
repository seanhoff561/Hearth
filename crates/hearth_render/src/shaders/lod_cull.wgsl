// GPU culling of distant (LOD) tiles against the Hi-Z pyramid of the full-detail terrain (built
// by `cull.wgsl`'s pass after the terrain's first phase, from this frame's depth): a tile wholly
// behind near terrain — a hill, a cave's walls — is not drawn. The CPU has already tested the
// candidates against the frustum; the survivors become the draws of one indirect-count
// multi-draw.

struct Cand {
    // Camera-relative bounds (xyz), including the planet's curvature.
    lo: vec4<f32>,
    hi: vec4<f32>,
    // Index count, base vertex, first instance, unused.
    draw: vec4<u32>,
};

struct DrawArgs {
    index_count: u32,
    instance_count: u32,
    first_index: u32,
    base_vertex: i32,
    first_instance: u32,
};

struct Params {
    view_proj: mat4x4<f32>,
    // Hi-Z level-0 size in texels (depth size / 2, unrounded).
    hzb_size: vec2<f32>,
    hzb_mips: u32,
    count: u32,
    near: f32,
    occlusion: u32,
    pad0: u32,
    pad1: u32,
};

@group(0) @binding(0) var<uniform> P: Params;
@group(0) @binding(1) var<storage, read> cands: array<Cand>;
@group(0) @binding(2) var<storage, read_write> draws: array<DrawArgs>;
@group(0) @binding(3) var<storage, read_write> count: atomic<u32>;
@group(0) @binding(4) var hzb: texture_2d<f32>;

// As `occluded` in `cull.wgsl`, for any box.
fn occluded(lo: vec3<f32>, hi: vec3<f32>) -> bool {
    var mn = vec2<f32>(1e9);
    var mx = vec2<f32>(-1e9);
    var nearest = 0.0;
    for (var c = 0u; c < 8u; c++) {
        let pick = vec3<f32>(f32(c & 1u), f32((c >> 1u) & 1u), f32((c >> 2u) & 1u));
        let clip = P.view_proj * vec4<f32>(mix(lo, hi, pick), 1.0);
        if clip.w <= P.near {
            return false;
        }
        let ndc = clip.xyz / clip.w;
        mn = min(mn, ndc.xy);
        mx = max(mx, ndc.xy);
        nearest = max(nearest, ndc.z);
    }
    let a = clamp(vec2<f32>(mn.x * 0.5 + 0.5, 0.5 - mx.y * 0.5), vec2<f32>(0.0), vec2<f32>(1.0));
    let b = clamp(vec2<f32>(mx.x * 0.5 + 0.5, 0.5 - mn.y * 0.5), vec2<f32>(0.0), vec2<f32>(1.0));
    let size = (b - a) * P.hzb_size;
    let level = u32(clamp(ceil(log2(max(max(size.x, size.y), 1.0))), 0.0, f32(P.hzb_mips - 1u)));
    let dims = vec2<i32>(textureDimensions(hzb, level));
    let scale = P.hzb_size / f32(1u << level);
    let p0 = clamp(vec2<i32>(floor(a * scale)), vec2<i32>(0), dims - 1);
    let p1 = clamp(vec2<i32>(floor(b * scale)), vec2<i32>(0), dims - 1);
    var far = 1.0;
    for (var y = p0.y; y <= min(p1.y, p0.y + 2); y++) {
        for (var x = p0.x; x <= min(p1.x, p0.x + 2); x++) {
            far = min(far, textureLoad(hzb, vec2<i32>(x, y), level).r);
        }
    }
    // Reverse-Z: hidden if the box's nearest point is farther than every occluder.
    return nearest < far;
}

@compute @workgroup_size(64)
fn cull_lod(@builtin(global_invocation_id) id: vec3<u32>) {
    let i = id.x;
    if i >= P.count {
        return;
    }
    let c = cands[i];
    if P.occlusion != 0u && occluded(c.lo.xyz, c.hi.xyz) {
        return;
    }
    let k = atomicAdd(&count, 1u);
    draws[k] = DrawArgs(c.draw.x, 1u, 0u, bitcast<i32>(c.draw.y), c.draw.z);
}
