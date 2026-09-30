// Distant terrain (`common.wgsl` is prepended): flat-topped LOD columns lit, tinted and faded
// like the full-detail terrain, drawn where it ends.

struct Tile { origin: vec4<f32> };

@group(1) @binding(0) var<storage, read> tiles: array<Tile>;

struct LodIn {
    // Tile-local X and Z in blocks (u16 each), absolute Y.
    @location(0) xz: u32,
    @location(1) y: i32,
    // sRGB colour of the block's texture, face (3 bits), tint kind (4 bits), water flag.
    @location(2) color: u32,
    // Climate code of the column (seasonal tints, snow, sea ice).
    @location(3) climate: u32,
};

struct LodOut {
    @builtin(position) pos: vec4<f32>,
    // Camera-relative position.
    @location(0) world: vec3<f32>,
    @location(1) @interpolate(flat) albedo: vec3<f32>,
    @location(2) @interpolate(flat) normal: vec3<f32>,
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
fn vs_lod(v: LodIn, @builtin(instance_index) ii: u32) -> LodOut {
    let local = vec3<f32>(f32(v.xz & 0xffffu), f32(v.y), f32(v.xz >> 16u));
    let world = curve(tiles[ii].origin.xyz + local);
    let face = (v.color >> 24u) & 7u;
    let kind = (v.color >> 27u) & 15u;
    let water = (v.color >> 31u) == 1u;
    var albedo = unpack_rgb(v.color & 0xffffffu);
    if (kind & 3u) != 0u {
        albedo = albedo * resolve_tint(kind, v.climate);
    }
    // Deciduous canopies stand bare in winter and in the dry season.
    albedo = mix(BARE, albedo, tint_leaf(kind, v.climate));
    // Snow lies on tops through the cold months, and the sea freezes in hard winters.
    let c = decode_climate(v.climate);
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
    return out;
}

@fragment
fn fs_lod(in: LodOut) -> @location(0) vec4<f32> {
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
    let c = in.albedo * (ambient + direct) / 3.14159265;
    return vec4<f32>(aerial(c, in.world), 1.0);
}
