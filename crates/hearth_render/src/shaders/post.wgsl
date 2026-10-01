// Post-processing: pre-exposed HDR → display, scaled by the highlight metering. ACES filmic tonemapping (Narkowicz fit) with a
// night-vision shift: in dim scenes colours desaturate and drift toward blue as rods take over
// from cones (the Purkinje effect).
//
// At a render scale other than 1 the frame is tonemapped at its own size (`fs_tonemap`, in
// perceptual sRGB values), then upscaled with the two passes of AMD FidelityFX Super Resolution
// 1 (MIT; EASU: edge-adaptive Lanczos upsampling, `fs_easu`; RCAS: contrast-adaptive
// sharpening, `fs_rcas`), or filtered down when it is larger (`fs_resample`). Dithering is
// always the last step.

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

struct Scale {
    // Source texels per output pixel, and the source's size in texels.
    ratio: vec2<f32>,
    size: vec2<f32>,
    // x: RCAS sharpening (2^-stops); y, z, w: unused.
    sharp: vec4<f32>,
};

// The upscaling and resampling passes: the tonemapped source and a bilinear sampler.
@group(0) @binding(3) var<uniform> S: Scale;
@group(0) @binding(4) var src: texture_2d<f32>;
@group(0) @binding(5) var src_linear: sampler;

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

fn to_srgb(c: vec3<f32>) -> vec3<f32> {
    return select(1.055 * pow(c, vec3<f32>(1.0 / 2.4)) - 0.055, c * 12.92, c <= vec3<f32>(0.0031308));
}

fn to_linear(c: vec3<f32>) -> vec3<f32> {
    return select(pow((c + 0.055) / 1.055, vec3<f32>(2.4)), c / 12.92, c <= vec3<f32>(0.04045));
}

// Two uniform values per pixel from its coordinates (a small integer hash).
fn hash2(p: vec2<u32>) -> vec2<f32> {
    var h = p.x * 0x8da6b343u ^ p.y * 0xd8163841u;
    h = (h ^ (h >> 15u)) * 0x2c1b3c6du;
    h = (h ^ (h >> 12u)) * 0x297a2d39u;
    h = h ^ (h >> 15u);
    return vec2<f32>(f32(h & 0xffffu), f32(h >> 16u)) / 65535.0;
}

// The HDR texel at `p`, exposed, night-shifted and tonemapped, in perceptual (sRGB) values.
fn graded(p: vec2<i32>) -> vec3<f32> {
    var c = textureLoad(hdr, p, 0).rgb * P.p.x * meter.scale;
    let night = P.p.y;
    if night > 0.0 {
        let lum = dot(c, vec3<f32>(0.2126, 0.7152, 0.0722));
        let scotopic = vec3<f32>(lum) * vec3<f32>(0.75, 0.88, 1.15);
        c = mix(c, scotopic, night * 0.75);
    }
    return to_srgb(aces(c));
}

// The output pixel `p` of a perceptual colour: dithered with triangular noise of about one step
// of the 8-bit sRGB output, added where the output is quantized so smooth skies and fog don't
// band, and returned linear for the sRGB target.
fn finish(s: vec3<f32>, p: vec2<f32>) -> vec4<f32> {
    let r = hash2(vec2<u32>(p));
    let noise = (r.x + r.y - 1.0) / 255.0;
    return vec4<f32>(to_linear(clamp(s + vec3<f32>(noise), vec3<f32>(0.0), vec3<f32>(1.0))), 1.0);
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    return finish(graded(vec2<i32>(in.pos.xy)), in.pos.xy);
}

// Tonemaps at the render size, for the upscaler.
@fragment
fn fs_tonemap(in: VsOut) -> @location(0) vec4<f32> {
    return vec4<f32>(graded(vec2<i32>(in.pos.xy)), 1.0);
}

// A texel of the source, clamped to its edges.
fn ld(p: vec2<i32>) -> vec3<f32> {
    return textureLoad(src, clamp(p, vec2<i32>(0), vec2<i32>(S.size) - 1), 0).rgb;
}

// FSR 1's luma estimate (twice the luma, from perceptual values).
fn luma2(c: vec3<f32>) -> f32 {
    return c.b * 0.5 + (c.r * 0.5 + c.g);
}

// EASU: how strongly and which way the edge runs around one of the four texels nearest the
// sample (a cross of lumas: up, left, centre, right, down), weighted by its bilinear weight.
fn easu_edge(w: f32, a: f32, b: f32, c: f32, d: f32, e: f32) -> vec3<f32> {
    let len_x = saturate(abs(d - b) / max(max(abs(d - c), abs(c - b)), 1e-6));
    let len_y = saturate(abs(e - a) / max(max(abs(e - c), abs(c - a)), 1e-6));
    return vec3<f32>((d - b) * w, (e - a) * w, (len_x * len_x + len_y * len_y) * w);
}

// EASU: one tap of the edge-shaped Lanczos-2 window (a polynomial approximation), at `off` from
// the sample.
fn easu_weight(off: vec2<f32>, dir: vec2<f32>, len2: vec2<f32>, lob: f32, clp: f32) -> f32 {
    let v = vec2<f32>(off.x * dir.x + off.y * dir.y, off.x * -dir.y + off.y * dir.x) * len2;
    let d2 = min(dot(v, v), clp);
    let wb = 0.4 * d2 - 1.0;
    let wa = lob * d2 - 1.0;
    return (25.0 / 16.0 * wb * wb - (25.0 / 16.0 - 1.0)) * wa * wa;
}

// EASU: the source around the output pixel, resampled by a 12-tap window stretched along the
// local edge and kept within the four nearest texels' range (no ringing).
@fragment
fn fs_easu(in: VsOut) -> @location(0) vec4<f32> {
    let pp = in.pos.xy * S.ratio - 0.5;
    let fp = floor(pp);
    let t = pp - fp;
    let o = vec2<i32>(fp);
    //    b c
    //  e f g h
    //  i j k l
    //    n o
    let b = ld(o + vec2<i32>(0, -1));
    let c = ld(o + vec2<i32>(1, -1));
    let e = ld(o + vec2<i32>(-1, 0));
    let f = ld(o);
    let g = ld(o + vec2<i32>(1, 0));
    let h = ld(o + vec2<i32>(2, 0));
    let i = ld(o + vec2<i32>(-1, 1));
    let j = ld(o + vec2<i32>(0, 1));
    let k = ld(o + vec2<i32>(1, 1));
    let l = ld(o + vec2<i32>(2, 1));
    let n = ld(o + vec2<i32>(0, 2));
    let q = ld(o + vec2<i32>(1, 2));
    let lb = luma2(b);
    let lc = luma2(c);
    let le = luma2(e);
    let lf = luma2(f);
    let lg = luma2(g);
    let lh = luma2(h);
    let li = luma2(i);
    let lj = luma2(j);
    let lk = luma2(k);
    let ll = luma2(l);
    let ln = luma2(n);
    let lq = luma2(q);
    let s = easu_edge((1.0 - t.x) * (1.0 - t.y), lb, le, lf, lg, lj)
        + easu_edge(t.x * (1.0 - t.y), lc, lf, lg, lh, lk)
        + easu_edge((1.0 - t.x) * t.y, lf, li, lj, lk, ln)
        + easu_edge(t.x * t.y, lg, lj, lk, ll, lq);
    var dir = s.xy;
    let r2 = dot(dir, dir);
    if r2 < 1.0 / 32768.0 {
        dir = vec2<f32>(1.0, 0.0);
    } else {
        dir = dir * inverseSqrt(r2);
    }
    let len = (s.z * 0.5) * (s.z * 0.5);
    // Stretch the window along the edge (more on diagonals) and narrow it across.
    let stretch = dot(dir, dir) / max(abs(dir.x), abs(dir.y));
    let len2 = vec2<f32>(1.0 + (stretch - 1.0) * len, 1.0 - 0.5 * len);
    // The negative lobe: 0.5 in flat areas, weaker along edges.
    let lob = 0.5 + ((1.0 / 4.0 - 0.04) - 0.5) * len;
    let clp = 1.0 / lob;
    // The twelve taps (unrolled: indexing arrays by a variable spills them to slow memory).
    let wb = easu_weight(vec2<f32>(0.0, -1.0) - t, dir, len2, lob, clp);
    let wc = easu_weight(vec2<f32>(1.0, -1.0) - t, dir, len2, lob, clp);
    let wi = easu_weight(vec2<f32>(-1.0, 1.0) - t, dir, len2, lob, clp);
    let wj = easu_weight(vec2<f32>(0.0, 1.0) - t, dir, len2, lob, clp);
    let wf = easu_weight(-t, dir, len2, lob, clp);
    let we = easu_weight(vec2<f32>(-1.0, 0.0) - t, dir, len2, lob, clp);
    let wk = easu_weight(vec2<f32>(1.0, 1.0) - t, dir, len2, lob, clp);
    let wl = easu_weight(vec2<f32>(2.0, 1.0) - t, dir, len2, lob, clp);
    let wh = easu_weight(vec2<f32>(2.0, 0.0) - t, dir, len2, lob, clp);
    let wg = easu_weight(vec2<f32>(1.0, 0.0) - t, dir, len2, lob, clp);
    let wq = easu_weight(vec2<f32>(1.0, 2.0) - t, dir, len2, lob, clp);
    let wn = easu_weight(vec2<f32>(0.0, 2.0) - t, dir, len2, lob, clp);
    let acc = b * wb + c * wc + i * wi + j * wj + f * wf + e * we + k * wk + l * wl + h * wh
        + g * wg + q * wq + n * wn;
    let wsum = wb + wc + wi + wj + wf + we + wk + wl + wh + wg + wq + wn;
    let lo = min(min(f, g), min(j, k));
    let hi = max(max(f, g), max(j, k));
    return vec4<f32>(clamp(acc / wsum, lo, hi), 1.0);
}

// RCAS: sharpens by the most negative lobe the neighbourhood allows without clipping, less where
// the pixel stands out alone (noise), then dithers.
@fragment
fn fs_rcas(in: VsOut) -> @location(0) vec4<f32> {
    let p = vec2<i32>(in.pos.xy);
    //   b
    // d e f
    //   h
    let b = ld(p + vec2<i32>(0, -1));
    let d = ld(p + vec2<i32>(-1, 0));
    let e = ld(p);
    let f = ld(p + vec2<i32>(1, 0));
    let h = ld(p + vec2<i32>(0, 1));
    let mn4 = min(min(b, d), min(f, h));
    let mx4 = max(max(b, d), max(f, h));
    let hit_min = min(mn4, e) / max(4.0 * mx4, vec3<f32>(1e-5));
    let hit_max = (1.0 - max(mx4, e)) / min(4.0 * mn4 - 4.0, vec3<f32>(-1e-5));
    let lobe3 = max(-hit_min, hit_max);
    // 0.25 - 1/16: the limit that keeps the filter from ringing.
    var lobe = max(-0.1875, min(max(lobe3.r, max(lobe3.g, lobe3.b)), 0.0)) * S.sharp.x;
    let lb = luma2(b);
    let ld_ = luma2(d);
    let le = luma2(e);
    let lf = luma2(f);
    let lh = luma2(h);
    let range = max(max(max(lb, ld_), max(le, lf)), lh) - min(min(min(lb, ld_), min(le, lf)), lh);
    let nz = saturate(abs(0.25 * (lb + ld_ + lf + lh) - le) / max(range, 1e-5));
    lobe *= 1.0 - 0.5 * nz;
    let c = (lobe * (b + d + f + h) + e) / (4.0 * lobe + 1.0);
    return finish(c, in.pos.xy);
}

// A larger render filtered down: four bilinear taps spread over the pixel's footprint (a 2×2
// box at a scale of 2), then dithered.
@fragment
fn fs_resample(in: VsOut) -> @location(0) vec4<f32> {
    let centre = in.pos.xy * S.ratio;
    let spread = S.ratio * 0.25;
    var c = vec3<f32>(0.0);
    for (var m = 0; m < 4; m++) {
        let sgn = vec2<f32>(f32(m & 1) * 2.0 - 1.0, f32(m >> 1u) * 2.0 - 1.0);
        c += textureSampleLevel(src, src_linear, (centre + sgn * spread) / S.size, 0.0).rgb;
    }
    return finish(c * 0.25, in.pos.xy);
}
