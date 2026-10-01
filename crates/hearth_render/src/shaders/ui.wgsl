// The interface: rectangles and text in screen pixels over the finished frame. Colours arrive
// in sRGB; they are made linear when the target encodes sRGB itself.

struct Screen {
    // xy: frame size (px); z: 1 when the target is sRGB.
    size: vec4<f32>,
};

@group(0) @binding(0) var<uniform> screen: Screen;
@group(0) @binding(1) var atlas: texture_2d<f32>;
@group(0) @binding(2) var samp: sampler;

struct VsIn {
    @location(0) pos: vec2<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) color: vec4<f32>,
};

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
};

@vertex
fn vs_main(v: VsIn) -> VsOut {
    var out: VsOut;
    let ndc = v.pos / screen.size.xy * 2.0 - 1.0;
    out.pos = vec4<f32>(ndc.x, -ndc.y, 0.0, 1.0);
    out.uv = v.uv;
    out.color = v.color;
    return out;
}

fn srgb_to_linear(c: vec3<f32>) -> vec3<f32> {
    let lo = c / 12.92;
    let hi = pow((c + 0.055) / 1.055, vec3<f32>(2.4));
    return select(hi, lo, c <= vec3<f32>(0.04045));
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let ink = textureSample(atlas, samp, in.uv).r;
    var rgb = in.color.rgb;
    if screen.size.z > 0.5 {
        rgb = srgb_to_linear(rgb);
    }
    return vec4<f32>(rgb, in.color.a * ink);
}
