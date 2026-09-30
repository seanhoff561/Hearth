// Post-processing: pre-exposed HDR → display, scaled by the highlight metering. ACES filmic tonemapping (Narkowicz fit) with a
// night-vision shift: in dim scenes colours desaturate and drift toward blue as rods take over
// from cones (the Purkinje effect).

struct Params {
    // x: extra exposure multiplier, y: night factor 0..1, z,w: unused.
    p: vec4<f32>,
};

struct Meter {
    scale: f32,
    target_scale: f32,
    bright: f32,
    pad: f32,
};

@group(0) @binding(0) var<uniform> P: Params;
@group(0) @binding(1) var hdr: texture_2d<f32>;
// Highlight metering (meter.wgsl).
@group(0) @binding(2) var<storage, read> meter: Meter;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) i: u32) -> VsOut {
    let uv = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    var out: VsOut;
    out.pos = vec4<f32>(uv * 2.0 - 1.0, 0.0, 1.0);
    return out;
}

fn aces(x: vec3<f32>) -> vec3<f32> {
    let a = 2.51;
    let b = 0.03;
    let c = 2.43;
    let d = 0.59;
    let e = 0.14;
    return clamp((x * (a * x + b)) / (x * (c * x + d) + e), vec3<f32>(0.0), vec3<f32>(1.0));
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    var c = textureLoad(hdr, vec2<i32>(in.pos.xy), 0).rgb * P.p.x * meter.scale;
    let night = P.p.y;
    if night > 0.0 {
        let lum = dot(c, vec3<f32>(0.2126, 0.7152, 0.0722));
        let scotopic = vec3<f32>(lum) * vec3<f32>(0.75, 0.88, 1.15);
        c = mix(c, scotopic, night * 0.75);
    }
    return vec4<f32>(aces(c), 1.0);
}
