// People seen on their own (the creator's preview): a camera and a light of their own, the
// result tone-mapped for the display.

struct Scene {
    view_proj: mat4x4<f32>,
    eye: vec4<f32>,
    light_dir: vec4<f32>,
    light: vec4<f32>,
    sky: vec4<f32>,
    ground: vec4<f32>,
    // x: exposure; y: 1 to encode the display's gamma.
    exposure: vec4<f32>,
};

@group(0) @binding(0) var<uniform> scene: Scene;

fn clip(world: vec3<f32>) -> vec4<f32> {
    return scene.view_proj * vec4<f32>(world, 1.0);
}

fn eye_pos() -> vec3<f32> {
    return scene.eye.xyz;
}

fn sun_dir() -> vec3<f32> {
    return scene.light_dir.xyz;
}

fn sun_rgb() -> vec3<f32> {
    return scene.light.rgb;
}

// The light from all round on a surface facing `n`.
fn ambient(n: vec3<f32>) -> vec3<f32> {
    return mix(scene.ground.rgb, scene.sky.rgb, 0.5 + 0.5 * n.y);
}

fn tonemap(x: vec3<f32>) -> vec3<f32> {
    return clamp(x * (2.51 * x + 0.03) / (x * (2.43 * x + 0.59) + 0.14), vec3<f32>(0.0), vec3<f32>(1.0));
}

fn finish(lit: vec3<f32>, world: vec3<f32>) -> vec4<f32> {
    var c = tonemap(lit * scene.exposure.x);
    if scene.exposure.y > 0.5 {
        c = pow(c, vec3<f32>(1.0 / 2.2));
    }
    return vec4<f32>(c, 1.0);
}
