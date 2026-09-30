// Hierarchical depth (Hi-Z) pyramid for occlusion culling. Reverse-Z: each texel keeps the
// farthest (smallest) depth of the pixels it covers. Level 0 is half the depth resolution,
// rounded up, so its last texel may cover a single pixel. Higher levels use the texture's own
// mip sizes (halved, rounded down); when the level below has an odd size, the last row/column
// also takes in the extra texel, so every texel of every level is covered.

@group(0) @binding(0) var depth: texture_depth_2d;
@group(0) @binding(1) var src: texture_2d<f32>;
@group(0) @binding(2) var dst: texture_storage_2d<r32float, write>;

@compute @workgroup_size(8, 8)
fn from_depth(@builtin(global_invocation_id) id: vec3<u32>) {
    let d = textureDimensions(dst);
    if id.x >= d.x || id.y >= d.y {
        return;
    }
    let s = vec2<i32>(textureDimensions(depth)) - 1;
    let p = vec2<i32>(id.xy) * 2;
    let a = textureLoad(depth, min(p, s), 0);
    let b = textureLoad(depth, min(p + vec2<i32>(1, 0), s), 0);
    let c = textureLoad(depth, min(p + vec2<i32>(0, 1), s), 0);
    let e = textureLoad(depth, min(p + vec2<i32>(1, 1), s), 0);
    textureStore(dst, vec2<i32>(id.xy), vec4<f32>(min(min(a, b), min(c, e)), 0.0, 0.0, 1.0));
}

@compute @workgroup_size(8, 8)
fn downsample(@builtin(global_invocation_id) id: vec3<u32>) {
    let d = textureDimensions(dst);
    if id.x >= d.x || id.y >= d.y {
        return;
    }
    let sd = vec2<i32>(textureDimensions(src));
    let p = vec2<i32>(id.xy) * 2;
    // Two texels per axis, three on the last row/column when the source size is odd.
    let last = id.xy == d - 1u;
    let odd = (sd & vec2<i32>(1)) == vec2<i32>(1);
    let extra = select(vec2<i32>(0), vec2<i32>(1), last & odd);
    let end = min(p + 1 + extra, sd - 1);
    var m = 1.0;
    for (var y = p.y; y <= end.y; y++) {
        for (var x = p.x; x <= end.x; x++) {
            m = min(m, textureLoad(src, vec2<i32>(x, y), 0).r);
        }
    }
    textureStore(dst, vec2<i32>(id.xy), vec4<f32>(m, 0.0, 0.0, 1.0));
}
