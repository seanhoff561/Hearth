// The sky pass: a full-screen triangle at the far plane drawing the sky-view LUT, the sun and
// moon discs, stars and a cloud layer. Output is pre-exposed HDR radiance.

const PI: f32 = 3.14159265;
const SUN_RADIUS: f32 = 0.00467;
const MOON_RADIUS: f32 = 0.00452;

struct Params {
    sun: vec4<f32>,
    moon: vec4<f32>,
    camera: vec4<f32>,
    inv_view_proj: mat4x4<f32>,
    stars0: vec4<f32>,
    stars1: vec4<f32>,
    stars2: vec4<f32>,
    clouds: vec4<f32>,
    // x: star visibility, y: exposure, z: night factor, w: moon disc radiance (pre-exposed).
    misc: vec4<f32>,
    // rgb: direct sun + moon illuminance at the camera (pre-exposed); w: unused.
    direct: vec4<f32>,
    // rgb: sky irradiance on a horizontal surface (pre-exposed); w: unused.
    ambient: vec4<f32>,
    // rgb: grey of an overcast sky (pre-exposed); w: how far it replaces the clear sky.
    overcast: vec4<f32>,
};

@group(0) @binding(0) var<uniform> P: Params;
@group(0) @binding(1) var skyview: texture_2d<f32>;
@group(0) @binding(2) var transmittance_lut: texture_2d<f32>;
// Wraps in azimuth (sky view).
@group(0) @binding(3) var lut_sampler: sampler;
// Clamps (transmittance: its u edges are the zenith and the horizon).
@group(0) @binding(4) var clamp_sampler: sampler;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) ndc: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) i: u32) -> VsOut {
    let uv = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    var out: VsOut;
    out.ndc = uv * 2.0 - 1.0;
    // Reverse-Z: the far plane is depth 0.
    out.pos = vec4<f32>(out.ndc, 0.0, 1.0);
    return out;
}

// Sky-view lookup for directions at or above the horizon (never blends in the ground rows).
fn skyview_uv(dir: vec3<f32>) -> vec2<f32> {
    let az = atan2(dir.x, -dir.z);
    let u = fract(az / (2.0 * PI) + 1.0);
    let el = asin(clamp(dir.y, 0.0, 1.0));
    let t = el / (PI / 2.0);
    let v = max(0.5 + 0.5 * sqrt(t), 0.5 + 1.0 / 128.0);
    return vec2<f32>(u, v);
}

fn hash3(p: vec3<f32>) -> f32 {
    var q = fract(p * vec3<f32>(0.1031, 0.1030, 0.0973));
    q += dot(q, q.yxz + 33.33);
    return fract((q.x + q.y) * q.z);
}

fn hash2(p: vec2<f32>) -> f32 {
    var q = fract(p * vec2<f32>(0.1031, 0.1030));
    q += dot(q, q.yx + 33.33);
    return fract((q.x + q.y) * q.x);
}

fn noise2(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    let a = hash2(i);
    let b = hash2(i + vec2<f32>(1.0, 0.0));
    let c = hash2(i + vec2<f32>(0.0, 1.0));
    let d = hash2(i + vec2<f32>(1.0, 1.0));
    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}

fn fbm2(p: vec2<f32>) -> f32 {
    var s = 0.0;
    var a = 0.5;
    var q = p;
    for (var i = 0; i < 5; i++) {
        s += noise2(q) * a;
        q = q * 2.03 + vec2<f32>(17.0, 31.0);
        a *= 0.5;
    }
    return s;
}

// Stars: a fixed celestial pattern, visible when the sky is dark.
fn stars(dir: vec3<f32>) -> vec3<f32> {
    if P.misc.x <= 0.001 || dir.y < -0.05 {
        return vec3<f32>(0.0);
    }
    // World → celestial (the rotation's transpose).
    let cel = vec3<f32>(dot(dir, P.stars0.xyz), dot(dir, P.stars1.xyz), dot(dir, P.stars2.xyz));
    let scale = 420.0;
    let cell = floor(cel * scale);
    let h = hash3(cell);
    if h > 0.012 {
        return vec3<f32>(0.0);
    }
    // Position of the star inside its cell; brightness by a steep magnitude distribution.
    let offs = vec3<f32>(hash3(cell + 1.7), hash3(cell + 3.1), hash3(cell + 5.3));
    let star_dir = normalize((cell + offs) / scale);
    let d = length(cel - star_dir) * scale;
    let core = exp(-d * d * 10.0);
    let mag = pow(h / 0.012, 3.0);
    let tint = mix(vec3<f32>(0.75, 0.85, 1.0), vec3<f32>(1.0, 0.85, 0.7), hash3(cell + 9.9));
    let twinkle = 0.8 + 0.2 * sin(P.camera.w * (3.0 + h * 400.0) + h * 1000.0);
    // Extinction toward the horizon.
    let ext = smoothstep(-0.02, 0.25, dir.y);
    return tint * core * (0.02 + mag * 0.6) * twinkle * ext * P.misc.x;
}

fn clouds(dir: vec3<f32>, sky: vec3<f32>) -> vec4<f32> {
    let cover = P.clouds.x;
    let base = P.clouds.y;
    if cover <= 0.01 || dir.y <= 0.01 || base <= 0.0 {
        return vec4<f32>(0.0);
    }
    let t = base / dir.y;
    let hit = dir.xz * t;
    let dist = length(hit);
    let p = (hit + P.clouds.zw) / 900.0;
    let n = fbm2(p) * 0.75 + fbm2(p * 3.1 + 7.0) * 0.25;
    let density = smoothstep(1.0 - cover, 1.0 - cover + 0.25, n + cover * 0.35);
    if density <= 0.0 {
        return vec4<f32>(0.0);
    }
    // A diffuse white layer: lit tops, darker dense cores, a bright rim toward the sun.
    let sun_c = max(dot(dir, P.sun.xyz), 0.0);
    let thickness = smoothstep(0.0, 1.0, density);
    let direct = P.direct.rgb * (0.3 + 0.7 * (1.0 - thickness)) * (1.0 + 1.5 * pow(sun_c, 12.0));
    let col = (direct + P.ambient.rgb * (1.0 - 0.4 * thickness)) * (0.8 / PI);
    // Distant clouds blend into the haze toward the horizon.
    let fade = 1.0 - smoothstep(6000.0, 20000.0, dist);
    return vec4<f32>(mix(sky, col, fade), density * max(fade, 0.35));
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    // Unproject a point on the near plane (depth 1 in reverse-Z); the far plane of an
    // infinite projection is at infinity (w = 0). The camera sits at the origin.
    let clip = P.inv_view_proj * vec4<f32>(in.ndc, 1.0, 1.0);
    let dir = normalize(clip.xyz / clip.w);
    var col = textureSampleLevel(skyview, lut_sampler, skyview_uv(dir), 0.0).rgb;
    // Below the horizon lies land and sea beyond the loaded area: it dissolves into the same
    // horizon haze as the terrain's edge fog (until distant terrain is drawn).
    if dir.y < 0.0 {
        col *= mix(1.0, 0.8, 1.0 - smoothstep(-0.4, 0.0, dir.y));
    }
    // Under a thick cloud deck, in rain or snow, the whole sky down to the horizon is grey.
    let overcast = P.overcast.w;
    col = mix(col, P.overcast.rgb, overcast);
    col += stars(dir) * (1.0 - overcast);
    // The sun and moon set behind the planet's horizon (a little below the horizontal plane
    // when the camera is high up).
    let r = 6360000.0 + max(P.camera.x, 1.0);
    let s_g = 6360000.0 / r;
    let open_sky = select(0.0, 1.0, dir.y > -sqrt(max(1.0 - s_g * s_g, 0.0)));
    // Sun disc with limb darkening.
    let sc = dot(dir, P.sun.xyz);
    let sun_d = acos(clamp(sc, -1.0, 1.0));
    if sun_d < SUN_RADIUS && P.sun.w > 0.0 && open_sky > 0.0 {
        let limb = 1.0 - 0.6 * (1.0 - sqrt(max(1.0 - (sun_d / SUN_RADIUS) * (sun_d / SUN_RADIUS), 0.0)));
        let trans = textureSampleLevel(transmittance_lut, clamp_sampler, transmittance_uv(r, P.sun.y), 0.0).rgb;
        // The true disc radiance would overflow half floats; it is scaled to a very bright
        // but finite value (it saturates after tonemapping either way).
        col += trans * P.sun.w / (PI * SUN_RADIUS * SUN_RADIUS) * limb * 0.0005 * (1.0 - overcast);
    }
    // Moon disc: lit by the sun according to the phase.
    let mc = dot(dir, P.moon.xyz);
    let moon_d = acos(clamp(mc, -1.0, 1.0));
    if moon_d < MOON_RADIUS && open_sky > 0.0 {
        // Normal on the moon's sphere facing us.
        let offset = (dir - P.moon.xyz * mc) / MOON_RADIUS;
        let n = normalize(offset - P.moon.xyz * sqrt(max(1.0 - dot(offset, offset), 0.0)));
        let lit = max(dot(-n, -P.sun.xyz), 0.0);
        // Maria and highlands as a coarse albedo pattern.
        let albedo = 0.8 + 0.4 * hash2(floor(offset.xz * 9.0));
        col += (vec3<f32>(0.95, 0.93, 0.88) * albedo * lit * P.misc.w + vec3<f32>(0.004) * P.misc.w) * (1.0 - overcast);
    }
    let cl = clouds(dir, col);
    col = mix(col, cl.rgb, cl.a);
    return vec4<f32>(col, 1.0);
}

// Same parameterisation as atmosphere.wgsl.
fn transmittance_uv(r: f32, mu: f32) -> vec2<f32> {
    let R_GROUND = 6360000.0;
    let R_TOP = 6460000.0;
    let H = sqrt(R_TOP * R_TOP - R_GROUND * R_GROUND);
    let rho = sqrt(max(r * r - R_GROUND * R_GROUND, 0.0));
    let disc = r * r * (mu * mu - 1.0) + R_TOP * R_TOP;
    let d = max(0.0, -r * mu + sqrt(max(disc, 0.0)));
    let d_min = R_TOP - r;
    let d_max = rho + H;
    return vec2<f32>((d - d_min) / max(d_max - d_min, 1e-3), rho / H);
}
