// A person's data (Amendment E §8, E7), shared by the skin, hair and eye shaders, in the
// preview and in the world. A view prefix comes before it (`person_preview.wgsl` or
// `common.wgsl` with `person_world.wgsl`), giving `clip`, `eye_pos`, `sun_dir`, `shade_sun`,
// `sun_rgb`, `ambient` and `finish`.

struct Person {
    skin: vec4<f32>,
    lips: vec4<f32>,
    nail: vec4<f32>,
    hair: vec4<f32>,
    palette: array<mat4x4<f32>, 17>,
    // The skin's state: x wet, y tan, z sunburn, w pallor; then flush, goosebumps.
    state: vec4<f32>,
    state2: vec4<f32>,
    // Per joint: dirt, blood, scar, and how covered (no sun there).
    marks: array<vec4<f32>, 17>,
    // The garment's colour (linear); w: 0 hide, 1 plant fibre.
    cloth: vec4<f32>,
    // The light where the person is: x sky (0–1), y firelight (0–1); z 1 when `shadow_joints`
    // holds the neck and the head as posed, drawn elsewhere (first person) but kept in the
    // shadow.
    light: vec4<f32>,
    shadow_joints: array<mat4x4<f32>, 2>,
};

struct Hair {
    // Linear albedo; w: how wet (0–1).
    color: vec4<f32>,
    // Each guide's points' offsets from the style (32 guides of 5).
    guides: array<vec4<f32>, 160>,
};

struct Eyes {
    parts: array<mat4x4<f32>, 6>,
    // The iris's colour; w: the pupil's size (0–1).
    iris: vec4<f32>,
};

@group(1) @binding(0) var<uniform> u: Person;
@group(1) @binding(1) var<uniform> hair: Hair;
@group(1) @binding(2) var<uniform> eyes: Eyes;
