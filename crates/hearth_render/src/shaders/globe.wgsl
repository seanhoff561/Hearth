// The planet as a globe: an equirectangular map on an orthographic sphere, lit like a desk
// globe, with a graticule every 30°, the camera's place and the point under the cursor marked,
// and a thin glow of air around it.

struct Globe {
    // East, north and outward at the centre; w: the globe's radius (px), frame width, height.
    right: vec4<f32>,
    up: vec4<f32>,
    out: vec4<f32>,
    // The camera's place and the point under the cursor (unit vectors); w: 1 when shown.
    camera: vec4<f32>,
    cursor: vec4<f32>,
};

@group(0) @binding(0) var<uniform> globe: Globe;
@group(0) @binding(1) var map: texture_2d<f32>;
@group(0) @binding(2) var samp: sampler;

const PI: f32 = 3.14159265;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) i: u32) -> VsOut {
    let uv = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    var out: VsOut;
    out.pos = vec4<f32>(uv * 2.0 - 1.0, 0.0, 1.0);
    return out;
}

// Distance (px) from a pixel to where a point of the sphere appears, far on the far side.
fn marker_distance(frag: vec2<f32>, p: vec3<f32>) -> f32 {
    if dot(p, globe.out.xyz) <= 0.0 {
        return 1e6;
    }
    let centre = 0.5 * vec2<f32>(globe.up.w, globe.out.w);
    let s = centre + vec2<f32>(dot(p, globe.right.xyz), -dot(p, globe.up.xyz)) * globe.right.w;
    return length(frag - s);
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let r = globe.right.w;
    let size = vec2<f32>(globe.up.w, globe.out.w);
    let q = (in.pos.xy - 0.5 * size) / r * vec2<f32>(1.0, -1.0);
    let d2 = dot(q, q);
    let z = sqrt(max(1.0 - d2, 0.0));
    let p = globe.right.xyz * q.x + globe.up.xyz * q.y + globe.out.xyz * z;
    let lat = asin(clamp(p.y, -1.0, 1.0));
    let lon = atan2(p.z, p.x);
    let uv = vec2<f32>(fract(lon / (2.0 * PI) + 1.0), 0.5 - lat / PI);
    // Gradients without the jump at the date line.
    var gx = dpdx(uv);
    var gy = dpdy(uv);
    gx.x -= round(gx.x);
    gy.x -= round(gy.x);
    var c = textureSampleGrad(map, samp, uv, gx, gy).rgb;
    // Graticule every 30°.
    let g = vec2<f32>(lat, lon) * (6.0 / PI);
    let wg = vec2<f32>(fwidth(g.x), min(fwidth(g.y), fwidth(fract(g.y / 12.0 + 0.5) * 12.0)));
    let f = abs(fract(g + 0.5) - 0.5) / max(wg, vec2<f32>(1e-5));
    c = mix(c, vec3<f32>(0.85), 0.2 * (1.0 - clamp(min(f.x, f.y), 0.0, 1.0)));
    // Lit from the upper left.
    let light = normalize(vec3<f32>(-0.45, 0.5, 0.75));
    c *= 0.4 + 0.7 * max(dot(vec3<f32>(q, z), light), 0.0);
    // The camera's place: a red dot in a white ring; the cursor's point: a white ring.
    if globe.camera.w > 0.0 {
        let d = marker_distance(in.pos.xy, globe.camera.xyz);
        c = mix(c, vec3<f32>(1.0), 1.0 - clamp(abs(d - 5.0) - 1.0, 0.0, 1.0));
        c = mix(c, vec3<f32>(0.9, 0.08, 0.05), 1.0 - clamp(d - 3.0, 0.0, 1.0));
    }
    if globe.cursor.w > 0.0 {
        let d = marker_distance(in.pos.xy, globe.cursor.xyz);
        c = mix(c, vec3<f32>(1.0), 1.0 - clamp(abs(d - 8.0) - 0.75, 0.0, 1.0));
    }
    // Space around the globe, with air glowing at its edge (antialiased over a pixel).
    let d = sqrt(d2);
    let inside = clamp((1.0 - d) * r + 0.5, 0.0, 1.0);
    let glow = exp(-max(d - 1.0, 0.0) * r / 7.0);
    let space = vec3<f32>(0.004, 0.006, 0.014) + vec3<f32>(0.12, 0.3, 0.75) * 0.45 * glow;
    return vec4<f32>(mix(space, c, inside), 1.0);
}
