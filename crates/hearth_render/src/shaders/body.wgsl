// A person's skin (Amendment E §8, E7; Amendment T §2.2): the anatomy's mesh carried by its
// joints (linear blend skinning over the 17-joint palette), shaded as measured skin is.
// - The light under it, pre-integrated through skin's diffusion profile by how sharply the
//   surface curves (`skin_lut`, from d'Eon and Luebke's six Gaussians): red carried round past
//   the terminator on an ear's rim or a nostril's wing, Lambert's law on a cheek.
// - Its surface by region (the vertex's `surface`): the T-zone oilier and smoother, the limbs and
//   hands drier and rougher. Weyrich et al. (2006) measured faces' highlights at Beckmann
//   roughness 0.3–0.5; here the main lobe's α runs 0.31 (oily) to 0.49 (dry), with a weaker,
//   broader sheen beside it (α 0.64–0.81), Fresnel at normal incidence 0.026–0.034 (index about
//   1.4, sebum's a little more), and the diffuse given only what the surface does not reflect.
// - Pores and fine furrows tilt the normal where they are coarser than a pixel; finer, their
//   slopes widen the highlight instead (so they neither shimmer nor vanish with distance).
// - Creases and pores shut out the sky's light and its reflection.
// - The sky reflected: its light in the mirror direction, by the lobe's directional albedo.
// - Wet: a thin, smooth film of water over it all, the skin's own sheen drowned under it.
// Lips are moister and a little glossier; nails smooth keratin, not scattering.

@group(1) @binding(3) var skin_lut: texture_2d<f32>;
@group(1) @binding(4) var skin_lut_s: sampler;

struct VsIn {
    @location(0) pos: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) joints: u32,
    @location(3) weights: vec4<f32>,
    @location(4) tissue: vec4<f32>,
    @location(5) surface: vec4<f32>,
};

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) normal: vec3<f32>,
    @location(1) world: vec3<f32>,
    @location(2) tissue: vec4<f32>,
    @location(3) bind: vec3<f32>,
    @location(4) marks: vec4<f32>,
    @location(5) @interpolate(flat) head: f32,
    @location(6) surface: vec4<f32>,
};

@vertex
fn vs_main(v: VsIn) -> VsOut {
    var m = mat4x4<f32>(vec4<f32>(0.0), vec4<f32>(0.0), vec4<f32>(0.0), vec4<f32>(0.0));
    var marks = vec4<f32>(0.0);
    for (var k = 0u; k < 4u; k++) {
        let j = (v.joints >> (8u * k)) & 255u;
        m += u.palette[j] * v.weights[k];
        marks += u.marks[j] * v.weights[k];
    }
    let world = (m * vec4<f32>(v.pos, 1.0)).xyz;
    var out: VsOut;
    out.pos = clip(world);
    out.normal = normalize((m * vec4<f32>(v.normal, 0.0)).xyz);
    out.world = world;
    out.tissue = v.tissue;
    out.bind = v.pos;
    out.marks = marks;
    out.head = select(0.0, 1.0, (v.joints & 255u) == 4u);
    out.surface = v.surface;
    return out;
}

// The body as it casts the sun's shadow: whole, its neck and head where they are posed even when
// the frame draws them elsewhere (in first person the eye is in the head).
@vertex
fn vs_shadow(v: VsIn) -> @builtin(position) vec4<f32> {
    var m = mat4x4<f32>(vec4<f32>(0.0), vec4<f32>(0.0), vec4<f32>(0.0), vec4<f32>(0.0));
    for (var k = 0u; k < 4u; k++) {
        let j = (v.joints >> (8u * k)) & 255u;
        var p = u.palette[j];
        if u.light.z > 0.5 && (j == 3u || j == 4u) {
            p = u.shadow_joints[j - 3u];
        }
        m += p * v.weights[k];
    }
    return clip((m * vec4<f32>(v.pos, 1.0)).xyz);
}

fn hash3(p: vec3<f32>) -> f32 {
    return fract(sin(dot(p, vec3<f32>(12.9898, 78.233, 37.719))) * 43758.5453);
}

// Smooth value noise in the bind pose's frame (moves with the skin).
fn noise(p: vec3<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let w = f * f * (3.0 - 2.0 * f);
    let a = mix(mix(hash3(i), hash3(i + vec3<f32>(1.0, 0.0, 0.0)), w.x),
                mix(hash3(i + vec3<f32>(0.0, 1.0, 0.0)), hash3(i + vec3<f32>(1.0, 1.0, 0.0)), w.x), w.y);
    let b = mix(mix(hash3(i + vec3<f32>(0.0, 0.0, 1.0)), hash3(i + vec3<f32>(1.0, 0.0, 1.0)), w.x),
                mix(hash3(i + vec3<f32>(0.0, 1.0, 1.0)), hash3(i + vec3<f32>(1.0, 1.0, 1.0)), w.x), w.y);
    return mix(a, b, w.z);
}


// An integer lattice point's hash, 0–1 (pcg3d: Jarzynski and Olano 2020), steady at any scale.
fn hash_i(c: vec3<i32>) -> vec3<f32> {
    var v = bitcast<vec3<u32>>(c) * 1664525u + vec3<u32>(1013904223u);
    v.x += v.y * v.z;
    v.y += v.z * v.x;
    v.z += v.x * v.y;
    v ^= v >> vec3<u32>(16u);
    v.x += v.y * v.z;
    v.y += v.z * v.x;
    v.z += v.x * v.y;
    return vec3<f32>(v) * (1.0 / 4294967296.0);
}

// Value noise and its gradient (x: the value, 0–1; yzw: its gradient).
fn noised(x: vec3<f32>) -> vec4<f32> {
    let i = vec3<i32>(floor(x));
    let w = fract(x);
    let u = w * w * (3.0 - 2.0 * w);
    let du = 6.0 * w * (1.0 - w);
    let a = hash_i(i).x;
    let b = hash_i(i + vec3<i32>(1, 0, 0)).x;
    let c = hash_i(i + vec3<i32>(0, 1, 0)).x;
    let d = hash_i(i + vec3<i32>(1, 1, 0)).x;
    let e = hash_i(i + vec3<i32>(0, 0, 1)).x;
    let f = hash_i(i + vec3<i32>(1, 0, 1)).x;
    let g = hash_i(i + vec3<i32>(0, 1, 1)).x;
    let h = hash_i(i + vec3<i32>(1, 1, 1)).x;
    let k1 = b - a;
    let k2 = c - a;
    let k3 = e - a;
    let k4 = a - b - c + d;
    let k5 = a - c - e + g;
    let k6 = a - b - e + f;
    let k7 = -a + b + c - d + e - f - g + h;
    let v = a + k1 * u.x + k2 * u.y + k3 * u.z + k4 * u.x * u.y + k5 * u.y * u.z
        + k6 * u.z * u.x + k7 * u.x * u.y * u.z;
    let grad = du * vec3<f32>(
        k1 + k4 * u.y + k6 * u.z + k7 * u.y * u.z,
        k2 + k5 * u.z + k4 * u.x + k7 * u.z * u.x,
        k3 + k6 * u.x + k5 * u.y + k7 * u.x * u.y,
    );
    return vec4<f32>(v, grad);
}

// The skin's relief at a point of the bind pose (metres) whose pixel spans `foot` metres:
// the height's gradient to tilt the normal by (slopes), the slope variance finer than the pixel
// (for the highlight's roughness), and how much light its hollows let reach it (1 open).
struct Relief {
    grad: vec3<f32>,
    hidden: f32,
    open: f32,
    // The coarse lines' own slope, which the light under the skin also feels.
    lines: vec3<f32>,
};

// How much of a relief of period `period` a pixel of `foot` resolves (1 all of it).
fn resolved(period: f32, foot: f32) -> f32 {
    return smoothstep(1.5 * foot, 3.5 * foot, period);
}

fn relief(p: vec3<f32>, foot: f32, oil: f32, fine: f32, bumps: f32) -> Relief {
    var r: Relief;
    r.grad = vec3<f32>(0.0);
    r.hidden = 0.0;
    r.open = 1.0;
    r.lines = vec3<f32>(0.0);
    // The micro-relief's furrows: a net of fine grooves some 0.7 mm across their cells, on every
    // skin, deeper where the lines are.
    let pf = 1400.0;
    let nf = noised(p * pf);
    // The height −af·ridge³ (grooves along the noise's mid contour).
    let ridge = 1.0 - abs(2.0 * nf.x - 1.0);
    let r3 = ridge * ridge * ridge;
    let af = (0.012 + 0.02 * fine) / pf;
    let gf = 6.0 * af * ridge * ridge * sign(2.0 * nf.x - 1.0) * nf.yzw * pf;
    let kf = resolved(1.0 / pf, foot);
    r.grad += gf * kf;
    r.hidden += (1.0 - kf) * (0.004 + 0.01 * fine);
    r.open *= mix(1.0 - 0.06 * fine, 1.0 - 0.2 * r3 * ridge * fine, kf);
    // Pores: one a cell of about 0.8 mm, open and larger where the skin is oily (the nose, the
    // cheeks), dimples a fifth of the cell across.
    let pp = 1250.0;
    let q = p * pp;
    let cell = vec3<i32>(floor(q));
    let o = vec3<f32>(cell) + 0.3 + 0.4 * hash_i(cell + vec3<i32>(7, 13, 29));
    let d = q - o;
    let rr = 0.11 + 0.08 * oil;
    let t = clamp(1.0 - dot(d, d) / (rr * rr), 0.0, 1.0);
    let depth = (0.004 + 0.012 * oil) / pp;
    let gp = depth * 4.0 * t * d / (rr * rr) * pp;
    let kp = resolved(1.0 / pp, foot);
    r.grad += gp * kp;
    r.hidden += (1.0 - kp) * (0.003 + 0.012 * oil);
    r.open *= mix(1.0 - 0.05 * oil, 1.0 - 0.45 * t * t, kp);
    // Fine lines some 3–5 mm apart where the skin creases with use (the forehead, the eyes'
    // corners, the knuckles), drawn out across the bind pose's height on the face.
    let pl = 260.0;
    let nl = noised(p * vec3<f32>(pl * 0.35, pl, pl * 0.35) + vec3<f32>(17.0, 3.0, 11.0));
    // The height −al·ridge⁴.
    let lr = 1.0 - abs(2.0 * nl.x - 1.0);
    let l4 = lr * lr * lr * lr;
    let al = 0.05 * max(fine - 0.3, 0.0) / pl;
    let gl = 8.0 * al * lr * lr * lr * sign(2.0 * nl.x - 1.0)
        * nl.yzw * vec3<f32>(pl * 0.35, pl, pl * 0.35);
    let kl = resolved(1.0 / pl, foot);
    r.lines = gl * kl;
    r.grad += gl * kl;
    r.hidden += (1.0 - kl) * 0.01 * max(fine - 0.3, 0.0);
    r.open *= 1.0 - 0.25 * l4 * max(fine - 0.3, 0.0) * kl;
    // Goosebumps: the hairs' roots raised, some 1.5 mm apart.
    if bumps > 0.0 {
        let pb = 650.0;
        let nb = noised(p * pb + vec3<f32>(5.0, 9.0, 1.0));
        let kb = resolved(1.0 / pb, foot);
        r.grad += bumps * 0.06 * nb.yzw * kb;
        r.hidden += bumps * (1.0 - kb) * 0.01;
    }
    return r;
}

// The normal `n` tilted by a height's gradient `g` (its part along `n` set aside).
fn tilt(n: vec3<f32>, g: vec3<f32>) -> vec3<f32> {
    return normalize(n - (g - n * dot(n, g)));
}

// A lobe's perceptual roughness widened by a slope variance finer than the pixel.
fn widen(rough: f32, hidden: f32) -> f32 {
    let a = rough * rough;
    return sqrt(sqrt(a * a + 2.0 * hidden));
}

// The share of the light a lobe reflects, over all directions, seen from `nv` (Karis's
// approximation of the split sum's environment term).
fn env_brdf(f0: f32, rough: f32, nv: f32) -> f32 {
    let r = rough * vec4<f32>(-1.0, -0.0275, -0.572, 0.022) + vec4<f32>(1.0, 0.0425, 1.04, -0.04);
    let a004 = min(r.x * r.x, exp2(-9.28 * nv)) * r.x + r.y;
    let ab = vec2<f32>(-1.04, 1.04) * a004 + r.zw;
    return f0 * ab.x + ab.y;
}

// GGX's normal distribution with Schlick's Fresnel and Smith's visibility, approximated.
fn ggx(n: vec3<f32>, v: vec3<f32>, l: vec3<f32>, rough: f32, f0: f32) -> f32 {
    let h = normalize(v + l);
    let nh = max(dot(n, h), 0.0);
    let nl = max(dot(n, l), 0.0);
    let nv = max(dot(n, v), 1e-3);
    let a = rough * rough;
    let a2 = a * a;
    let d = nh * nh * (a2 - 1.0) + 1.0;
    let ndf = a2 / (3.14159265 * d * d);
    let f = f0 + (1.0 - f0) * pow(1.0 - max(dot(v, h), 0.0), 5.0);
    let k = a * 0.5;
    let vis = 1.0 / ((nl * (1.0 - k) + k) * (nv * (1.0 - k) + k));
    return ndf * f * vis * 0.25 * nl;
}

// A garment: tanned hide (mottled, a little sheen at grazing angles) or plant fibre (twisted
// strands, matte).
fn garment(in: VsOut, n: vec3<f32>, v: vec3<f32>, l: vec3<f32>) -> vec4<f32> {
    var albedo = u.cloth.rgb;
    var rough = 0.7;
    if u.cloth.w < 0.5 {
        let mottle = noise(in.bind * 35.0) * 0.6 + noise(in.bind * 140.0) * 0.4;
        albedo *= 0.75 + 0.45 * mottle;
    } else {
        let twist = in.bind.y * 700.0 + noise(in.bind * 30.0) * 8.0 + in.bind.x * 120.0;
        albedo *= 0.65 + 0.4 * abs(sin(twist));
        rough = 0.9;
    }
    let hemi = ambient(n);
    let diffuse = max((dot(n, l) + 0.1) / 1.1, 0.0);
    var lit = albedo * (sun_rgb() * diffuse + hemi) / 3.14159265;
    lit += sun_rgb() * ggx(n, v, l, rough, 0.03);
    return finish(lit, in.world);
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    // The metres of skin a pixel spans (taken while every pixel of the quad is here).
    let foot = max(length(fwidth(in.bind)), 1e-5);
    // The surface's own normal: the light under the skin follows it, not the pores.
    let ng = normalize(in.normal);
    shade_sun(in.world, ng);
    let v = normalize(eye_pos() - in.world);
    let l = sun_dir();
    if in.tissue.w > 0.5 {
        return garment(in, ng, v, l);
    }
    let lips = in.tissue.x;
    let nail = in.tissue.y;
    let oil = in.surface.x;
    let shut = in.surface.y * (1.0 - nail);
    let curved = in.surface.z;
    let fine = in.surface.w;
    var albedo = mix(mix(u.skin.rgb, u.lips.rgb, lips), u.nail.rgb, nail);
    // Short hair over the skin: the hair's colour in a fine stipple of hairs, as thick as the
    // tissue's third weight says.
    let cell = floor(in.bind * 1400.0);
    let grain = fract(sin(dot(cell, vec3<f32>(12.9898, 78.233, 37.719))) * 43758.5453);
    let hairs = in.tissue.z * smoothstep(0.15, 0.6, grain + in.tissue.z * 0.35);
    albedo = mix(albedo, u.hair.rgb * 0.8, clamp(hairs, 0.0, 1.0));
    // The surface by region: the main lobe and the sheen beside it (rough enough that, the
    // pores and furrows widening it at a distance, the measured highlights result); lips moist,
    // nails smooth.
    var rough = vec2<f32>(mix(0.68, 0.51, oil), mix(0.90, 0.80, oil));
    rough = mix(rough, vec2<f32>(0.48, 0.70), lips);
    rough = mix(rough, vec2<f32>(0.30, 0.45), nail);
    let f0 = mix(mix(0.026, 0.034, oil), 0.04, nail);
    var sheen = mix(0.8, 1.15, oil) * (1.0 + 0.3 * lips);
    // The skin's state (E7 (e)). The sun: tanned and burnt where it reaches.
    let bare = 1.0 - clamp(in.marks.w, 0.0, 1.0);
    let skin_only = 1.0 - max(nail, hairs);
    albedo *= mix(vec3<f32>(1.0), vec3<f32>(0.74, 0.66, 0.58), u.state.y * bare * skin_only);
    albedo = mix(albedo, albedo * vec3<f32>(1.3, 0.62, 0.55), u.state.z * bare * skin_only);
    // Pallor (cold, blood lost): paler and greyer, the lips bluish; flush (heat, effort): the
    // face and chest redden.
    let grey = dot(albedo, vec3<f32>(0.3, 0.55, 0.15));
    albedo = mix(albedo, vec3<f32>(grey) * vec3<f32>(1.05, 1.03, 1.08), u.state.w * 0.45);
    albedo = mix(albedo, albedo * vec3<f32>(0.75, 0.8, 1.15), u.state.w * lips);
    albedo = mix(albedo, albedo * vec3<f32>(1.18, 0.82, 0.8), u.state2.x * (0.4 + 0.6 * in.head) * skin_only);
    // Scars: pale, a little shiny lines in a patch of the region.
    let scar_n = noise(in.bind * 18.0);
    let line = 1.0 - smoothstep(0.0, 0.06, abs(fract(dot(in.bind, vec3<f32>(31.0, 47.0, 13.0))) - 0.5) - 0.42);
    let scar = in.marks.z * smoothstep(0.55, 0.7, scar_n) * line;
    albedo = mix(albedo, albedo * vec3<f32>(1.15, 0.95, 0.95) + vec3<f32>(0.03, 0.02, 0.02), scar);
    rough = mix(rough, rough * 0.8, scar);
    // Dirt: patchy, thicker in its patches.
    let dabs = noise(in.bind * 22.0) * 0.6 + noise(in.bind * 70.0) * 0.4;
    let dirt = smoothstep(1.0 - in.marks.x, 1.1 - in.marks.x, dabs) * in.marks.x;
    albedo = mix(albedo, vec3<f32>(0.11, 0.075, 0.045), clamp(dirt, 0.0, 0.9));
    rough = mix(rough, vec2<f32>(0.85, 0.95), dirt);
    sheen *= 1.0 - 0.6 * dirt;
    // Blood: streaks running down from where it came.
    let streak = noise(in.bind * vec3<f32>(60.0, 9.0, 60.0));
    let blood = smoothstep(1.0 - in.marks.y, 1.05 - in.marks.y, streak) * in.marks.y;
    albedo = mix(albedo, vec3<f32>(0.16, 0.01, 0.008), clamp(blood, 0.0, 0.95));
    // Wet: darker under the film, the skin's relief filled.
    let wet = clamp(u.state.x, 0.0, 1.0);
    albedo *= 1.0 - 0.15 * wet;
    rough = mix(rough, rough * 0.85, wet);
    // The relief, as fine as a pixel shows it (a nail has none to speak of).
    let rel = relief(in.bind, foot, oil, fine, u.state2.y);
    let keep = (1.0 - nail) * (1.0 - 0.5 * dirt) * (1.0 - 0.6 * wet);
    let n = tilt(ng, rel.grad * keep);
    let r1 = widen(rough.x, rel.hidden * keep);
    let r2 = widen(rough.y, rel.hidden * keep);
    let nv = max(dot(n, v), 1e-3);
    // The light under the skin: by the curvature and the coarse lines' slope (the finer relief
    // is lost in the scattering); a nail lit as a hard surface.
    let nd = tilt(ng, rel.lines * keep * 0.5);
    let ndl = dot(nd, l);
    let scattered = textureSampleLevel(skin_lut, skin_lut_s, vec2<f32>(ndl * 0.5 + 0.5, curved), 0.0).rgb;
    let diffuse = mix(scattered, vec3<f32>(max(ndl, 0.0)), nail);
    // What the surface reflects is not scattered in.
    let reflected = env_brdf(f0, r1, nv) * sheen;
    let into = 1.0 - min(reflected, 0.5);
    // The sky's light, less in the creases; the ambient scatters a little red in the shade.
    let open = rel.open;
    let hemi = ambient(ng);
    let amb = hemi * vec3<f32>(1.05, 0.98, 0.96) * (1.0 - 0.7 * shut) * mix(1.0, open, 0.5);
    var lit = albedo * (sun_rgb() * diffuse * (1.0 - 0.25 * shut) * into + amb * into) / 3.14159265;
    // The sun's highlight: the main lobe and the broader sheen, under the hairs less.
    let veil = (1.0 - 0.8 * hairs) * sheen * open * (1.0 - 0.5 * shut);
    let spec = (0.85 * ggx(n, v, l, r1, f0) + 0.15 * ggx(n, v, l, r2, f0)) * veil;
    // The sky reflected, shut out in the creases more than the sun's light is.
    let rd = reflect(-v, n);
    let sky = ambient(rd) / 3.14159265;
    let sky_spec = sky * (0.85 * env_brdf(f0, r1, nv) + 0.15 * env_brdf(f0, r2, nv))
        * (1.0 - 0.8 * hairs) * sheen * open * (1.0 - 0.85 * shut);
    // Under a film of water the skin's own sheen is drowned (water against skin hardly reflects).
    let drown = 1.0 - 0.85 * wet;
    lit += (sun_rgb() * spec + sky_spec) * drown;
    // The film: smooth water over the relief (its own surface the skin's), what it reflects kept
    // from the skin beneath.
    if wet > 0.0 {
        let nvg = max(dot(ng, v), 1e-3);
        let film = 0.02 + 0.98 * pow(1.0 - nvg, 5.0);
        let water = sun_rgb() * ggx(ng, v, l, 0.12, 0.02)
            + ambient(reflect(-v, ng)) / 3.14159265 * env_brdf(0.02, 0.12, nvg) * (1.0 - 0.7 * shut);
        lit = lit * (1.0 - wet * film) + wet * water;
    }
    return finish(lit, in.world);
}
