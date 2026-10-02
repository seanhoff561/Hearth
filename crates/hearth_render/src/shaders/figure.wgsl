// Bodies in the world: boxes placed by instances (`hearth_character::FigureInstance`), lit as
// the terrain is (`common.wgsl` is prepended: globals, aerial perspective, curvature).

struct Part {
    r0: vec4<f32>,
    r1: vec4<f32>,
    r2: vec4<f32>,
        // sRGB colour (rgba8), and the light where the body is (sky 0–15 in the low byte, block
    // 0–15 in the next).
    color: u32,
    light: u32,
    // A coat: the unwrap's corner in the coat texture (x low, y high), and its size in pixels
    // (x, y, z in bytes; the top bit set when the box wears one).
    coat_at: u32,
    coat_size: u32,
};

@group(1) @binding(0) var<storage, read> parts: array<Part>;
@group(2) @binding(0) var coats: texture_2d<f32>;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) normal: vec3<f32>,
    // Camera-relative position.
    @location(1) world: vec3<f32>,
    @location(2) @interpolate(flat) color: u32,
    @location(3) @interpolate(flat) light: u32,
    // Darker toward the bottom of each box: a hint of the light that does not reach under.
    @location(4) shade: f32,
    // Where on its coat (texels), and the face's rectangle there (x0, y0, x1, y1; x1 = 0 for
    // a plain box).
    @location(5) coat_uv: vec2<f32>,
    @location(6) @interpolate(flat) coat_rect: vec4<u32>,
};

// A face's rectangle in a box's unwrap (x, y, width, height) for the box's size in pixels:
// the top and bottom in the first row, the four sides in the second (as
// `hearth_texgen::coats::face_rect`).
fn coat_face(face: u32, d: vec3<u32>) -> vec4<u32> {
    let w = d.x;
    let h = d.y;
    let z = d.z;
    switch face {
        case 0u: { return vec4<u32>(z + w, z, z, h); }
        case 1u: { return vec4<u32>(0u, z, z, h); }
        case 2u: { return vec4<u32>(z, 0u, w, z); }
        case 3u: { return vec4<u32>(z + w, 0u, w, z); }
        case 4u: { return vec4<u32>(z, z, w, h); }
        default: { return vec4<u32>(2u * z + w, z, w, h); }
    }
}

// A face's normal and the two edges along it (u × v = normal), for the six faces.
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

// Corner of the unit cube (−½..½) for a vertex of 36: six faces of two triangles.
fn cube_corner(vi: u32) -> vec3<f32> {
    let ax = face_axes(vi / 6u);
    let k = vi % 6u;
    let corner = array<u32, 6>(0u, 1u, 2u, 0u, 2u, 3u)[k];
    let cu = select(-1.0, 1.0, corner == 1u || corner == 2u);
    let cv = select(-1.0, 1.0, corner >= 2u);
    return 0.5 * (ax[0] + cu * ax[1] + cv * ax[2]);
}

fn part_matrix(p: Part) -> mat3x3<f32> {
    return mat3x3<f32>(
        vec3<f32>(p.r0.x, p.r1.x, p.r2.x),
        vec3<f32>(p.r0.y, p.r1.y, p.r2.y),
        vec3<f32>(p.r0.z, p.r1.z, p.r2.z),
    );
}

@vertex
fn vs_main(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> VsOut {
    let p = parts[ii];
    let local = cube_corner(vi);
    let m = part_matrix(p);
    let t = vec3<f32>(p.r0.w, p.r1.w, p.r2.w);
    let world = m * local + t;
    var out: VsOut;
    out.pos = g.view_proj * vec4<f32>(curve(world), 1.0);
    out.normal = normalize(m * face_axes(vi / 6u)[0]);
    out.world = world;
    out.color = p.color;
    out.light = p.light;
    out.shade = mix(0.8, 1.0, local.y + 0.5);
    out.coat_uv = vec2<f32>(0.0);
    out.coat_rect = vec4<u32>(0u);
    if (p.coat_size & 0x80000000u) != 0u {
        let d = vec3<u32>(p.coat_size & 255u, (p.coat_size >> 8u) & 255u, (p.coat_size >> 16u) & 255u);
        let r = coat_face(vi / 6u, d);
        let origin = vec2<u32>(p.coat_at & 0xffffu, p.coat_at >> 16u) + r.xy;
        // The corner's place on the face, as `cube_corner` has it.
        let corner = array<u32, 6>(0u, 1u, 2u, 0u, 2u, 3u)[vi % 6u];
        let s = select(0.0, 1.0, corner == 1u || corner == 2u);
        let t = select(0.0, 1.0, corner >= 2u);
        out.coat_uv = vec2<f32>(origin) + vec2<f32>(s * f32(r.z), (1.0 - t) * f32(r.w));
        out.coat_rect = vec4<u32>(origin, origin + r.zw);
        out.shade = mix(0.9, 1.0, local.y + 0.5);
    }
    return out;
}

fn figure_light_curve(l: f32) -> f32 {
    return l / (4.0 - 3.0 * l);
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    var albedo = unpack_rgb(in.color);
    if in.coat_rect.z > 0u {
        let lo = vec2<i32>(in.coat_rect.xy);
        let hi = vec2<i32>(in.coat_rect.zw) - vec2<i32>(1);
        let texel = clamp(vec2<i32>(floor(in.coat_uv)), lo, hi);
        albedo = textureLoad(coats, texel, 0).rgb;
    }
    let sky = f32(in.light & 255u) / 15.0;
    let block = f32((in.light >> 8u) & 255u) / 15.0;
    let n = normalize(in.normal);
    let sky_dir = 0.62 + 0.38 * n.y + 0.1 * (1.0 - abs(n.y));
    let ambient = g.sky_light.rgb * figure_light_curve(sky) * max(sky_dir, 0.2)
        + vec3<f32>(g.sky_light.a);
    // Direct light where the sky is open; wrapped a little, as skin and hide scatter it.
    let open = smoothstep(0.8, 1.0, sky);
    let wrap = max((dot(n, g.sun.xyz) + 0.15) / 1.15, 0.0);
    let direct = g.sun_light.rgb * wrap * open;
    let fire = g.block_light.rgb * figure_light_curve(block);
    let c = albedo * ((ambient + fire) * in.shade + direct) / 3.14159265;
    return vec4<f32>(aerial(c, in.world), 1.0);
}
