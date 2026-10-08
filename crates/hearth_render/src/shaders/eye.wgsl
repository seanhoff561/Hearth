// Eyes (Amendment E §8, E7): the balls turned to the gaze, the lids over them, the lashes. The
// iris is painted on the ball and seen through the cornea a little shifted by the view (it lies
// some millimetres behind it), with a dark limbal ring, the pupil, radial fibres and a lighter
// collarette; the sclera white with a warm tinge at the corners; the cornea wet: a sharp
// highlight and the sky's reflection. Lids are skin, their inner edges wet; lashes are fine
// dark strands.






struct VsIn {
    @location(0) pos: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) local: vec3<f32>,
    @location(3) uv: vec2<f32>,
    @location(4) part: vec4<u32>,
};

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) world: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) local: vec3<f32>,
    // The view direction in the eye's own frame.
    @location(3) view_local: vec3<f32>,
    @location(4) uv: vec2<f32>,
    @location(5) @interpolate(flat) surface: u32,
};

@vertex
fn vs_main(v: VsIn) -> VsOut {
    let m = eyes.parts[v.part.x];
    let world = (m * vec4<f32>(v.pos, 1.0)).xyz;
    let m3 = mat3x3<f32>(m[0].xyz, m[1].xyz, m[2].xyz);
    var out: VsOut;
    out.pos = clip(world);
    out.world = world;
    out.normal = normalize(m3 * v.normal);
    out.local = v.local;
    out.view_local = transpose(m3) * normalize(eye_pos() - world);
    out.uv = v.uv;
    out.surface = v.part.y;
    return out;
}

fn hash(x: f32) -> f32 {
    return fract(sin(x * 127.1 + 311.7) * 43758.5453);
}


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
fn fs_main(in: VsOut, @builtin(front_facing) front: bool) -> @location(0) vec4<f32> {
    var n = normalize(in.normal);
    if !front {
        n = -n;
    }
    let v = normalize(eye_pos() - in.world);
    let l = sun_dir();
    let hemi = ambient(n);
    if in.surface == 3u {
        // Lashes: fine dark strands, thinning to their tips.
        let x = in.uv.x * 60.0;
        let id = floor(x);
        let end = 0.55 + 0.45 * hash(id);
        let c = 1.0 - smoothstep(0.2, 0.45, abs(fract(x) - 0.5));
        if c < 0.5 || in.uv.y > end {
            discard;
        }
        let albedo = u.hair.rgb * 0.35;
        let lit = albedo * (sun_rgb() * 0.5 + hemi) / 3.14159265;
        return finish(lit, in.world);
    }
    if in.surface != 0u {
        // Lid skin; its inner edge wet and pinker.
        let margin = select(0.0, 1.0, in.surface == 2u);
        let albedo = mix(u.skin.rgb, u.lips.rgb, 0.25 + 0.5 * margin);
        let nl = dot(n, l);
        let wrap = vec3<f32>(0.42, 0.22, 0.16);
        let diffuse = max((vec3<f32>(nl) + wrap) / (1.0 + wrap), vec3<f32>(0.0));
        let spec = ggx(n, v, l, mix(0.45, 0.15, margin), 0.028);
        return finish(albedo * (sun_rgb() * diffuse + hemi) / 3.14159265 + sun_rgb() * spec, in.world);
    }
    // The ball.
    let d = normalize(in.local);
    // The iris seen through the cornea: shifted against the view, as it lies behind.
    let vl = normalize(in.view_local);
    let q = d.xy - vl.xy * 0.08 * smoothstep(0.75, 0.95, d.z);
    let r = length(q);
    let iris_r = 0.47;
    let pupil_r = mix(0.12, 0.26, eyes.iris.w);
    var albedo = vec3<f32>(0.78, 0.76, 0.72);
    // The sclera warmer toward the corners, and shaded under the lid.
    albedo = mix(albedo, vec3<f32>(0.8, 0.55, 0.5), smoothstep(0.55, 0.95, abs(d.x)) * 0.5);
    if d.z > 0.0 && r < iris_r + 0.03 {
        let angle = atan2(q.y, q.x);
        let fibres = 0.75 + 0.25 * hash(floor(angle * 40.0)) * (0.6 + 0.4 * sin(r * 90.0));
        var iris = eyes.iris.rgb * fibres * 1.5;
        // A lighter ring round the pupil, a dark ring at the edge.
        iris = mix(iris, iris * 1.6 + vec3<f32>(0.03, 0.02, 0.0), smoothstep(pupil_r + 0.12, pupil_r, r) * 0.5);
        iris *= mix(1.0, 0.35, smoothstep(iris_r - 0.08, iris_r, r));
        let into = smoothstep(iris_r + 0.03, iris_r - 0.01, r);
        albedo = mix(albedo, iris, into);
        albedo = mix(albedo, vec3<f32>(0.01), smoothstep(pupil_r + 0.015, pupil_r - 0.01, r));
    }
    // The lids shade the ball's top.
    let shade = mix(0.45, 1.0, smoothstep(0.55, -0.1, d.y));
    let nl = max(dot(n, l), 0.0);
    // Light under the sclera's surface softens its terminator.
    let soft = clamp(dot(n, l) * 0.7 + 0.3, 0.0, 1.0);
    let amb = vec3<f32>(dot(hemi, vec3<f32>(0.3, 0.4, 0.3)));
    var lit = albedo * (sun_rgb() * mix(nl, soft, 0.5) + amb) / 3.14159265 * shade;
    // The wet cornea: a sharp highlight and the sky in it.
    let fres = 0.025 + 0.975 * pow(1.0 - max(dot(n, v), 0.0), 5.0);
    lit += sun_rgb() * ggx(n, v, l, 0.05, 0.025) * shade;
    lit += ambient(vec3<f32>(0.0, 1.0, 0.0)) * fres * 0.25 * shade / 3.14159265;
    return finish(lit, in.world);
}
