// The glow laid over the finished frame along the mask of the thing looked at
// (`outline.wgsl`, Amendment T §2.3): a soft light line just outside what of the thing is seen,
// and a fainter halo beyond it; over the patch of ground or water, a faint wash and a brighter
// rim where it ends. Drawn after the tonemap, in the display's light, alike by day and by night.

struct Glow {
    // Linear colour; a the glow's strength.
    color: vec4<f32>,
    // xy the frame's size in pixels; z a pixel's scale (the line widens on large frames).
    size: vec4<f32>,
};

@group(0) @binding(0) var<uniform> u: Glow;
@group(0) @binding(1) var mask: texture_2d<f32>;
@group(0) @binding(2) var samp: sampler;

@vertex
fn vs_full(@builtin(vertex_index) vi: u32) -> @builtin(position) vec4<f32> {
    let p = vec2<f32>(f32((vi << 1u) & 2u), f32(vi & 2u));
    return vec4<f32>(p * 2.0 - 1.0, 0.0, 1.0);
}

const RING: u32 = 12u;

@fragment
fn fs_glow(@builtin(position) frag: vec4<f32>) -> @location(0) vec4<f32> {
    let uv = frag.xy / u.size.xy;
    let px = u.size.z / u.size.xy;
    let here = textureSampleLevel(mask, samp, uv, 0.0);
    // The most of the thing within a pixel and a half, and within four.
    var near = 0.0;
    var far = 0.0;
    for (var i = 0u; i < RING; i++) {
        let a = f32(i) * 6.2831853 / f32(RING);
        let d = vec2<f32>(cos(a), sin(a));
        near = max(near, textureSampleLevel(mask, samp, uv + d * px * 1.5, 0.0).r);
        far = max(far, textureSampleLevel(mask, samp, uv + d * px * 4.0, 0.0).r);
    }
    let line = clamp(near - here.r, 0.0, 1.0);
    let halo = clamp(far - here.r, 0.0, 1.0);
    // The patch: its wash, and its rim where the disc fades out.
    let g = here.g;
    let rim = smoothstep(0.05, 0.5, g) * (1.0 - smoothstep(0.5, 0.95, g));
    let a = max(max(line * 0.85, halo * 0.3), max(g * 0.12, rim * 0.65)) * u.color.a;
    if a <= 0.001 {
        discard;
    }
    return vec4<f32>(u.color.rgb, a);
}
