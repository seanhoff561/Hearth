// Precipitation: rain streaks and snowflakes generated in the vertex shader from the vertex
// index. Each particle has a fixed place in a box that wraps around the camera, so the rain
// stays put in the world while the camera moves; particles fall with the wind, are hidden where
// something blocks the sky above them (a height map of the highest sky-blocking block per
// column) and behind terrain (depth test), and are lit by the frame's sky and sun light.

struct Params {
    // Camera-relative view-projection.
    view_proj: mat4x4<f32>,
    // xyz: camera position modulo the box; w: seconds.
    cam_mod: vec4<f32>,
    // x, z: camera position relative to the height map origin; y: camera height; w: map size.
    cam: vec4<f32>,
    // xyz: fall velocity of rain (m/s, wind included); w: share of rain among the particles.
    rain: vec4<f32>,
    // xyz: wind (m/s); w: unused.
    wind: vec4<f32>,
    // xyz: box size (m); w: opacity scale.
    box_size: vec4<f32>,
    // rgb: sky irradiance (pre-exposed).
    ambient: vec4<f32>,
    // rgb: direct sun or moon light (pre-exposed).
    direct: vec4<f32>,
    // xyz: camera right vector.
    right: vec4<f32>,
    // xyz: camera up vector.
    up: vec4<f32>,
};

@group(0) @binding(0) var<uniform> P: Params;
@group(0) @binding(1) var heights: texture_2d<i32>;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
    // 0 rain, 1 snow.
    @location(1) @interpolate(flat) kind: u32,
    @location(2) alpha: f32,
};

fn hash_u(x: u32) -> u32 {
    var h = x * 747796405u + 2891336453u;
    h = ((h >> ((h >> 28u) + 4u)) ^ h) * 277803737u;
    return (h >> 22u) ^ h;
}

fn rand(i: u32, k: u32) -> f32 {
    return f32(hash_u(i * 4u + k)) / 4294967296.0;
}

@vertex
fn vs_main(@builtin(vertex_index) vi: u32) -> VsOut {
    var out: VsOut;
    let i = vi / 6u;
    let corners = array<vec2<f32>, 6>(
        vec2<f32>(-1.0, -1.0), vec2<f32>(1.0, -1.0), vec2<f32>(1.0, 1.0),
        vec2<f32>(-1.0, -1.0), vec2<f32>(1.0, 1.0), vec2<f32>(-1.0, 1.0),
    );
    let corner = corners[vi % 6u];
    out.uv = corner;
    let box_size = P.box_size.xyz;
    let base = vec3<f32>(rand(i, 0u), rand(i, 1u), rand(i, 2u)) * box_size;
    let r = rand(i, 3u);
    let is_rain = r < P.rain.w;
    let t = P.cam_mod.w;
    var motion: vec3<f32>;
    if is_rain {
        motion = P.rain.xyz * (0.85 + 0.3 * fract(r * 7.13)) * t;
        out.kind = 0u;
    } else {
        // Snow drifts with the wind and wobbles as it falls at about a metre a second.
        let phase = r * 40.0;
        let fall = 0.8 + 0.5 * fract(r * 3.7);
        motion = vec3<f32>(
            P.wind.x * t + 0.4 * sin(t * 1.3 + phase),
            -fall * t,
            P.wind.z * t + 0.4 * cos(t * 1.1 + phase * 1.7),
        );
        out.kind = 1u;
    }
    // Wrap into the box around the camera.
    let rel = fract((base + motion - P.cam_mod.xyz) / box_size) * box_size - box_size * 0.5;
    // Fade toward the box edges so wrapping never pops.
    let d = length(rel.xz) / (box_size.x * 0.5);
    let dy = abs(rel.y) / (box_size.y * 0.5);
    out.alpha = (1.0 - smoothstep(0.55, 1.0, d)) * (1.0 - smoothstep(0.6, 1.0, dy)) * P.box_size.w;
    // Hidden under anything that blocks the sky.
    let map_xz = P.cam.xz + rel.xz;
    let size = i32(P.cam.w);
    let texel = vec2<i32>(floor(map_xz));
    var hidden = out.alpha <= 0.0;
    if all(texel >= vec2<i32>(0)) && all(texel < vec2<i32>(size)) {
        let top = textureLoad(heights, texel, 0).r;
        hidden = hidden || P.cam.y + rel.y < f32(top) + 1.0;
    }
    if hidden {
        // Outside the clip volume: the triangle is dropped.
        out.pos = vec4<f32>(-2.0, -2.0, 0.5, 1.0);
        return out;
    }
    var p: vec3<f32>;
    if is_rain {
        // A streak along the fall velocity (motion blur), a centimetre or so wide.
        let axis = normalize(P.rain.xyz);
        let side = normalize(cross(axis, rel));
        p = rel + axis * corner.y * 0.25 + side * corner.x * 0.007;
    } else {
        p = rel + (P.right.xyz * corner.x + P.up.xyz * corner.y) * 0.03;
    }
    out.pos = P.view_proj * vec4<f32>(p, 1.0);
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    var a: f32;
    var col: vec3<f32>;
    if in.kind == 0u {
        // Drops refract the sky around them: about as bright as the sky, soft across and
        // tapered along the streak.
        a = (1.0 - in.uv.x * in.uv.x) * (1.0 - in.uv.y * in.uv.y) * 0.35;
        col = (P.ambient.rgb * 0.9 + P.direct.rgb * 0.2) / 3.14159265;
    } else {
        let r2 = dot(in.uv, in.uv);
        if r2 > 1.0 {
            discard;
        }
        a = 1.0 - r2 * r2;
        col = 0.9 * (P.ambient.rgb * 0.8 + P.direct.rgb * 0.5) / 3.14159265;
    }
    return vec4<f32>(col, a * in.alpha);
}
