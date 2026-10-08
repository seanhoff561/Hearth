// Smoke over fires in the vegetation: soft puffs generated in the vertex shader from the
// vertex index. Each plume (a source, its strength and whether it is a far fire's) sends up a
// stream of puffs that rise and slow, swell, lean with the wind and thin out; each puff is a
// camera-facing disc, lit by the sky and the sun, glowing over the flames at its foot, and
// fading into the haze with distance. Depth-tested against the scene, never written.

struct Plume {
    // xyz: camera-relative source (m); w: strength 0..1.
    pos: vec4<f32>,
    // x: 1 for a far fire; y: the plume's seed.
    params: vec4<f32>,
};

struct Params {
    view_proj: mat4x4<f32>,
    right: vec4<f32>,
    up: vec4<f32>,
    // xyz: wind (m/s); w: seconds.
    wind: vec4<f32>,
    // rgb: sky irradiance (pre-exposed).
    ambient: vec4<f32>,
    // rgb: direct sun or moon light (pre-exposed).
    direct: vec4<f32>,
    // rgb: the glow of flames (pre-exposed).
    fire: vec4<f32>,
    // rgb: the colour of the haze (pre-exposed); w: its extinction (1/m).
    haze: vec4<f32>,
    // x: plumes; y: puffs per plume.
    count: vec4<u32>,
    plumes: array<Plume, 64>,
};

@group(0) @binding(0) var<uniform> P: Params;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec3<f32>,
    @location(2) alpha: f32,
    @location(3) @interpolate(flat) seed: f32,
};

fn hash_u(x: u32) -> u32 {
    var h = x * 747796405u + 2891336453u;
    h = ((h >> ((h >> 28u) + 4u)) ^ h) * 277803737u;
    return (h >> 22u) ^ h;
}

fn rand(i: u32, k: u32) -> f32 {
    return f32(hash_u(i * 8u + k)) / 4294967296.0;
}

@vertex
fn vs_main(@builtin(vertex_index) vi: u32) -> VsOut {
    var out: VsOut;
    let puffs = max(P.count.y, 1u);
    let i = vi / 6u;
    let pi = i / puffs;
    let k = i % puffs;
    let corners = array<vec2<f32>, 6>(
        vec2<f32>(-1.0, -1.0), vec2<f32>(1.0, -1.0), vec2<f32>(1.0, 1.0),
        vec2<f32>(-1.0, -1.0), vec2<f32>(1.0, 1.0), vec2<f32>(-1.0, 1.0),
    );
    let corner = corners[vi % 6u];
    out.uv = corner;
    if pi >= P.count.x {
        out.pos = vec4<f32>(-2.0, -2.0, 0.5, 1.0);
        return out;
    }
    let plume = P.plumes[pi];
    let far = plume.params.x > 0.5;
    let strength = plume.pos.w;
    let id = u32(plume.params.y) * 977u + k;
    // A puff's life: seconds of real time (the plume animates as it is watched).
    let life = select(40.0, 300.0, far);
    let a = fract(P.wind.w / life + rand(id, 0u));
    // Rising and slowing, swelling, leaning downwind.
    let top = select(70.0, 1600.0, far) * (0.6 + 0.4 * strength);
    let h = top * (1.0 - (1.0 - a) * (1.0 - a));
    // Carried off by the wind, at the wind's speed by the end of its life.
    let lean = P.wind.xyz * a * a * life * 0.5;
    let r = select(1.8 + 9.0 * a, 40.0 + 420.0 * a, far) * (0.7 + 0.5 * rand(id, 1u));
    let spread = select(0.8 + 3.0 * a, 60.0 + 250.0 * a, far);
    let off = vec3<f32>(rand(id, 2u) - 0.5, 0.0, rand(id, 3u) - 0.5) * 2.0 * spread;
    let center = plume.pos.xyz + vec3<f32>(0.0, h, 0.0) + lean + off;
    // Thin at birth, thickest early, thinning as it spreads.
    out.alpha = (0.35 + 0.65 * strength) * smoothstep(0.0, 0.05, a) * pow(1.0 - a, 1.3)
        * select(0.85, 0.7, far);
    // Grey smoke, dark and brown low and thick, lighter as it thins.
    let albedo = mix(vec3<f32>(0.16, 0.14, 0.12), vec3<f32>(0.55, 0.55, 0.57), sqrt(a));
    var col = albedo * (P.ambient.rgb * 0.9 + P.direct.rgb * 0.55) / 3.14159265;
    // The flames light the smoke over them.
    col += P.fire.rgb * 0.08 * strength * (1.0 - smoothstep(0.0, 0.25, a));
    // Into the haze with distance.
    let d = length(center);
    let t = exp(-P.haze.w * d);
    out.color = col * t + P.haze.rgb * (1.0 - t);
    out.alpha *= mix(1.0, t, 0.5);
    out.seed = rand(id, 4u) * 17.0;
    let p = center + (P.right.xyz * corner.x + P.up.xyz * corner.y) * r;
    out.pos = P.view_proj * vec4<f32>(p, 1.0);
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let r2 = dot(in.uv, in.uv);
    if r2 > 1.0 {
        discard;
    }
    // A soft, lumpy disc.
    let lump = 0.7 + 0.3 * sin(in.uv.x * 5.0 + in.seed) * sin(in.uv.y * 4.0 + in.seed * 1.7);
    let a = (1.0 - smoothstep(0.35, 1.0, sqrt(r2))) * lump * in.alpha;
    return vec4<f32>(in.color, a);
}
