// The tree looked at, drawn into the highlight's mask (`outline.rs`; `common.wgsl` and
// `tree.wgsl` are prepended): placed and swaying as `tree_vertex` places it, but with the camera
// unjittered and pulled a little toward the eye (its pixels unchanged), so that only what of it
// is seen is marked — its wood, or its leaves cut to their shapes as `fs_leaf` cuts them.

struct TreeMask {
    // Clip space from camera-relative space, without the jitter.
    view_proj: mat4x4<f32>,
    // x how far toward the eye it is drawn (a scale on its distance).
    params: vec4<f32>,
};

@group(2) @binding(0) var<uniform> m: TreeMask;

@vertex
fn vs_tree_mask(v: TreeIn) -> TreeOut {
    var out = tree_vertex(v);
    // A leaf fallen: left where `tree_vertex` put it, out of sight.
    if all(out.world == vec3<f32>(0.0)) {
        return out;
    }
    out.pos = m.view_proj * vec4<f32>(curve(out.world) * m.params.x, 1.0);
    return out;
}

@fragment
fn fs_mask_wood(in: TreeOut) -> @location(0) vec4<f32> {
    return vec4<f32>(1.0, 0.0, 0.0, 1.0);
}

@fragment
fn fs_mask_leaf(in: TreeOut) -> @location(0) vec4<f32> {
    if leaf_shape(in.uv, in.info.y, f32(in.info.z) / 255.0) < 0.5 {
        discard;
    }
    return vec4<f32>(1.0, 0.0, 0.0, 1.0);
}
