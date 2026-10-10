// One triangle over the whole of a slice of a 3D texture. Which slice is being drawn into is
// the number of the instance, passed on to the fragment shader as it is.

struct Across {
    @builtin(position) position: vec4<f32>,
    @location(0) @interpolate(flat) slice: u32,
}

@vertex
fn across(@builtin(vertex_index) corner: u32, @builtin(instance_index) slice: u32) -> Across {
    // Corners at (-1, -1), (3, -1) and (-1, 3): a triangle that covers the whole target.
    let x = f32(i32(corner & 1u) * 4 - 1);
    let y = f32(i32(corner >> 1u) * 4 - 1);
    return Across(vec4<f32>(x, y, 0.0, 1.0), slice);
}

// The texel a fragment is drawing: its column and row in the slice, and the slice.
fn texel(at: Across) -> vec3<i32> {
    return vec3<i32>(vec2<i32>(at.position.xy), i32(at.slice));
}
