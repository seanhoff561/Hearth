// The planet as a globe (Amendment T §2.4): an equirectangular map on an orthographic sphere,
// in relief. The map's heights tilt the surface's normal (their slopes exaggerated by
// `look.x`), so that ranges and the sea's trenches catch the desk lamp's light as the globe
// turns, and a cartographer's light from the north-west (and, softer, the west and north) shows
// their shapes wherever they are. The land is tinted by its height, the sea by its depth;
// rivers, lakes and ice are drawn; the land coloured by biome, by climate or as plain relief
// (`look.y`). A graticule every 30°, the camera's place and the point under the cursor marked,
// and a thin glow of air around it.

struct Globe {
    // East, north and outward at the centre; w: the globe's radius (px), frame width, height.
    right: vec4<f32>,
    up: vec4<f32>,
    out: vec4<f32>,
    // The camera's place and the point under the cursor (unit vectors); w: 1 when shown.
    camera: vec4<f32>,
    cursor: vec4<f32>,
    // x: the relief's exaggeration; y: the mode (0 biomes, 1 climate, 2 relief); z: the
    // planet's radius (m); w: the relief's width (texels).
    look: vec4<f32>,
    // The finer relief's window (radians: south, north, west, east; none when north is not
    // above south) and its width (texels).
    detail: vec4<f32>,
    detail_size: vec4<f32>,
};

@group(0) @binding(0) var<uniform> globe: Globe;
// The ground's colour by biome (sRGB) and its water (alpha: 0 land, 0.5 a lake, 1 the sea).
@group(0) @binding(1) var map: texture_2d<f32>;
@group(0) @binding(2) var samp: sampler;
// The river, ice (1 a sheet or glacier, 0.7 the sea's pack), warmth and rain.
@group(0) @binding(3) var facts: texture_2d<f32>;
// The surface's height, or the water's floor's (m).
@group(0) @binding(4) var heights: texture_2d<f32>;
// The finer relief over the window seen near (m), rows from its north.
@group(0) @binding(5) var detail: texture_2d<f32>;

// The relief's height and its slopes (m a metre, east and north) from a height texture at `uv`
// (its gradients `gx`, `gy`), the texture's texels `texel` apart in u and v, spanning
// `span_u`, `span_v` radians of longitude and latitude, at latitude `lat`. The slopes are read
// a texel or a pixel either side, whichever is wider; z: the distance (m) they are read over.
fn relief_at(
    tex: texture_2d<f32>,
    uv: vec2<f32>,
    gx: vec2<f32>,
    gy: vec2<f32>,
    texel: vec2<f32>,
    span: vec2<f32>,
    lat: f32,
) -> vec4<f32> {
    let h = textureSampleGrad(tex, samp, uv, gx, gy).r;
    let su = max(texel.x, abs(gx.x) + abs(gy.x));
    let sv = max(texel.y, abs(gx.y) + abs(gy.y));
    let e = textureSampleGrad(tex, samp, uv + vec2<f32>(su, 0.0), gx, gy).r;
    let w = textureSampleGrad(tex, samp, uv - vec2<f32>(su, 0.0), gx, gy).r;
    let n = textureSampleGrad(tex, samp, uv - vec2<f32>(0.0, sv), gx, gy).r;
    let s = textureSampleGrad(tex, samp, uv + vec2<f32>(0.0, sv), gx, gy).r;
    let radius = globe.look.z;
    let across = 2.0 * su * span.x * radius * max(cos(lat), 0.05);
    let along = 2.0 * sv * span.y * radius;
    return vec4<f32>(h, (e - w) / across, (n - s) / along, max(across, along));
}

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

fn linear(c: vec3<f32>) -> vec3<f32> {
    return pow(c, vec3<f32>(2.2));
}

// Land tinted by its height (m): green lowlands, pale uplands, brown and grey mountains, white
// peaks.
fn hypsometric(h: f32) -> vec3<f32> {
    var stops = array<vec4<f32>, 8>(
        vec4<f32>(0.0, 0.31, 0.48, 0.26),
        vec4<f32>(300.0, 0.54, 0.64, 0.35),
        vec4<f32>(800.0, 0.76, 0.71, 0.47),
        vec4<f32>(1500.0, 0.71, 0.55, 0.37),
        vec4<f32>(2500.0, 0.56, 0.43, 0.34),
        vec4<f32>(3500.0, 0.61, 0.56, 0.53),
        vec4<f32>(4500.0, 0.91, 0.91, 0.93),
        vec4<f32>(6000.0, 1.0, 1.0, 1.0),
    );
    var c = stops[0].yzw;
    for (var i = 1; i < 8; i++) {
        let a = stops[i - 1];
        let b = stops[i];
        if h > a.x {
            c = mix(a.yzw, b.yzw, clamp((h - a.x) / (b.x - a.x), 0.0, 1.0));
        }
    }
    return linear(c);
}

// The sea by its depth (m): pale over the shelves, blue down the slopes, dark over the abyssal
// plains, darkest in the trenches.
fn bathymetric(d: f32) -> vec3<f32> {
    var stops = array<vec4<f32>, 7>(
        vec4<f32>(0.0, 0.66, 0.85, 0.91),
        vec4<f32>(200.0, 0.44, 0.70, 0.84),
        vec4<f32>(1000.0, 0.24, 0.53, 0.75),
        vec4<f32>(3000.0, 0.12, 0.35, 0.60),
        vec4<f32>(5000.0, 0.07, 0.25, 0.47),
        vec4<f32>(7000.0, 0.04, 0.15, 0.33),
        vec4<f32>(10000.0, 0.02, 0.08, 0.20),
    );
    var c = stops[0].yzw;
    for (var i = 1; i < 7; i++) {
        let a = stops[i - 1];
        let b = stops[i];
        if d > a.x {
            c = mix(a.yzw, b.yzw, clamp((d - a.x) / (b.x - a.x), 0.0, 1.0));
        }
    }
    return linear(c);
}

// The climate: warmth (0–1 over −40 to 40 °C) in hue from ice blue through green to a hot
// orange; rain (the square root of its share of 4 m) pale and sandy when dry, deep when wet.
fn climate(warmth: f32, rain: f32) -> vec3<f32> {
    var hues = array<vec3<f32>, 5>(
        vec3<f32>(0.78, 0.86, 0.95),
        vec3<f32>(0.38, 0.62, 0.72),
        vec3<f32>(0.36, 0.62, 0.30),
        vec3<f32>(0.86, 0.76, 0.32),
        vec3<f32>(0.86, 0.42, 0.20),
    );
    let x = clamp(warmth, 0.0, 1.0) * 4.0;
    let i = min(u32(x), 3u);
    var c = mix(hues[i], hues[i + 1u], x - f32(i));
    let dry = 1.0 - smoothstep(0.15, 0.6, rain);
    c = mix(c, vec3<f32>(0.84, 0.76, 0.60), dry * 0.6);
    c *= mix(1.0, 0.75, smoothstep(0.5, 0.9, rain));
    return linear(c);
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
    let base = textureSampleGrad(map, samp, uv, gx, gy);
    let f = textureSampleGrad(facts, samp, uv, gx, gy);
    // The relief: the map's, and the finer window's where it is drawn (faded in from its
    // edges).
    var rel = relief_at(heights, uv, gx, gy, vec2<f32>(1.0, 2.0) / globe.look.w, vec2<f32>(2.0 * PI, PI), lat);
    let win = globe.detail;
    let dw = win.w - win.z;
    let dh = win.y - win.x;
    let duv = vec2<f32>(
        (lon - win.z - 2.0 * PI * floor((lon - win.z) / (2.0 * PI))) / max(dw, 1e-6),
        (win.y - lat) / max(dh, 1e-6),
    );
    let dgx = dpdx(duv);
    let dgy = dpdy(duv);
    let edge = min(min(duv.x, 1.0 - duv.x), min(duv.y, 1.0 - duv.y));
    let near = select(0.0, smoothstep(0.0, 0.05, edge), dh > 0.0 && edge > 0.0);
    if near > 0.0 {
        let fine = relief_at(detail, duv, dgx, dgy, vec2<f32>(1.0) / globe.detail_size.x, vec2<f32>(dw, dh), lat);
        rel = mix(rel, fine, near);
    }
    let h = rel.x;
    // The relief exaggerated as maps of smaller scale exaggerate it: slopes read over more than
    // 10 km steepened in proportion (up to ten times), so that a range seen whole still stands.
    let k = globe.look.x * clamp(rel.w / 10000.0, 1.0, 10.0);
    // Under water the floor's relief shows fainter.
    let water = base.a;
    let wet = smoothstep(0.2, 0.4, water);
    let sea = smoothstep(0.7, 0.9, water);
    let kk = k * mix(1.0, 0.6, wet);
    let local = normalize(vec3<f32>(-kk * rel.y, -kk * rel.z, 1.0));
    // The local frame: east, north, up.
    let up = p;
    let east = normalize(vec3<f32>(-sin(lon), 0.0, cos(lon)));
    let north = cross(east, up);
    let n = normalize(east * local.x + north * local.y + up * local.z);
    // The desk lamp, upper left of the frame: the relief catches it as the globe turns.
    let lamp = normalize(globe.right.xyz * -0.45 + globe.up.xyz * 0.5 + globe.out.xyz * 0.75);
    let sphere_lit = max(dot(up, lamp), 0.0);
    let lamp_relief = (max(dot(n, lamp), 0.0) + 0.12) / (sphere_lit + 0.12);
    // The cartographer's light: from the north-west at 45°, the west and the north softer.
    let a = 0.70710678;
    let carto = (0.5 * max(dot(local, vec3<f32>(-a * a, a * a, a)), 0.0)
        + 0.25 * max(dot(local, vec3<f32>(-a, 0.0, a)), 0.0)
        + 0.25 * max(dot(local, vec3<f32>(0.0, a, a)), 0.0)) / a;
    let relief = clamp(mix(lamp_relief, carto, 0.5), 0.3, 1.9);
    // The land by the mode, its rivers and its ice.
    let mode = globe.look.y;
    let hyps = hypsometric(max(h, 0.0));
    var land = mix(base.rgb, hyps, 0.3);
    if mode > 1.5 {
        land = hyps;
    } else if mode > 0.5 {
        land = climate(f.z, f.w);
    }
    let river = smoothstep(0.02, 0.3, f.x) * (1.0 - wet);
    land = mix(land, linear(vec3<f32>(0.20, 0.42, 0.68)), river * 0.85);
    land = mix(land, linear(vec3<f32>(0.93, 0.96, 0.99)), smoothstep(0.85, 1.0, f.y));
    // The sea by its depth, its pack ice; a lake flat and fresh.
    let pack = smoothstep(0.55, 0.7, f.y) * (1.0 - smoothstep(0.85, 1.0, f.y));
    let deep = mix(bathymetric(max(-h, 0.0)), linear(vec3<f32>(0.86, 0.91, 0.95)), pack * 0.85);
    let lake = linear(vec3<f32>(0.33, 0.58, 0.76));
    var c = mix(land, mix(lake, deep, sea), wet) * relief;
    // Graticule every 30°.
    let g = vec2<f32>(lat, lon) * (6.0 / PI);
    let wg = vec2<f32>(fwidth(g.x), min(fwidth(g.y), fwidth(fract(g.y / 12.0 + 0.5) * 12.0)));
    let fr = abs(fract(g + 0.5) - 0.5) / max(wg, vec2<f32>(1e-5));
    c = mix(c, vec3<f32>(0.85), 0.2 * (1.0 - clamp(min(fr.x, fr.y), 0.0, 1.0)));
    // The lamp on the globe as a whole.
    c *= 0.4 + 0.7 * max(dot(vec3<f32>(q, z), normalize(vec3<f32>(-0.45, 0.5, 0.75))), 0.0);
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
