// Distant terrain (`common.wgsl` is prepended): flat-topped LOD columns lit, tinted and faded
// like the full-detail terrain, drawn where it ends.

struct Tile { origin: vec4<f32> };
// A quad record (`hearth_lod::LodQuad`): see there for the packing.
struct Quad { a: u32, b: u32, c: u32, d: u32 };

@group(1) @binding(0) var<storage, read> tiles: array<Tile>;
@group(1) @binding(1) var<storage, read> quads: array<Quad>;

struct LodOut {
    @builtin(position) pos: vec4<f32>,
    // Camera-relative position.
    @location(0) world: vec3<f32>,
    @location(1) @interpolate(flat) albedo: vec3<f32>,
    @location(2) @interpolate(flat) normal: vec3<f32>,
    // Share of open water (not frozen) on a water surface; 0 elsewhere.
    @location(3) @interpolate(flat) water: f32,
};

// Face codes as the terrain's: down, up, north, south, west, east.
fn lod_normal(face: u32) -> vec3<f32> {
    switch face {
        case 0u: { return vec3<f32>(0.0, -1.0, 0.0); }
        case 1u: { return vec3<f32>(0.0, 1.0, 0.0); }
        case 2u: { return vec3<f32>(0.0, 0.0, -1.0); }
        case 3u: { return vec3<f32>(0.0, 0.0, 1.0); }
        case 4u: { return vec3<f32>(-1.0, 0.0, 0.0); }
        default: { return vec3<f32>(1.0, 0.0, 0.0); }
    }
}

// Linear colours of snow, sea ice and a leafless canopy (twigs over litter).
const SNOW: vec3<f32> = vec3<f32>(0.85, 0.87, 0.9);
const SEA_ICE: vec3<f32> = vec3<f32>(0.72, 0.78, 0.84);
const BARE: vec3<f32> = vec3<f32>(0.10, 0.08, 0.06);

@vertex
fn vs_lod(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> LodOut {
    // Four vertices per quad record (the index buffer makes two triangles of them).
    let q = quads[vi >> 2u];
    let corner = vi & 3u;
    let u = select(0.0, 1.0, corner == 1u || corner == 2u);
    let v = select(0.0, 1.0, corner >= 2u);
    let x0 = f32(q.a & 0x1fffu);
    let z0 = f32((q.a >> 13u) & 0x1fffu);
    let face = (q.a >> 26u) & 7u;
    let water = ((q.a >> 29u) & 1u) == 1u;
    let kind = ((q.a >> 30u) & 3u) | (((q.c >> 13u) & 3u) << 2u);
    let y0 = f32((i32(q.b) << 16u) >> 16u);
    let h = f32(q.b >> 16u);
    let w = f32(q.c & 0x1fffu);
    let climate = (q.c >> 15u) | ((q.d >> 24u) << 17u);
    // Width along X (Z for west and east faces), height along Z for tops and bottoms, along Y
    // for sides.
    var local: vec3<f32>;
    switch face {
        case 0u, 1u: { local = vec3<f32>(x0 + u * w, y0, z0 + v * h); }
        case 2u, 3u: { local = vec3<f32>(x0 + u * w, y0 + v * h, z0); }
        default: { local = vec3<f32>(x0, y0 + v * h, z0 + u * w); }
    }
    let world = curve(tiles[ii].origin.xyz + local);
    var albedo = unpack_rgb(q.d & 0xffffffu);
    if (kind & 3u) != 0u {
        albedo = albedo * resolve_tint(kind, climate);
    }
    // Deciduous canopies stand bare in winter and in the dry season.
    albedo = mix(BARE, albedo, tint_leaf(kind, climate));
    // Snow lies on tops through the cold months, and the sea freezes in hard winters.
    let c = decode_climate(climate);
    let t = season_temp(c, g.camera.w);
    if face == 1u {
        if water {
            albedo = mix(albedo, SEA_ICE, 1.0 - smoothstep(-6.0, -3.0, t));
        } else {
            let snow = (1.0 - smoothstep(-3.0, 0.0, t)) * smoothstep(100.0, 400.0, c.precip);
            albedo = mix(albedo, SNOW, snow * select(1.0, 0.6, (kind & 3u) >= 2u));
        }
    }
    var out: LodOut;
    out.pos = g.view_proj * vec4<f32>(world, 1.0);
    // A hair farther than it is, so the full-detail terrain wins where both lie.
    out.pos.z *= 0.9998;
    out.world = world;
    out.albedo = albedo;
    out.normal = lod_normal(face);
    out.water = 0.0;
    if face == 1u && water {
        out.water = smoothstep(-6.0, -3.0, t);
    }
    return out;
}

@fragment
fn fs_lod(in: LodOut) -> @location(0) vec4<f32> {
    // Screen gradients for the waves, taken where control flow is uniform.
    let gx = dpdx(in.world);
    let gy = dpdy(in.world);
    // Inside the full-detail area the cubes are the ground. Across the band at its edge the
    // cubes thin out (dithered) and the LOD, just behind them, shows through their gaps.
    if near_weight(in.world.xz) >= 0.999 {
        discard;
    }
    // Lit like an open full-detail face: sky light by orientation, direct light by angle.
    let n = in.normal;
    let ambient = g.sky_light.rgb * max(0.62 + 0.38 * n.y + 0.1 * (1.0 - abs(n.y)), 0.2)
        + vec3<f32>(g.sky_light.a);
    let direct = g.sun_light.rgb * max(dot(n, g.sun.xyz), 0.0);
    var c = in.albedo * (ambient + direct) / 3.14159265;
    if in.water > 0.0 {
        // Open water as near water looks: the colour the column was given (the bed through the
        // water and the light the water scatters back) under the sky's reflection and the
        // sun's glitter on the same waves.
        let dist = length(in.world);
        let view = -in.world / max(dist, 1e-3);
        let slope = wave_slope_far(in.world.xz + g.camera.xz, gx.xz, gy.xz);
        let wn = normalize(vec3<f32>(-slope.x, 1.0, -slope.y));
        let f = water_fresnel(dot(wn, view));
        let w = mix(c, water_sky(reflect(-view, wn)), f) + water_glitter(wn, view, dist, 1.0);
        c = mix(c, w, in.water);
    }
    return vec4<f32>(aerial(c, in.world), 1.0);
}
