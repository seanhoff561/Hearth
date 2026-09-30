// Physically based sky (after Hillaire 2020, "A Scalable and Production Ready Sky and
// Atmosphere Rendering Technique"): Rayleigh + Mie + ozone on an Earth-sized planet, with
// lookup tables for transmittance, multiple scattering and the sky as seen from the camera.
// Distances are metres (1 block = 1 m); radiance is per unit illuminance of the light source,
// multiplied by pre-exposed illuminances in the sky-view pass.

const PI: f32 = 3.14159265;
const R_GROUND: f32 = 6360000.0;
const R_TOP: f32 = 6460000.0;
const RAYLEIGH_SCATTER: vec3<f32> = vec3<f32>(5.802e-6, 13.558e-6, 33.1e-6);
const RAYLEIGH_H: f32 = 8000.0;
const MIE_SCATTER: f32 = 3.996e-5;
const MIE_EXTINCT: f32 = 4.44e-5;
const MIE_H: f32 = 1200.0;
const MIE_G: f32 = 0.8;
const OZONE_ABSORB: vec3<f32> = vec3<f32>(0.650e-6, 1.881e-6, 0.085e-6);
const GROUND_ALBEDO: vec3<f32> = vec3<f32>(0.3);

struct Params {
    // xyz: direction to the sun; w: sun illuminance (pre-exposed).
    sun: vec4<f32>,
    // xyz: direction to the moon; w: moon illuminance (pre-exposed).
    moon: vec4<f32>,
    // x: camera altitude above sea level (m); y: haze multiplier on Mie; z: moon phase; w: time (s).
    camera: vec4<f32>,
    // Inverse view-projection (camera-relative), for the sky pass.
    inv_view_proj: mat4x4<f32>,
    // Celestial → world rotation for stars (columns), w unused.
    stars0: vec4<f32>,
    stars1: vec4<f32>,
    stars2: vec4<f32>,
    // x: cloud cover 0..1, y: cloud base height (blocks, camera-relative), z,w: wind offset.
    clouds: vec4<f32>,
    // x: star visibility (0..1), y: exposure, z: night factor, w: moon disc radiance.
    misc: vec4<f32>,
    direct: vec4<f32>,
    ambient: vec4<f32>,
    overcast: vec4<f32>,
};

@group(0) @binding(0) var<uniform> P: Params;
@group(0) @binding(1) var transmittance_lut: texture_2d<f32>;
@group(0) @binding(2) var multiscatter_lut: texture_2d<f32>;
// Clamps at the edges (the transmittance table's u edges are the zenith and the horizon).
@group(0) @binding(3) var lut_sampler: sampler;
@group(0) @binding(4) var out_lut: texture_storage_2d<rgba16float, write>;

struct Medium {
    scatter: vec3<f32>,
    extinct: vec3<f32>,
    rayleigh: vec3<f32>,
    mie: f32,
};

fn medium(h: f32) -> Medium {
    let rd = exp(-h / RAYLEIGH_H);
    let md = exp(-h / MIE_H) * P.camera.y;
    let od = max(0.0, 1.0 - abs(h - 25000.0) / 15000.0);
    var m: Medium;
    m.rayleigh = RAYLEIGH_SCATTER * rd;
    m.mie = MIE_SCATTER * md;
    m.scatter = m.rayleigh + vec3<f32>(m.mie);
    m.extinct = m.rayleigh + vec3<f32>(MIE_EXTINCT * md) + OZONE_ABSORB * od;
    return m;
}

// Distance along a ray from radius r with cosine mu to a sphere of radius R (or -1).
fn ray_sphere(r: f32, mu: f32, R: f32) -> f32 {
    let b = r * mu;
    let c = r * r - R * R;
    let disc = b * b - c;
    if disc < 0.0 {
        return -1.0;
    }
    let s = sqrt(disc);
    let t0 = -b - s;
    let t1 = -b + s;
    if t0 > 0.0 {
        return t0;
    }
    if t1 > 0.0 {
        return t1;
    }
    return -1.0;
}

// Bruneton's transmittance parameterisation.
fn transmittance_uv(r: f32, mu: f32) -> vec2<f32> {
    let H = sqrt(R_TOP * R_TOP - R_GROUND * R_GROUND);
    let rho = sqrt(max(r * r - R_GROUND * R_GROUND, 0.0));
    let disc = r * r * (mu * mu - 1.0) + R_TOP * R_TOP;
    let d = max(0.0, -r * mu + sqrt(max(disc, 0.0)));
    let d_min = R_TOP - r;
    let d_max = rho + H;
    return vec2<f32>((d - d_min) / max(d_max - d_min, 1e-3), rho / H);
}

fn transmittance_rmu(uv: vec2<f32>) -> vec2<f32> {
    let H = sqrt(R_TOP * R_TOP - R_GROUND * R_GROUND);
    let rho = H * uv.y;
    let r = sqrt(rho * rho + R_GROUND * R_GROUND);
    let d_min = R_TOP - r;
    let d_max = rho + H;
    let d = d_min + uv.x * (d_max - d_min);
    var mu = 1.0;
    if d > 0.0 {
        mu = clamp((H * H - rho * rho - d * d) / (2.0 * r * d), -1.0, 1.0);
    }
    return vec2<f32>(r, mu);
}

fn transmittance(r: f32, mu: f32) -> vec3<f32> {
    return textureSampleLevel(transmittance_lut, lut_sampler, transmittance_uv(r, mu), 0.0).rgb;
}

// 1 where a light in direction cosine `mu` is above the planet's horizon at radius `r`, 0 in
// the planet's shadow, blended across the sun's angular radius. (The transmittance table only
// covers rays that miss the ground and clamps below the horizon.)
fn planet_shadow(r: f32, mu: f32) -> f32 {
    let s = R_GROUND / r;
    let mu_h = -sqrt(max(1.0 - s * s, 0.0));
    return smoothstep(mu_h - 0.0047, mu_h + 0.0047, mu);
}

fn rayleigh_phase(c: f32) -> f32 {
    return 3.0 / (16.0 * PI) * (1.0 + c * c);
}

fn mie_phase(c: f32) -> f32 {
    // Cornette-Shanks.
    let g = MIE_G;
    let k = 3.0 / (8.0 * PI) * (1.0 - g * g) / (2.0 + g * g);
    return k * (1.0 + c * c) / pow(1.0 + g * g - 2.0 * g * c, 1.5);
}

// ---------------------------------------------------------------- transmittance LUT (256×64)
@compute @workgroup_size(8, 8)
fn transmittance_main(@builtin(global_invocation_id) id: vec3<u32>) {
    let size = textureDimensions(out_lut);
    if id.x >= size.x || id.y >= size.y {
        return;
    }
    let uv = (vec2<f32>(id.xy) + 0.5) / vec2<f32>(size);
    let rm = transmittance_rmu(uv);
    let r = rm.x;
    let mu = rm.y;
    let t_max = max(ray_sphere(r, mu, R_TOP), 0.0);
    let steps = 40;
    let dt = t_max / f32(steps);
    var depth = vec3<f32>(0.0);
    for (var i = 0; i < steps; i++) {
        let t = (f32(i) + 0.5) * dt;
        let h = sqrt(r * r + t * t + 2.0 * r * mu * t) - R_GROUND;
        depth += medium(h).extinct * dt;
    }
    textureStore(out_lut, vec2<i32>(id.xy), vec4<f32>(exp(-depth), 1.0));
}

// ---------------------------------------------------------------- multiple scattering (32×32)
@compute @workgroup_size(8, 8)
fn multiscatter_main(@builtin(global_invocation_id) id: vec3<u32>) {
    let size = textureDimensions(out_lut);
    if id.x >= size.x || id.y >= size.y {
        return;
    }
    let uv = (vec2<f32>(id.xy) + 0.5) / vec2<f32>(size);
    let sun_mu = uv.x * 2.0 - 1.0;
    let r = R_GROUND + uv.y * (R_TOP - R_GROUND) + 1.0;
    let sun = vec3<f32>(sqrt(max(1.0 - sun_mu * sun_mu, 0.0)), sun_mu, 0.0);
    var lum = vec3<f32>(0.0);
    var fms = vec3<f32>(0.0);
    let n_dir = 8;
    for (var a = 0; a < n_dir; a++) {
        for (var b = 0; b < n_dir; b++) {
            let theta = PI * (f32(a) + 0.5) / f32(n_dir);
            let phi = 2.0 * PI * (f32(b) + 0.5) / f32(n_dir);
            let dir = vec3<f32>(sin(theta) * cos(phi), cos(theta), sin(theta) * sin(phi));
            let mu = dir.y;
            let t_ground = ray_sphere(r, mu, R_GROUND);
            let t_top = ray_sphere(r, mu, R_TOP);
            var t_max = t_top;
            if t_ground > 0.0 {
                t_max = t_ground;
            }
            t_max = max(t_max, 0.0);
            let steps = 20;
            let dt = t_max / f32(steps);
            var trans = vec3<f32>(1.0);
            var l = vec3<f32>(0.0);
            var f = vec3<f32>(0.0);
            for (var i = 0; i < steps; i++) {
                let t = (f32(i) + 0.5) * dt;
                let p = vec3<f32>(0.0, r, 0.0) + dir * t;
                let pr = length(p);
                let m = medium(pr - R_GROUND);
                let step_t = exp(-m.extinct * dt);
                let up = p / pr;
                let lmu = dot(up, sun);
            let sun_t = transmittance(pr, lmu) * planet_shadow(pr, lmu);
                let isotropic = 1.0 / (4.0 * PI);
                let s = m.scatter * isotropic * sun_t;
                let integral = (s - s * step_t) / max(m.extinct, vec3<f32>(1e-9));
                l += trans * integral;
                let fi = (m.scatter - m.scatter * step_t) / max(m.extinct, vec3<f32>(1e-9));
                f += trans * fi;
                trans *= step_t;
            }
            if t_ground > 0.0 {
                let p = vec3<f32>(0.0, r, 0.0) + dir * t_ground;
                let up = normalize(p);
                l += trans * transmittance(length(p), dot(up, sun)) * max(dot(up, sun), 0.0) * GROUND_ALBEDO / PI;
            }
            let w = sin(theta) * (PI / f32(n_dir)) * (2.0 * PI / f32(n_dir)) / (4.0 * PI);
            lum += l * w;
            fms += f * w;
        }
    }
    let psi = lum / (vec3<f32>(1.0) - fms);
    // Stored as a logarithm: in twilight it falls tenfold every degree or two, and linear
    // interpolation would smear daylight into the dusk.
    textureStore(out_lut, vec2<i32>(id.xy), vec4<f32>(log(max(psi, vec3<f32>(1e-30))), 1.0));
}

fn multiscatter(r: f32, sun_mu: f32) -> vec3<f32> {
    let uv = vec2<f32>(sun_mu * 0.5 + 0.5, (r - R_GROUND) / (R_TOP - R_GROUND));
    return exp(textureSampleLevel(multiscatter_lut, lut_sampler, uv, 0.0).rgb);
}

// Radiance along a view ray from altitude `h` (single + multiple scattering) from one light.
fn scatter_ray(r: f32, dir: vec3<f32>, light: vec3<f32>, steps: i32) -> vec3<f32> {
    let mu = dir.y;
    let t_ground = ray_sphere(r, mu, R_GROUND);
    let t_top = ray_sphere(r, mu, R_TOP);
    var t_max = t_top;
    if t_ground > 0.0 {
        t_max = t_ground;
    }
    if t_max <= 0.0 {
        return vec3<f32>(0.0);
    }
    let c = dot(dir, light);
    let ph_r = rayleigh_phase(c);
    let ph_m = mie_phase(c);
    var trans = vec3<f32>(1.0);
    var l = vec3<f32>(0.0);
    // Exponentially distributed samples (denser near the camera).
    var t_prev = 0.0;
    for (var i = 0; i < steps; i++) {
        let f = (f32(i) + 0.5) / f32(steps);
        let t = t_max * f * f;
        let dt = max(t - t_prev, 1.0);
        t_prev = t;
        let p = vec3<f32>(0.0, r, 0.0) + dir * t;
        let pr = length(p);
        let m = medium(pr - R_GROUND);
        let up = p / pr;
        let light_mu = dot(up, light);
        let sun_t = transmittance(pr, light_mu) * planet_shadow(pr, light_mu);
        let single = (m.rayleigh * ph_r + vec3<f32>(m.mie * ph_m)) * sun_t;
        let ms = m.scatter * multiscatter(pr, light_mu);
        let s = single + ms;
        let step_t = exp(-m.extinct * dt);
        l += trans * (s - s * step_t) / max(m.extinct, vec3<f32>(1e-9));
        trans *= step_t;
    }
    return l;
}

// Sky-view mapping: u = azimuth (world, from north clockwise), v = elevation with more texels
// near the horizon.
fn skyview_dir(uv: vec2<f32>) -> vec3<f32> {
    let az = uv.x * 2.0 * PI;
    let v = uv.y * 2.0 - 1.0;
    let el = sign(v) * v * v * (PI / 2.0);
    let c = cos(el);
    // North is −Z, east is +X.
    return vec3<f32>(sin(az) * c, sin(el), -cos(az) * c);
}

// ---------------------------------------------------------------- sky-view LUT (256×128)
@compute @workgroup_size(8, 8)
fn skyview_main(@builtin(global_invocation_id) id: vec3<u32>) {
    let size = textureDimensions(out_lut);
    if id.x >= size.x || id.y >= size.y {
        return;
    }
    let uv = (vec2<f32>(id.xy) + 0.5) / vec2<f32>(size);
    let dir = skyview_dir(uv);
    let r = R_GROUND + max(P.camera.x, 1.0);
    var l = scatter_ray(r, dir, P.sun.xyz, 24) * P.sun.w;
    if P.moon.w > 0.0 {
        l += scatter_ray(r, dir, P.moon.xyz, 16) * P.moon.w;
    }
    // Stay inside half-float range whatever the exposure.
    textureStore(out_lut, vec2<i32>(id.xy), vec4<f32>(min(l, vec3<f32>(60000.0)), 1.0));
}
