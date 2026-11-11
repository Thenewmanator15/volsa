// Which blocks of a label volume hold any part at all, worked out by the graphics device
// from the labels it holds: 1 for a block with a labelled voxel in it, 0 for one with none.

@group(0) @binding(0) var parts: texture_3d<u32>;

// The side of a block, in voxels. The same as PART_BLOCK in filled.rs and in parts.wgsl.
const BLOCK: i32 = 8;

@fragment
fn any_in_block(at: Across) -> @location(0) vec4<f32> {
    let voxels = vec3<i32>(textureDimensions(parts));
    let first = texel(at) * BLOCK;
    let last = min(first + vec3<i32>(BLOCK), voxels);
    var found = 0.0;
    for (var slice = first.z; slice < last.z; slice++) {
        for (var row = first.y; row < last.y; row++) {
            for (var column = first.x; column < last.x; column++) {
                if textureLoad(parts, vec3<i32>(column, row, slice), 0).r != 0u {
                    found = 1.0;
                }
            }
        }
    }
    return vec4<f32>(found, 0.0, 0.0, 1.0);
}
