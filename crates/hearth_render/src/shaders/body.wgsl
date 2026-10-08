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
    palette: array<mat4x4<f32>, 17>,
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
    @location(2) tissue: vec2<f32>,
};

@vertex
fn vs_main(v: VsIn) -> VsOut {
    var m = mat4x4<f32>(vec4<f32>(0.0), vec4<f32>(0.0), vec4<f32>(0.0), vec4<f32>(0.0));
    for (var k = 0u; k < 4u; k++) {
        let j = (v.joints >> (8u * k)) & 255u;
        m += u.palette[j] * v.weights[k];
    }
    let world = (m * vec4<f32>(v.pos, 1.0)).xyz;
    var out: VsOut;
    out.pos = u.view_proj * vec4<f32>(world, 1.0);
    out.normal = normalize((m * vec4<f32>(v.normal, 0.0)).xyz);
    out.world = world;
    out.tissue = v.tissue.xy;
    return out;
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

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let n = normalize(in.normal);
    let v = normalize(u.eye.xyz - in.world);
    let l = u.light_dir.xyz;
    // Skin's two lobes (rough 0.48 and 0.25, mixed 0.85 : 0.15), its Fresnel at normal
    // incidence some 0.028 (index 1.4); lips a little glossier, nails smooth.
    let lips = in.tissue.x;
    let nail = in.tissue.y;
    var albedo = mix(mix(u.skin.rgb, u.lips.rgb, lips), u.nail.rgb, nail);
    let rough = mix(mix(vec2<f32>(0.48, 0.25), vec2<f32>(0.38, 0.18), lips), vec2<f32>(0.25, 0.12), nail);
    let f0 = mix(0.028, 0.04, nail);
    // Light under the skin: the terminator wrapped by how far each colour scatters.
    let nl = dot(n, l);
    let wrap = vec3<f32>(0.42, 0.22, 0.16);
    let diffuse = max((vec3<f32>(nl) + wrap) / (1.0 + wrap), vec3<f32>(0.0));
    let hemi = mix(u.ground.rgb, u.sky.rgb, 0.5 + 0.5 * n.y);
    // The ambient also scatters: a little more red in the shade.
    let ambient = hemi * vec3<f32>(1.05, 0.98, 0.96);
    var lit = albedo * (u.light.rgb * diffuse + ambient) / 3.14159265;
    let spec = 0.85 * ggx(n, v, l, rough.x, f0) + 0.15 * ggx(n, v, l, rough.y, f0);
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
