// Trees and shrubs as meshes (`common.wgsl` is prepended; Amendment S §7.1–7.2): their wood
// as bark by the species' pattern, their leaves on cards painted by leaf kind and alpha-tested,
// swaying in the wind (the whole tree slowly, branches faster, leaves fluttering), coloured by
// place and season as the terrain's leaves are, the deciduous dropping their leaves card by
// card in autumn, and lit as the terrain is: the sky's light by the light level where the tree
// stands, the sun where it is open, light through the leaves from behind.

struct TreeFrame {
    // xyz: the wind's direction and w its speed (m/s).
    wind: vec4<f32>,
};

@group(1) @binding(0) var<uniform> frame: TreeFrame;

struct TreeIn {
    @location(0) pos: vec3<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) normal: vec4<f32>,
    @location(3) sway: vec4<u32>,
    @location(4) extra: vec4<u32>,
    // The instance: where its skeleton's origin is (camera-relative), its bark (sRGB, pattern
    // and flags), the turn in the ground's plane, its leaves (sRGB and tint kind), the climate
    // code and snow, and the light levels where it stands with its sway's own phase.
    @location(5) origin: vec3<f32>,
    @location(6) bark: u32,
    @location(7) turn: vec4<f32>,
    @location(8) leaf: u32,
    @location(9) climate: u32,
    @location(10) light: u32,
};

struct TreeOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) world: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) @interpolate(flat) albedo: vec3<f32>,
    // What it is (0 wood, 1 leaf, 2 end grain), the leaf kind or bark pattern, the card's
    // number, the snow on it (0..255).
    @location(4) @interpolate(flat) info: vec4<u32>,
    // Sky and block light levels (0..1), the wood's radius (m).
    @location(5) @interpolate(flat) light: vec3<f32>,
};

const BARK_CHARRED: u32 = 0x10u;

fn turned(t: vec4<f32>, p: vec3<f32>) -> vec3<f32> {
    return vec3<f32>(t.x * p.x + t.y * p.z, p.y, t.z * p.x + t.w * p.z);
}

fn hash12(p: vec2<f32>) -> f32 {
    var q = fract(p * vec2<f32>(123.34, 456.21));
    q += dot(q, q + 45.32);
    return fract(q.x * q.y);
}

fn hash22(p: vec2<f32>) -> vec2<f32> {
    let n = hash12(p);
    return vec2<f32>(n, hash12(p + n * 17.17 + 3.1));
}

fn vnoise2(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = p - i;
    let u = f * f * (3.0 - 2.0 * f);
    let a = hash12(i);
    let b = hash12(i + vec2<f32>(1.0, 0.0));
    let c = hash12(i + vec2<f32>(0.0, 1.0));
    let d = hash12(i + vec2<f32>(1.0, 1.0));
    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}

@vertex
fn vs_tree(v: TreeIn) -> TreeOut {
    var out: TreeOut;
    let what = u32(round(max(v.normal.w, 0.0) * 127.0));
    let tint_kind = (v.leaf >> 24u) & 15u;
    let climate = v.climate & 0xffffffu;
    // Fallen leaves: a card goes once its number passes the tree's leaf cover.
    if what == 1u && f32(v.sway.z) / 255.0 >= tint_leaf(tint_kind, climate) {
        out.pos = vec4<f32>(0.0, 0.0, 0.0, 1.0);
        return out;
    }
    // Swaying: the tree bends with the wind and back (its top a few decimetres in a fresh
    // breeze), branches nod faster, leaves flutter.
    let w = f32(v.sway.x) / 255.0;
    let phase = (f32(v.sway.y) + f32((v.light >> 8u) & 255u)) / 255.0 * TAU;
    let t = g.params.x;
    let speed = frame.wind.w;
    let dir = turned_inverse(v.turn, frame.wind.xyz);
    var local = v.pos;
    let gust = 0.65 + 0.35 * sin(t * 0.45 + phase * 0.3);
    local += dir * w * 0.004 * speed * speed * gust * (1.0 + 0.25 * sin(t * 1.1 + phase));
    local += vec3<f32>(sin(t * 1.9 + phase * 2.0), 0.35 * sin(t * 2.7 + phase), cos(t * 1.6 + phase * 3.0))
        * w * w * 0.015 * speed;
    if what == 1u {
        local += vec3<f32>(sin(t * 7.3 + phase * 5.0 + v.pos.x * 2.0), sin(t * 8.1 + phase * 3.0), cos(t * 6.7 + phase * 7.0 + v.pos.z * 2.0))
            * 0.006 * speed;
    }
    let world = v.origin + turned(v.turn, local);
    out.pos = g.view_proj * vec4<f32>(curve(world), 1.0);
    out.world = world;
    out.normal = normalize(turned(v.turn, v.normal.xyz));
    out.uv = v.uv;
    if what == 1u {
        // The species' leaf colour, tinted by place and season as the terrain's leaves are,
        // each card a little lighter or darker.
        var c = unpack_rgb(v.leaf & 0xffffffu);
        if (tint_kind & 3u) != 0u {
            c = c * resolve_tint(tint_kind, climate);
        }
        out.albedo = c * (0.82 + 0.36 * f32(v.sway.z) / 255.0);
        out.info = vec4<u32>(1u, v.extra.x, v.sway.z, v.climate >> 24u);
    } else {
        var c = unpack_rgb(v.bark & 0xffffffu);
        if ((v.bark >> 24u) & BARK_CHARRED) != 0u {
            c = vec3<f32>(0.025, 0.022, 0.02);
        }
        out.albedo = c;
        out.info = vec4<u32>(what, (v.bark >> 24u) & 15u, 0u, v.climate >> 24u);
    }
    out.light = vec3<f32>(
        f32(v.light & 15u) / 15.0,
        f32((v.light >> 4u) & 15u) / 15.0,
        f32(v.extra.y) / 100.0
    );
    return out;
}

// The wind's direction in the tree's own frame (the inverse of its turn).
fn turned_inverse(t: vec4<f32>, d: vec3<f32>) -> vec3<f32> {
    let det = t.x * t.w - t.y * t.z;
    let inv = vec4<f32>(t.w, -t.y, -t.z, t.x) / select(det, 1.0, abs(det) < 1e-6);
    return turned(inv, d);
}

fn level_curve(l: f32) -> f32 {
    return l / (4.0 - 3.0 * l);
}

// Lit as the terrain is: the sky's light by the level where the tree stands and by the way it
// faces, the sun where it gets through (`sun_open`), firelight; aerial perspective on the way to
// the eye.
fn tree_light(albedo: vec3<f32>, n: vec3<f32>, light: vec3<f32>, through: f32, world: vec3<f32>) -> vec3<f32> {
    let sky_dir = 0.62 + 0.38 * n.y + 0.1 * (1.0 - abs(n.y));
    let ambient = g.sky_light.rgb * level_curve(light.x) * max(sky_dir, 0.2)
        + vec3<f32>(g.sky_light.a) + g.block_light.rgb * level_curve(light.y);
    let sun = g.sun_light.rgb * sun_open(curve(world), n, light.x);
    var c = albedo * (ambient + sun * max(dot(n, g.sun.xyz), 0.0)) / 3.14159265;
    // Light through thin leaves from behind them: yellower, about a third of what falls on them.
    c += albedo * vec3<f32>(1.1, 1.15, 0.6) * sun * max(-dot(n, g.sun.xyz), 0.0) * through
        / 3.14159265;
    return c;
}

// Bark by pattern (`BarkPattern`): around (turns) and along (m) its stem.
fn bark(uv: vec2<f32>, pattern: u32, r: f32) -> f32 {
    let u = uv.x;
    let v = uv.y;
    switch pattern {
        // Furrowed: deep vertical furrows, wandering.
        case 0u: {
            let w = u * 6.0 + vnoise2(vec2<f32>(u * 4.0, v * 1.5)) * 1.6;
            let ridge = abs(fract(w) - 0.5) * 2.0;
            return 0.55 + 0.6 * smoothstep(0.15, 0.6, ridge) * (0.8 + 0.4 * vnoise2(vec2<f32>(u * 20.0, v * 6.0)));
        }
        // Smooth and grey, mottled.
        case 1u: {
            return 0.9 + 0.2 * vnoise2(vec2<f32>(u * 8.0, v * 2.0)) - 0.15 * smoothstep(0.75, 0.9, vnoise2(vec2<f32>(u * 3.0, v * 0.7)));
        }
        // Peeling (birch): white, dark lenticel dashes across, dark patches at the foot.
        case 2u: {
            let dash = step(0.93, vnoise2(vec2<f32>(u * 3.0, v * 14.0))) * step(0.5, vnoise2(vec2<f32>(u * 9.0, v * 2.0)));
            let patch_ = smoothstep(0.62, 0.78, vnoise2(vec2<f32>(u * 2.5, v * 1.2))) * smoothstep(6.0, 0.5, v);
            return 1.0 - 0.75 * max(dash, patch_);
        }
        // Scaly plates.
        case 3u: {
            let p = vec2<f32>(u * 7.0, v * 3.5);
            let cell = floor(p);
            var edge = 1.0;
            for (var j = -1; j <= 1; j++) {
                for (var i = -1; i <= 1; i++) {
                    let c = cell + vec2<f32>(f32(i), f32(j));
                    let d = length(p - c - hash22(c));
                    edge = min(edge, d);
                }
            }
            return 0.55 + 0.55 * smoothstep(0.05, 0.4, edge);
        }
        // Long shallow fissures.
        case 4u: {
            let w = u * 9.0 + vnoise2(vec2<f32>(u * 3.0, v * 0.8)) * 1.2;
            return 0.75 + 0.3 * smoothstep(0.1, 0.35, abs(fract(w) - 0.5));
        }
        // Bands of lenticels across.
        case 5u: {
            let band = step(0.85, fract(v * 9.0 + vnoise2(vec2<f32>(u * 2.0, v)) * 0.3));
            return 0.95 - 0.45 * band * step(0.4, vnoise2(vec2<f32>(u * 12.0, v * 9.0)));
        }
        // Interlacing ridges.
        case 6u: {
            let a = abs(sin((u * 5.0 + v * 1.4) * 3.14159));
            let b = abs(sin((u * 5.0 - v * 1.4) * 3.14159));
            return 0.6 + 0.5 * smoothstep(0.2, 0.8, max(a, b) * 0.6 + 0.4 * vnoise2(vec2<f32>(u * 9.0, v * 3.0)));
        }
        // Flaky.
        default: {
            return 0.7 + 0.45 * step(0.45, vnoise2(vec2<f32>(u * 10.0, v * 4.0)));
        }
    }
}

@fragment
fn fs_wood(in: TreeOut) -> @location(0) vec4<f32> {
    var albedo = in.albedo;
    var n = normalize(in.normal);
    if in.info.x == 2u {
        // End grain: pale wood in growth rings, a darker heart.
        let r = length(in.uv);
        let rings = 0.85 + 0.15 * sin(r * in.light.z * 100.0 * 1.2);
        albedo = vec3<f32>(0.55, 0.42, 0.27) * rings * mix(0.75, 1.0, smoothstep(0.1, 0.6, r));
    } else {
        albedo = albedo * bark(in.uv, in.info.y, in.light.z);
    }
    albedo = snowed(albedo, n, in.info.w);
    return vec4<f32>(aerial(tree_light(albedo, n, in.light, 0.0, in.world), in.world), 1.0);
}

// Snow lying on what faces up.
fn snowed(albedo: vec3<f32>, n: vec3<f32>, snow: u32) -> vec3<f32> {
    if snow == 0u {
        return albedo;
    }
    let s = f32(snow) / 255.0 * smoothstep(0.25, 0.7, n.y);
    return mix(albedo, vec3<f32>(0.80, 0.82, 0.86), s);
}

// How much of a card is leaf at a point, by leaf kind.
fn leaf_shape(uv: vec2<f32>, kind: u32, id: f32) -> f32 {
    switch kind {
        // Needle sprays: a shoot up the middle, needles from it reaching forward.
        case 1u: {
            let x = (uv.x - 0.5) * 2.0;
            let width = pow(max(sin(3.14159 * uv.y), 0.0), 0.6) * 0.95;
            if abs(x) > width {
                return 0.0;
            }
            let along = uv.y * 26.0 - abs(x) * 7.0 + id * 3.0;
            return max(step(fract(along), 0.32), step(abs(x), 0.05));
        }
        // Scale-leaved sprays: rows of small overlapping scales on flat fans.
        case 2u: {
            let x = (uv.x - 0.5) * 2.0;
            let width = pow(max(sin(3.14159 * uv.y), 0.0), 0.8) * 0.9;
            let lobes = 0.85 + 0.15 * sin(uv.y * 40.0 + id * 6.0);
            return step(abs(x), width * lobes) * step(0.25, vnoise2(uv * vec2<f32>(14.0, 28.0) + id * 9.0));
        }
        // Palm fronds: leaflets from the rachis, long and narrow.
        case 3u: {
            let x = (uv.x - 0.5) * 2.0;
            let width = 0.25 + 0.75 * pow(max(sin(3.14159 * uv.y), 0.0), 0.5);
            if abs(x) > width {
                return 0.0;
            }
            let along = uv.y * 18.0 - abs(x) * 2.2 + id;
            return max(step(fract(along), 0.42), step(abs(x), 0.03));
        }
        // Broad leaves: a few on the card, each a pointed oval turned its own way.
        default: {
            let r = length(uv - 0.5) * 2.0;
            if r > 1.0 {
                return 0.0;
            }
            let p = uv * 3.4;
            let cell = floor(p);
            var hit = 0.0;
            for (var j = -1; j <= 1; j++) {
                for (var i = -1; i <= 1; i++) {
                    let c = cell + vec2<f32>(f32(i), f32(j));
                    let h = hash22(c + id * 7.13);
                    let centre = c + 0.2 + 0.6 * h;
                    let a = h.x * 6.2832;
                    let d = p - centre;
                    let q = vec2<f32>(d.x * cos(a) + d.y * sin(a), -d.x * sin(a) + d.y * cos(a));
                    // Pointed at both ends: narrower toward the tips.
                    let along = q.x / 0.62;
                    let half = 0.3 * sqrt(max(1.0 - along * along, 0.0));
                    if abs(along) < 1.0 && abs(q.y) < half {
                        hit = 1.0;
                    }
                }
            }
            return hit * step(r, 0.98);
        }
    }
}

// A card casts the shape of its leaves into the sun's shadow maps.
@fragment
fn fs_leaf_shadow(in: TreeOut) {
    if leaf_shape(in.uv, in.info.y, f32(in.info.z) / 255.0) < 0.5 {
        discard;
    }
}

@fragment
fn fs_leaf(in: TreeOut, @builtin(front_facing) front: bool) -> @location(0) vec4<f32> {
    let id = f32(in.info.z) / 255.0;
    if leaf_shape(in.uv, in.info.y, id) < 0.5 {
        discard;
    }
    var n = normalize(in.normal);
    // Lighter at the crown's outside, darker within (the card's light is the crown's).
    let albedo = snowed(in.albedo, n, in.info.w);
    let inside = 0.75 + 0.25 * smoothstep(-0.3, 0.6, n.y);
    let c = tree_light(albedo, n, in.light, 0.35, in.world) * inside;
    return vec4<f32>(aerial(c, in.world), 1.0);
}
