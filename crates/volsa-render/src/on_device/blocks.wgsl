// The largest value in each block of voxels, worked out by the graphics device from the
// volume it holds. Values are stored from 0 to 1, so the largest of none is 0.

@group(0) @binding(0) var values: texture_3d<f32>;

// The side of a block, in voxels. The same as BLOCK in prepared.rs.
const BLOCK: i32 = 8;

// The largest value from `first` up to, and not including, `last`, each way.
fn largest_between(first: vec3<i32>, last: vec3<i32>) -> f32 {
    var largest = 0.0;
    for (var slice = first.z; slice < last.z; slice++) {
        for (var row = first.y; row < last.y; row++) {
            for (var column = first.x; column < last.x; column++) {
                largest = max(largest, textureLoad(values, vec3<i32>(column, row, slice), 0).r);
            }
        }
    }
    return largest;
}

// Drawn into a texture of one texel a block: the largest value among the block's voxels.
// Blocks at the far faces hold fewer voxels where the volume is not a whole number of them.
@fragment
fn largest_in_block(at: Across) -> @location(0) vec4<f32> {
    let voxels = vec3<i32>(textureDimensions(values));
    let first = texel(at) * BLOCK;
    let largest = largest_between(first, min(first + vec3<i32>(BLOCK), voxels));
    return vec4<f32>(largest, 0.0, 0.0, 1.0);
}

// Drawn from that texture into another like it: the largest of a block's own figure and
// those of the blocks touching it, corners included. A value read anywhere in a block is
// blended from voxels that may lie just over its faces; after this its figure covers them.
@fragment
fn largest_about(at: Across) -> @location(0) vec4<f32> {
    let blocks = vec3<i32>(textureDimensions(values));
    let here = texel(at);
    let first = max(here - vec3<i32>(1), vec3<i32>(0));
    let largest = largest_between(first, min(here + vec3<i32>(2), blocks));
    return vec4<f32>(largest, 0.0, 0.0, 1.0);
}
