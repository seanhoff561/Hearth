// Temporal anti-aliasing: each frame is rendered with the camera jittered by a fraction of a
// pixel; this pass finds where each pixel's surface was in the last frame (through the depth
// buffer and the last frame's camera; the sky at infinite depth by its direction alone), takes
// the history there, clamps it to the colours around the pixel now (so what moved or came into
// view does not ghost), and blends a tenth of the new frame into it.

struct Params {
    // This frame's jittered clip space back to camera-relative space.
    inv_view_proj: mat4x4<f32>,
    // The last frame's unjittered view-projection, camera-relative to the last camera.
    prev_view_proj: mat4x4<f32>,
    // xyz: this camera minus the last (m); w: 1 when there is no history.
    delta: vec4<f32>,
    // xy: this frame's jitter (pixels); zw: the target's size (pixels).
    jitter: vec4<f32>,
};

@group(0) @binding(0) var<uniform> P: Params;
@group(0) @binding(1) var current: texture_2d<f32>;
@group(0) @binding(2) var depth: texture_depth_2d;
@group(0) @binding(3) var history: texture_2d<f32>;
@group(0) @binding(4) var lin: sampler;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vi: u32) -> VsOut {
    let p = vec2<f32>(f32((vi << 1u) & 2u), f32(vi & 2u));
    var out: VsOut;
    out.pos = vec4<f32>(p * 2.0 - 1.0, 0.0, 1.0);
    out.uv = vec2<f32>(p.x, 1.0 - p.y);
    return out;
}

fn to_ycocg(c: vec3<f32>) -> vec3<f32> {
    return vec3<f32>(
        0.25 * c.r + 0.5 * c.g + 0.25 * c.b,
        0.5 * c.r - 0.5 * c.b,
        -0.25 * c.r + 0.5 * c.g - 0.25 * c.b,
    );
}

fn from_ycocg(c: vec3<f32>) -> vec3<f32> {
    return vec3<f32>(c.x + c.y - c.z, c.x + c.z, c.x - c.y - c.z);
}

// Weighs bright samples down so a single firefly does not dominate the blend.
fn tone(c: vec3<f32>) -> vec3<f32> {
    return c / (1.0 + max(c.r, max(c.g, c.b)));
}

fn untone(c: vec3<f32>) -> vec3<f32> {
    return c / max(1.0 - max(c.r, max(c.g, c.b)), 1e-4);
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let size = P.jitter.zw;
    let px = vec2<i32>(in.pos.xy);
    let uv = in.pos.xy / size;
    let now = tone(textureLoad(current, px, 0).rgb);
    if P.delta.w > 0.5 {
        return vec4<f32>(untone(now), 1.0);
    }
    // The colours around the pixel now, as a box in YCoCg (mean ± deviation).
    var m1 = vec3<f32>(0.0);
    var m2 = vec3<f32>(0.0);
    for (var dy = -1; dy <= 1; dy++) {
        for (var dx = -1; dx <= 1; dx++) {
            let q = clamp(px + vec2<i32>(dx, dy), vec2<i32>(0), vec2<i32>(size) - 1);
            let c = to_ycocg(tone(textureLoad(current, q, 0).rgb));
            m1 += c;
            m2 += c * c;
        }
    }
    let mean = m1 / 9.0;
    let dev = sqrt(max(m2 / 9.0 - mean * mean, vec3<f32>(0.0)));
    let lo = mean - 1.25 * dev;
    let hi = mean + 1.25 * dev;
    // Where the surface at this pixel was in the last frame.
    let d = textureLoad(depth, px, 0);
    let ndc = vec2<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0);
    let world = P.inv_view_proj * vec4<f32>(ndc, d, 1.0);
    let prev = P.prev_view_proj * vec4<f32>(world.xyz + P.delta.xyz * world.w, world.w);
    if prev.w <= 0.0 {
        return vec4<f32>(untone(now), 1.0);
    }
    let prev_ndc = prev.xy / prev.w;
    let prev_uv = vec2<f32>(prev_ndc.x * 0.5 + 0.5, 0.5 - prev_ndc.y * 0.5);
    if any(prev_uv < vec2<f32>(0.0)) || any(prev_uv > vec2<f32>(1.0)) {
        return vec4<f32>(untone(now), 1.0);
    }
    let old = tone(textureSampleLevel(history, lin, prev_uv, 0.0).rgb);
    let clamped = from_ycocg(clamp(to_ycocg(old), lo, hi));
    // Faster to forget when the view moved a lot (the history is resampled and soft).
    let moved = length((prev_uv - uv) * size);
    let alpha = mix(0.1, 0.25, clamp(moved / 8.0, 0.0, 1.0));
    return vec4<f32>(untone(mix(clamped, now, alpha)), 1.0);
}
