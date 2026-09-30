// GPU-driven terrain culling. The CPU supplies candidate cubes (cave-culled and frustum-tested,
// with camera-relative origins in `instances`); this shader does per-direction face culling and
// two-phase Hi-Z occlusion culling, and writes indirect draws for the four opaque/cutout passes.
//
// Phase 0 draws the cubes that were visible last frame. The Hi-Z pyramid is then built from the
// resulting depth, and phase 1 tests every candidate against it: visible cubes that were not
// drawn in phase 0 are drawn now, and each cube's visibility bit is updated for next frame.

struct Slot {
    packed_off: u32,
    general_off: u32,
    model_opaque: u32,
    model_cutout: u32,
    counts: array<u32, 12>,
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
    phase: u32,
    capacity: u32,
    occlusion: u32,
    near: f32,
};

@group(0) @binding(0) var<uniform> P: Params;
@group(0) @binding(1) var<storage, read> slots: array<Slot>;
@group(0) @binding(2) var<storage, read> cands: array<u32>;
@group(0) @binding(3) var<storage, read> instances: array<vec4<f32>>;
@group(0) @binding(4) var<storage, read_write> vis: array<u32>;
@group(0) @binding(5) var<storage, read_write> draws: array<DrawArgs>;
@group(0) @binding(6) var<storage, read_write> counts: array<atomic<u32>>;
@group(0) @binding(7) var hzb: texture_2d<f32>;

const MAX_QUADS: u32 = 16384u;
const CUBE: f32 = 16.0;

fn emit(pass_index: u32, off: u32, n: u32, inst: u32) {
    let region = P.phase * 4u + pass_index;
    var done = 0u;
    loop {
        if done >= n {
            break;
        }
        let k = min(n - done, MAX_QUADS);
        let idx = atomicAdd(&counts[region], 1u);
        // Quads per region, after the draw counts (statistics for the benchmark).
        atomicAdd(&counts[8u + region], k);
        if idx < P.capacity {
            draws[region * P.capacity + idx] = DrawArgs(k * 6u, 1u, 0u, i32((off + done) * 4u), inst);
        }
        done += k;
    }
}

// True when face group `d` of a cube at camera-relative `o` can face the camera.
fn facing(d: u32, o: vec3<f32>) -> bool {
    let c = -o;
    switch d {
        case 0u: { return c.y < CUBE; }
        case 1u: { return c.y > 0.0; }
        case 2u: { return c.z < CUBE; }
        case 3u: { return c.z > 0.0; }
        case 4u: { return c.x < CUBE; }
        default: { return c.x > 0.0; }
    }
}

fn occluded(o: vec3<f32>) -> bool {
    if P.occlusion == 0u {
        return false;
    }
    var mn = vec2<f32>(1e9);
    var mx = vec2<f32>(-1e9);
    var nearest = 0.0;
    for (var c = 0u; c < 8u; c++) {
        let corner = o + vec3<f32>(f32(c & 1u), f32((c >> 1u) & 1u), f32((c >> 2u) & 1u)) * CUBE;
        let clip = P.view_proj * vec4<f32>(corner, 1.0);
        if clip.w <= P.near {
            return false;
        }
        let ndc = clip.xyz / clip.w;
        mn = min(mn, ndc.xy);
        mx = max(mx, ndc.xy);
        nearest = max(nearest, ndc.z);
    }
    // Texture space (v down), clamped to the screen.
    let a = clamp(vec2<f32>(mn.x * 0.5 + 0.5, 0.5 - mx.y * 0.5), vec2<f32>(0.0), vec2<f32>(1.0));
    let b = clamp(vec2<f32>(mx.x * 0.5 + 0.5, 0.5 - mn.y * 0.5), vec2<f32>(0.0), vec2<f32>(1.0));
    let size = (b - a) * P.hzb_size;
    let level = u32(clamp(ceil(log2(max(max(size.x, size.y), 1.0))), 0.0, f32(P.hzb_mips - 1u)));
    let dims = vec2<i32>(textureDimensions(hzb, level));
    let scale = P.hzb_size / f32(1u << level);
    let p0 = clamp(vec2<i32>(floor(a * scale)), vec2<i32>(0), dims - 1);
    let p1 = clamp(vec2<i32>(floor(b * scale)), vec2<i32>(0), dims - 1);
    // The level is chosen so the rectangle spans at most two texels per axis; the loop bound
    // keeps it safe if rounding adds one more.
    var far = 1.0;
    for (var y = p0.y; y <= min(p1.y, p0.y + 2); y++) {
        for (var x = p0.x; x <= min(p1.x, p0.x + 2); x++) {
            far = min(far, textureLoad(hzb, vec2<i32>(x, y), level).r);
        }
    }
    // Reverse-Z: the box is hidden if its nearest point is farther than every occluder.
    return nearest < far;
}

@compute @workgroup_size(64)
fn cull(@builtin(global_invocation_id) id: vec3<u32>) {
    let i = id.x;
    if i >= P.count {
        return;
    }
    let slot_index = cands[i];
    let o = instances[i].xyz;
    if P.phase == 0u {
        if vis[slot_index] == 0u {
            return;
        }
    } else {
        let visible = !occluded(o);
        let was = vis[slot_index];
        vis[slot_index] = select(0u, 1u, visible);
        if !visible || was != 0u {
            return;
        }
    }
    let s = slots[slot_index];
    var off = s.packed_off;
    for (var li = 0u; li < 2u; li++) {
        for (var d = 0u; d < 6u; d++) {
            let n = s.counts[li * 6u + d];
            if n > 0u && facing(d, o) {
                emit(li, off, n, i);
            }
            off += n;
        }
    }
    if s.model_opaque > 0u {
        emit(2u, s.general_off, s.model_opaque, i);
    }
    if s.model_cutout > 0u {
        emit(3u, s.general_off + s.model_opaque, s.model_cutout, i);
    }
}
