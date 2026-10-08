// Hair cards (Amendment E §8, E7): strips riding the head joint and swung by the guide strands,
// each painted with strands here (straight, wavy, coiled, braided, locs, brows, beard), tested
// against their coverage, and shaded as hair: a diffuse wrapped about the strand, two highlights
// shifted along it (the white reflection toward the root, the coloured one through the fibre
// toward the tip), light through the hair from behind, and darker deeper in.

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
};

struct Hair {
    // Linear albedo; w: how wet (0–1).
    color: vec4<f32>,
    // Each guide's points' offsets from the style (32 guides of 5).
    guides: array<vec4<f32>, 160>,
};

@group(0) @binding(0) var<uniform> u: Body;
@group(0) @binding(1) var<uniform> hair: Hair;

const HEAD: u32 = 4u;
const GUIDE_POINTS: u32 = 5u;

struct VsIn {
    @location(0) pos: vec3<f32>,
    @location(1) tangent: vec3<f32>,
    @location(2) normal: vec3<f32>,
    @location(3) uv: vec2<f32>,
    @location(4) info: vec4<u32>,
};

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) world: vec3<f32>,
    @location(1) tangent: vec3<f32>,
    @location(2) normal: vec3<f32>,
    @location(3) uv: vec2<f32>,
    @location(4) @interpolate(flat) kind: u32,
    @location(5) depth: f32,
};

@vertex
fn vs_main(v: VsIn) -> VsOut {
    let m = u.palette[HEAD];
    var world = (m * vec4<f32>(v.pos, 1.0)).xyz;
    // The guide's swing at this far along it.
    if v.info.x < 32u {
        let a = f32(v.info.w) / 255.0 * f32(GUIDE_POINTS - 1u);
        let k = min(u32(floor(a)), GUIDE_POINTS - 2u);
        let f = a - f32(k);
        let base = v.info.x * GUIDE_POINTS + k;
        world += mix(hair.guides[base].xyz, hair.guides[base + 1u].xyz, f);
    }
    var out: VsOut;
    out.pos = u.view_proj * vec4<f32>(world, 1.0);
    out.world = world;
    out.tangent = normalize((m * vec4<f32>(v.tangent, 0.0)).xyz);
    out.normal = normalize((m * vec4<f32>(v.normal, 0.0)).xyz);
    out.uv = v.uv;
    out.kind = v.info.y;
    out.depth = f32(v.info.z) / 255.0;
    return out;
}

fn hash(x: f32) -> f32 {
    return fract(sin(x * 127.1 + 311.7) * 43758.5453);
}

fn tonemap(x: vec3<f32>) -> vec3<f32> {
    return clamp(x * (2.51 * x + 0.03) / (x * (2.43 * x + 0.59) + 0.14), vec3<f32>(0.0), vec3<f32>(1.0));
}

// A highlight along a fibre of direction `t`: strongest where the half vector is across it.
fn fibre(t: vec3<f32>, h: vec3<f32>, power: f32) -> f32 {
    let th = dot(t, h);
    return pow(sqrt(max(1.0 - th * th, 0.0)), power);
}

// How much of the card the strands cover here (0–1), and the strand's own shade.
fn coverage(kind: u32, uv: vec2<f32>, depth: f32) -> vec2<f32> {
    let v = uv.y;
    var n = 14.0;
    var s = uv.x;
    if kind == 5u {
        n = 6.0;
    } else if kind == 6u {
        n = 10.0;
    } else if kind == 2u {
        n = 18.0;
    }
    if kind == 3u {
        // A braid: three strands crossing in turn, the rope's round edge.
        let rope = 1.0 - smoothstep(0.38, 0.5, abs(uv.x - 0.5));
        let lobe = fract(uv.x * 1.5 + abs(fract(v * 26.0) - 0.5) * 1.6);
        let gap = smoothstep(0.0, 0.12, lobe) * smoothstep(1.0, 0.88, lobe);
        return vec2<f32>(rope, 0.6 + 0.4 * gap);
    }
    if kind == 4u {
        // A loc: a felted rope, its edge uneven.
        let edge = 0.44 + 0.05 * sin(v * 70.0 + uv.x * 3.0);
        let rope = 1.0 - smoothstep(edge - 0.06, edge, abs(uv.x - 0.5));
        let felt = 0.75 + 0.25 * hash(floor(v * 60.0) + floor(uv.x * 8.0) * 13.0);
        return vec2<f32>(rope, felt);
    }
    if kind == 1u {
        s += 0.06 * sin(v * 30.0 + floor(uv.x * n) * 0.7);
    }
    if kind == 2u {
        // Coils: each strand zigzags tightly.
        s += 0.04 * sin(v * 140.0 + floor(uv.x * n) * 2.1);
    }
    let x = s * n;
    let id = floor(x);
    let r = hash(id);
    // Each strand's profile across it (1 at its middle), and its end: strands end at different
    // lengths, so the card thins to a ragged tip.
    let c = 1.0 - smoothstep(0.25, 0.5, abs(fract(x) - 0.5));
    // Short hair (brows, beards, coils) ends more raggedly than long.
    let short = kind == 2u || kind == 5u || kind == 6u;
    let end = select(0.7, 0.4, short) + select(0.3, 0.6, short) * hash(id + 17.0);
    let body = 1.0 - smoothstep(end - 0.25, end, v);
    // Within the card the strands lie close (solid, the gaps only shaded); at its edges and tips
    // only the strands themselves cover. Short hair is fibres all through, except a coil's
    // inner shells.
    let edge = smoothstep(0.0, 0.18, uv.x) * smoothstep(1.0, 0.82, uv.x);
    var solid = body * edge;
    if short {
        var keep = 0.0;
        if kind == 2u && depth < 0.5 {
            keep = 0.8;
        } else if kind == 5u {
            keep = 0.75;
        }
        solid *= keep;
    }
    let alpha = max(solid, c * body * 0.9) * step(v, end);
    return vec2<f32>(alpha, (0.6 + 0.4 * c) * (0.85 + 0.3 * r));
}

@fragment
fn fs_main(in: VsOut, @builtin(front_facing) front: bool) -> @location(0) vec4<f32> {
    let cov = coverage(in.kind, in.uv, in.depth);
    if cov.x < 0.4 {
        discard;
    }
    var n = normalize(in.normal);
    if !front {
        n = -n;
    }
    let t = normalize(in.tangent);
    let v = normalize(u.eye.xyz - in.world);
    let l = u.light_dir.xyz;
    let h = normalize(l + v);
    let wet = hair.color.w;
    var albedo = hair.color.rgb * cov.y * mix(1.0, 0.55, wet);
    // Darker deep in the hair and toward the roots (the hair over it shades it).
    let shade = mix(0.4, 1.0, in.depth) * mix(0.65, 1.0, smoothstep(0.0, 0.5, in.uv.y));
    // Diffuse about the fibre (Kajiya–Kay), wrapped.
    let tl = dot(t, l);
    let diffuse = mix(0.3, 1.0, sqrt(max(1.0 - tl * tl, 0.0))) * clamp(dot(n, l) * 0.5 + 0.6, 0.0, 1.0);
    // The two highlights: the surface's (white, toward the root) and the one through the fibre
    // (coloured, toward the tip); wet hair's sharper and brighter.
    let t1 = normalize(t + n * 0.08);
    let t2 = normalize(t - n * 0.12);
    let sheen = select(1.0, 0.35, in.kind == 5u || in.kind == 6u);
    let r = fibre(t1, h, mix(90.0, 180.0, wet)) * mix(0.08, 0.2, wet) * sheen;
    let trt = fibre(t2, h, 24.0) * 0.18;
    let spec = vec3<f32>(r) + albedo * trt * 2.0;
    // Light through the hair from behind.
    let through = albedo * pow(clamp(dot(-v, l), 0.0, 1.0), 4.0) * 0.6;
    let hemi = mix(u.ground.rgb, u.sky.rgb, 0.5 + 0.5 * n.y);
    var lit = (albedo * (u.light.rgb * diffuse + hemi) / 3.14159265 + u.light.rgb * (spec + through * 0.3)) * shade;
    var shown = tonemap(lit * u.exposure.x);
    if u.exposure.y > 0.5 {
        shown = pow(shown, vec3<f32>(1.0 / 2.2));
    }
    return vec4<f32>(shown, 1.0);
}
