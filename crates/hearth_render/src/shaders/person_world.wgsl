// People in the world (after `common.wgsl`): camera-relative, the planet's curve, lit as the
// terrain and the figures are (the sun where the sky is open, the sky's light and firelight by
// the light levels where the person stands), into the HDR target through aerial perspective.

fn clip(world: vec3<f32>) -> vec4<f32> {
    return g.view_proj * vec4<f32>(curve(world), 1.0);
}

fn eye_pos() -> vec3<f32> {
    return vec3<f32>(0.0);
}

fn sun_dir() -> vec3<f32> {
    return g.sun.xyz;
}

// The share of the sun reaching the point being shaded (`shade_sun`).
var<private> sun_share: f32 = 1.0;

// Looks up how much of the sun reaches a point of the person (before `sun_rgb`): by the shadow
// maps, or where the sky is open where the person stands.
fn shade_sun(world: vec3<f32>, n: vec3<f32>) {
    sun_share = sun_open(curve(world), n, u.light.x);
}

fn sun_rgb() -> vec3<f32> {
    return g.sun_light.rgb * sun_share;
}

fn level_curve(l: f32) -> f32 {
    return l / (4.0 - 3.0 * l);
}

fn ambient(n: vec3<f32>) -> vec3<f32> {
    let sky_dir = 0.62 + 0.38 * n.y + 0.1 * (1.0 - abs(n.y));
    return g.sky_light.rgb * level_curve(u.light.x) * max(sky_dir, 0.2) + vec3<f32>(g.sky_light.a)
        + g.block_light.rgb * level_curve(u.light.y);
}

fn finish(lit: vec3<f32>, world: vec3<f32>) -> vec4<f32> {
    return vec4<f32>(aerial(lit, world), 1.0);
}
