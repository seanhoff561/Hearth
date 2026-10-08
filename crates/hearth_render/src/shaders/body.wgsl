// A person's skin (Amendment E §8, E7): the anatomy's mesh carried by its joints (linear blend
// skinning over the 17-joint palette), shaded as skin: light that scatters under it (a
// terminator wrapped wider for red than for green and blue, as the red travels furthest in the
// dermis) and two specular lobes for its oily, finely rough surface; lips redder and a little
// glossier; nails pale and smooth.

struct Body {
    view_proj: mat4x4<f32>,
    eye: vec4<f32>,
    light_dir: vec4<f32>,
    light: vec4<f32>,
    sky: vec4<f32>,
    ground: vec4<f32>,
    exposure: vec4<f32>,
    skin: vec4<f32>,
    lips: vec4<f32>,
    nail: vec4<f32>,
    hair: vec4<f32>,
    palette: array<mat4x4<f32>, 17>,
    // The skin's state: x wet, y tan, z sunburn, w pallor; then flush, goosebumps.
    state: vec4<f32>,
    state2: vec4<f32>,
    // Per joint: dirt, blood, scar, and how covered (no sun there).
    marks: array<vec4<f32>, 17>,
    // The garment's colour (linear); w: 0 hide, 1 plant fibre.
    cloth: vec4<f32>,
};

@group(0) @binding(0) var<uniform> u: Body;

struct VsIn {
    @location(0) pos: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) joints: u32,
    @location(3) weights: vec4<f32>,
    @location(4) tissue: vec4<f32>,
};

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) normal: vec3<f32>,
    @location(1) world: vec3<f32>,
    @location(2) tissue: vec4<f32>,
    @location(3) bind: vec3<f32>,
    @location(4) marks: vec4<f32>,
    @location(5) @interpolate(flat) head: f32,
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
    out.pos = u.view_proj * vec4<f32>(world, 1.0);
    out.normal = normalize((m * vec4<f32>(v.normal, 0.0)).xyz);
    out.world = world;
    out.tissue = v.tissue;
    out.bind = v.pos;
    out.marks = marks;
    out.head = select(0.0, 1.0, (v.joints & 255u) == 4u);
    return out;
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

fn tonemap(x: vec3<f32>) -> vec3<f32> {
    return clamp(x * (2.51 * x + 0.03) / (x * (2.43 * x + 0.59) + 0.14), vec3<f32>(0.0), vec3<f32>(1.0));
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
    let hemi = mix(u.ground.rgb, u.sky.rgb, 0.5 + 0.5 * n.y);
    let diffuse = max((dot(n, l) + 0.1) / 1.1, 0.0);
    var lit = albedo * (u.light.rgb * diffuse + hemi) / 3.14159265;
    lit += u.light.rgb * ggx(n, v, l, rough, 0.03);
    var shown = tonemap(lit * u.exposure.x);
    if u.exposure.y > 0.5 {
        shown = pow(shown, vec3<f32>(1.0 / 2.2));
    }
    return vec4<f32>(shown, 1.0);
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let n = normalize(in.normal);
    let v = normalize(u.eye.xyz - in.world);
    let l = u.light_dir.xyz;
    if in.tissue.w > 0.5 {
        return garment(in, n, v, l);
    }
    // Skin's two lobes (rough 0.48 and 0.25, mixed 0.85 : 0.15), its Fresnel at normal
    // incidence some 0.028 (index 1.4); lips a little glossier, nails smooth.
    let lips = in.tissue.x;
    let nail = in.tissue.y;
    var albedo = mix(mix(u.skin.rgb, u.lips.rgb, lips), u.nail.rgb, nail);
    // Short hair over the skin: the hair's colour in a fine stipple of hairs, as thick as the
    // tissue's third weight says.
    let cell = floor(in.bind * 1400.0);
    let grain = fract(sin(dot(cell, vec3<f32>(12.9898, 78.233, 37.719))) * 43758.5453);
    let hairs = in.tissue.z * smoothstep(0.15, 0.6, grain + in.tissue.z * 0.35);
    albedo = mix(albedo, u.hair.rgb * 0.8, clamp(hairs, 0.0, 1.0));
    var rough = mix(mix(vec2<f32>(0.48, 0.25), vec2<f32>(0.38, 0.18), lips), vec2<f32>(0.25, 0.12), nail);
    let f0 = mix(0.028, 0.04, nail);
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
    // Dirt: patchy, thicker in its patches.
    let dabs = noise(in.bind * 22.0) * 0.6 + noise(in.bind * 70.0) * 0.4;
    let dirt = smoothstep(1.0 - in.marks.x, 1.1 - in.marks.x, dabs) * in.marks.x;
    albedo = mix(albedo, vec3<f32>(0.11, 0.075, 0.045), clamp(dirt, 0.0, 0.9));
    rough = mix(rough, vec2<f32>(0.75, 0.6), dirt);
    // Blood: streaks running down from where it came.
    let streak = noise(in.bind * vec3<f32>(60.0, 9.0, 60.0));
    let blood = smoothstep(1.0 - in.marks.y, 1.05 - in.marks.y, streak) * in.marks.y;
    albedo = mix(albedo, vec3<f32>(0.16, 0.01, 0.008), clamp(blood, 0.0, 0.95));
    // Wet: darker, and the water's film glossy over it all.
    let wet = clamp(u.state.x, 0.0, 1.0);
    albedo *= 1.0 - 0.15 * wet;
    rough = mix(rough, vec2<f32>(0.16, 0.08), wet);
    // Light under the skin: the terminator wrapped by how far each colour scatters.
    let nl = dot(n, l);
    let wrap = vec3<f32>(0.42, 0.22, 0.16);
    let diffuse = max((vec3<f32>(nl) + wrap) / (1.0 + wrap), vec3<f32>(0.0));
    let hemi = mix(u.ground.rgb, u.sky.rgb, 0.5 + 0.5 * n.y);
    // The ambient also scatters: a little more red in the shade.
    let ambient = hemi * vec3<f32>(1.05, 0.98, 0.96);
    var lit = albedo * (u.light.rgb * diffuse + ambient) / 3.14159265;
    // Hair over the skin scatters the sheen away.
    // Goosebumps roughen the surface's light a touch.
    let bumps = 1.0 - u.state2.y * 0.25 * noise(in.bind * 600.0);
    let spec = (0.85 * ggx(n, v, l, rough.x, f0) + 0.15 * ggx(n, v, l, rough.y, f0)) * (1.0 - 0.8 * hairs) * (1.0 + wet) * bumps;
    lit += u.light.rgb * spec;
    // The sky's sheen at grazing angles.
    let fres = f0 + (1.0 - f0) * pow(1.0 - max(dot(n, v), 0.0), 5.0);
    lit += hemi * fres * 0.08 / 3.14159265;
    var shown = tonemap(lit * u.exposure.x);
    if u.exposure.y > 0.5 {
        shown = pow(shown, vec3<f32>(1.0 / 2.2));
    }
    return vec4<f32>(shown, 1.0);
}
