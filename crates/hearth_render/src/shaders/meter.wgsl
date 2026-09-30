// Highlight metering: the frame is lit (pre-exposed) for the light the eye is adapted to, and a
// view into something much brighter than that (a sunset sky, a snowfield under a low sun seen
// from shade) would clip to white. A histogram of the frame's luminance finds its bright end
// (90th percentile) and scales the exposure down just enough to keep it in the tonemapper's
// colourful range; ordinary scenes are left alone. The result adapts over time here on the GPU
// and the tonemap pass reads it directly (no readback).

struct Params {
    // x: seconds since the last frame, y: 1 = adapt instantly, z: target for the bright end,
    // w: lowest scale.
    p: vec4<f32>,
};

struct Meter {
    // Adapted exposure scale, the scale aimed for, the measured bright end, unused.
    scale: f32,
    target_scale: f32,
    bright: f32,
    pad: f32,
};

@group(0) @binding(0) var<uniform> P: Params;
@group(0) @binding(1) var hdr: texture_2d<f32>;
@group(0) @binding(2) var<storage, read_write> M: Meter;

const BINS: u32 = 128u;
const LOG_MIN: f32 = -12.0;
const LOG_MAX: f32 = 8.0;
// A 64 × 36 grid of samples, nine per thread.
const GRID_X: u32 = 64u;
const GRID_Y: u32 = 36u;
const PER_THREAD: u32 = 9u;

var<workgroup> hist: array<atomic<u32>, 128>;

@compute @workgroup_size(256)
fn meter_main(@builtin(local_invocation_index) li: u32) {
    if li < BINS {
        atomicStore(&hist[li], 0u);
    }
    workgroupBarrier();
    let size = vec2<f32>(textureDimensions(hdr));
    for (var k = 0u; k < PER_THREAD; k++) {
        let s = li * PER_THREAD + k;
        let cell = vec2<f32>(f32(s % GRID_X), f32(s / GRID_X));
        let uv = (cell + 0.5) / vec2<f32>(f32(GRID_X), f32(GRID_Y));
        let c = textureLoad(hdr, vec2<i32>(uv * size), 0).rgb;
        let lum = dot(c, vec3<f32>(0.2126, 0.7152, 0.0722));
        let l2 = clamp(log2(max(lum, 1e-9)), LOG_MIN, LOG_MAX - 0.001);
        let bin = u32((l2 - LOG_MIN) / (LOG_MAX - LOG_MIN) * f32(BINS));
        // The middle of the view counts double.
        let d = uv - 0.5;
        atomicAdd(&hist[bin], select(1u, 2u, dot(d, d) < 0.09));
    }
    workgroupBarrier();
    if li != 0u {
        return;
    }
    var total = 0u;
    for (var b = 0u; b < BINS; b++) {
        total += atomicLoad(&hist[b]);
    }
    let want = u32(f32(total) * 0.9);
    var acc = 0u;
    var found = BINS - 1u;
    for (var b = 0u; b < BINS; b++) {
        acc += atomicLoad(&hist[b]);
        if acc >= want {
            found = b;
            break;
        }
    }
    let bright = exp2(LOG_MIN + (f32(found) + 0.5) / f32(BINS) * (LOG_MAX - LOG_MIN));
    let target_scale = clamp(P.p.z / bright, P.p.w, 1.0);
    var scale = target_scale;
    if P.p.y < 0.5 {
        // Adapt in log space over about a second.
        let k = 1.0 - exp(-P.p.x * 1.5);
        scale = exp2(mix(log2(clamp(M.scale, P.p.w, 1.0)), log2(target_scale), k));
    }
    M.scale = scale;
    M.target_scale = target_scale;
    M.bright = bright;
}
