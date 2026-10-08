// Distant terrain (`common.wgsl` and `water.wgsl` are prepended): the ground as smooth height
// fields and the trees' crowns and trunks as boxes, lit, tinted and faded like the full-detail
// terrain, drawn where it ends.

struct Tile { origin: vec4<f32> };
// A quad record (`hearth_lod::LodQuad`): see there for the packing.
struct Quad { a: u32, b: u32, c: u32, d: u32 };
// A tile's ground: its origin (camera-relative) and column size, its first corner in the pool
// and its skirts' depth.
struct GroundTile { origin: vec4<f32>, base: u32, skirt: f32, pad0: u32, pad1: u32 };
// A corner of the ground (`hearth_lod::GroundVertex`): see there for the packing.
struct GroundVert { y: i32, n: u32, c: u32, m: u32 };
// A ground material, as the smooth near ground's (`terrain.wgsl`).
struct GroundMat { color: vec4<f32>, color2: vec4<f32>, tint: u32, relief: f32, strata: f32, pad1: f32 };

@group(1) @binding(0) var<storage, read> tiles: array<Tile>;
@group(1) @binding(1) var<storage, read> quads: array<Quad>;
@group(1) @binding(2) var<storage, read> gtiles: array<GroundTile>;
@group(1) @binding(3) var<storage, read> gverts: array<GroundVert>;
@group(1) @binding(4) var<storage, read> gmats: array<GroundMat>;

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

// Lit like an open full-detail face: sky light by orientation, direct light by angle; water as
// near water looks, `water` its share of open water.
fn lod_light(albedo: vec3<f32>, n: vec3<f32>, world: vec3<f32>, water: f32, gx: vec3<f32>, gy: vec3<f32>) -> vec3<f32> {
    let ambient = g.sky_light.rgb * max(0.62 + 0.38 * n.y + 0.1 * (1.0 - abs(n.y)), 0.2)
        + vec3<f32>(g.sky_light.a);
    let direct = g.sun_light.rgb * max(dot(n, g.sun.xyz), 0.0);
    var c = albedo * (ambient + direct) / 3.14159265;
    if water > 0.0 {
        // Open water: the colour the column was given (the bed through the water and the light
        // the water scatters back) under the sky's reflection and the sun's glitter on the
        // same waves.
        let dist = length(world);
        let view = -world / max(dist, 1e-3);
        let slope = wave_slope_far(world.xz + g.camera.xz, gx.xz, gy.xz);
        let wn = normalize(vec3<f32>(-slope.x, 1.0, -slope.y));
        let f = water_fresnel(dot(wn, view));
        let w = mix(c, water_sky(reflect(-view, wn)), f) + water_glitter(wn, view, dist, 1.0);
        c = mix(c, w, water);
    }
    return c;
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
    return vec4<f32>(aerial(lod_light(in.albedo, in.normal, in.world, in.water, gx, gy), in.world), 1.0);
}

// ---------------------------------------------------------------- the ground (S §5)

struct GroundOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) world: vec3<f32>,
    @location(1) albedo: vec3<f32>,
    @location(2) normal: vec3<f32>,
    // Share of open water, and of the ground under a crown.
    @location(3) water: f32,
    @location(4) shade: f32,
};

// Corners along a tile side, and the vertices of its height field (skirts' come after).
const SIDE: u32 = 33u;

// A share of the grass's bare patches, on average (`fs_smooth`'s `bare`).
const BARE_SHARE: f32 = 0.08;

// A ground material's mean albedo, as `fs_smooth` blends it over its noise.
fn ground_mean(slot: u32, climate: u32) -> vec3<f32> {
    let m = gmats[slot];
    var col = mix(m.color.rgb, m.color2.rgb, 0.5);
    if m.tint == 1u {
        let sward = resolve_tint(1u, climate) * 0.435;
        col = mix(sward, m.color2.rgb * 0.8, BARE_SHARE * 0.6);
    }
    // Wet ground is darker.
    return col * mix(1.0, 0.6, g.block_light.w);
}

@vertex
fn vs_ground(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> GroundOut {
    let t = gtiles[ii];
    // The height field's corners; then the skirts' lower edges, north, south, west and east.
    var k = vi;
    var drop = 0.0;
    if vi >= SIDE * SIDE {
        let e = (vi - SIDE * SIDE) / SIDE;
        let s = (vi - SIDE * SIDE) % SIDE;
        switch e {
            case 0u: { k = s; }
            case 1u: { k = (SIDE - 1u) * SIDE + s; }
            case 2u: { k = s * SIDE; }
            default: { k = s * SIDE + SIDE - 1u; }
        }
        drop = t.skirt;
    }
    let v = gverts[t.base + k];
    let cs = t.origin.w;
    let local = vec3<f32>(f32(k % SIDE) * cs, f32(v.y) / 16.0 - drop, f32(k / SIDE) * cs);
    let world = curve(t.origin.xyz + local);
    let nx = f32((i32(v.n) << 20u) >> 20u) / 2047.0;
    let nz = f32((i32(v.n) << 8u) >> 20u) / 2047.0;
    let n = vec3<f32>(nx, sqrt(max(1.0 - nx * nx - nz * nz, 0.0)), nz);
    let kind = v.n >> 24u;
    let slot = v.c >> 24u;
    let climate = v.m & 0xffffffu;
    let water = f32((v.m >> 24u) & 15u) / 4.0;
    var albedo: vec3<f32>;
    if slot != 255u {
        albedo = ground_mean(slot, climate);
    } else {
        albedo = unpack_rgb(v.c & 0xffffffu);
        if (kind & 3u) != 0u {
            albedo = albedo * resolve_tint(kind, climate);
        }
    }
    // Snow lies through the cold months, less on steep ground; the sea freezes in hard
    // winters.
    let c = decode_climate(climate);
    let temp = season_temp(c, g.camera.w);
    let snow = (1.0 - smoothstep(-3.0, 0.0, temp)) * smoothstep(100.0, 400.0, c.precip)
        * smoothstep(0.55, 0.85, n.y);
    let ice = 1.0 - smoothstep(-6.0, -3.0, temp);
    albedo = mix(albedo, SNOW, snow * (1.0 - water));
    albedo = mix(albedo, SEA_ICE, ice * water);
    var out: GroundOut;
    out.pos = g.view_proj * vec4<f32>(world, 1.0);
    // A hair farther than it is, so the full-detail terrain wins where both lie.
    out.pos.z *= 0.9998;
    out.world = world;
    out.albedo = albedo;
    out.normal = n;
    out.water = water * (1.0 - ice);
    out.shade = f32(v.m >> 28u) / 4.0;
    return out;
}

@fragment
fn fs_ground(in: GroundOut) -> @location(0) vec4<f32> {
    let gx = dpdx(in.world);
    let gy = dpdy(in.world);
    if near_weight(in.world.xz) >= 0.999 {
        discard;
    }
    let n = normalize(in.normal);
    var c = lod_light(in.albedo, n, in.world, in.water, gx, gy);
    if in.shade > 0.0 {
        // Under the crowns: the sky's light from below them, no sun.
        let under = lod_light(in.albedo, vec3<f32>(0.0, -1.0, 0.0), in.world, 0.0, gx, gy);
        c = mix(c, under, in.shade);
    }
    return vec4<f32>(aerial(c, in.world), 1.0);
}
