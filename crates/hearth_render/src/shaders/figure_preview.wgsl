// A body on its own (the character screen): the same boxes as `figure.wgsl`, lit by a sun, a
// sky and the ground's bounce, exposed and tone-mapped straight to the display.

struct Preview {
    view_proj: mat4x4<f32>,
    // xyz: direction to the light.
    light_dir: vec4<f32>,
    // rgb: the light's illuminance (lux); the sky's from above and the ground's from below.
    light: vec4<f32>,
    sky: vec4<f32>,
    ground: vec4<f32>,
    // x: exposure; y: 1 to encode the display's gamma here (a target that is not sRGB).
    exposure: vec4<f32>,
};

struct Part {
    r0: vec4<f32>,
    r1: vec4<f32>,
    r2: vec4<f32>,
    color: u32,
    light: u32,
    pad0: u32,
    pad1: u32,
};

@group(0) @binding(0) var<uniform> u: Preview;
@group(1) @binding(0) var<storage, read> parts: array<Part>;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) normal: vec3<f32>,
    @location(1) @interpolate(flat) color: u32,
    @location(2) shade: f32,
};

fn face_axes(f: u32) -> mat3x3<f32> {
    switch f {
        case 0u: { return mat3x3<f32>(vec3(1.0, 0.0, 0.0), vec3(0.0, 0.0, -1.0), vec3(0.0, 1.0, 0.0)); }
        case 1u: { return mat3x3<f32>(vec3(-1.0, 0.0, 0.0), vec3(0.0, 0.0, 1.0), vec3(0.0, 1.0, 0.0)); }
        case 2u: { return mat3x3<f32>(vec3(0.0, 1.0, 0.0), vec3(1.0, 0.0, 0.0), vec3(0.0, 0.0, -1.0)); }
        case 3u: { return mat3x3<f32>(vec3(0.0, -1.0, 0.0), vec3(1.0, 0.0, 0.0), vec3(0.0, 0.0, 1.0)); }
        case 4u: { return mat3x3<f32>(vec3(0.0, 0.0, 1.0), vec3(1.0, 0.0, 0.0), vec3(0.0, 1.0, 0.0)); }
        default: { return mat3x3<f32>(vec3(0.0, 0.0, -1.0), vec3(-1.0, 0.0, 0.0), vec3(0.0, 1.0, 0.0)); }
    }
}

fn cube_corner(vi: u32) -> vec3<f32> {
    let ax = face_axes(vi / 6u);
    let k = vi % 6u;
    let corner = array<u32, 6>(0u, 1u, 2u, 0u, 2u, 3u)[k];
    let cu = select(-1.0, 1.0, corner == 1u || corner == 2u);
    let cv = select(-1.0, 1.0, corner >= 2u);
    return 0.5 * (ax[0] + cu * ax[1] + cv * ax[2]);
}

@vertex
fn vs_main(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> VsOut {
    let p = parts[ii];
    let local = cube_corner(vi);
    let m = mat3x3<f32>(
        vec3<f32>(p.r0.x, p.r1.x, p.r2.x),
        vec3<f32>(p.r0.y, p.r1.y, p.r2.y),
        vec3<f32>(p.r0.z, p.r1.z, p.r2.z),
    );
    let world = m * local + vec3<f32>(p.r0.w, p.r1.w, p.r2.w);
    var out: VsOut;
    out.pos = u.view_proj * vec4<f32>(world, 1.0);
    out.normal = normalize(m * face_axes(vi / 6u)[0]);
    out.color = p.color;
    out.shade = mix(0.8, 1.0, local.y + 0.5);
    return out;
}

fn srgb_to_linear(c: vec3<f32>) -> vec3<f32> {
    return pow(c, vec3<f32>(2.2));
}

// A filmic curve (after Narkowicz's fit of ACES).
fn tonemap(x: vec3<f32>) -> vec3<f32> {
    return clamp(x * (2.51 * x + 0.03) / (x * (2.43 * x + 0.59) + 0.14), vec3<f32>(0.0), vec3<f32>(1.0));
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let c = in.color;
    let albedo = srgb_to_linear(vec3<f32>(f32(c & 255u), f32((c >> 8u) & 255u), f32((c >> 16u) & 255u)) / 255.0);
    let n = normalize(in.normal);
    let wrap = max((dot(n, u.light_dir.xyz) + 0.15) / 1.15, 0.0);
    let hemi = mix(u.ground.rgb, u.sky.rgb, 0.5 + 0.5 * n.y);
    let lit = albedo * (u.light.rgb * wrap + hemi * in.shade) / 3.14159265;
    var shown = tonemap(lit * u.exposure.x);
    if u.exposure.y > 0.5 {
        shown = pow(shown, vec3<f32>(1.0 / 2.2));
    }
    return vec4<f32>(shown, 1.0);
}
