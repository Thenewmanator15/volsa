// Draws one slice. The image values are in a single-channel floating-point texture; this
// shader finds which part of the image lies under each screen pixel, blends the four nearest
// values, and turns the result into grey through the window.

struct Params {
    // Image column = dot(to_col.xyz, (screen x, screen y, 1)); likewise the row.
    to_col: vec4<f32>,
    to_row: vec4<f32>,
    // Viewport width and height in screen pixels, then image columns and rows.
    sizes: vec4<f32>,
    // The value shown as black, then the span of values from black to white.
    window: vec4<f32>,
}

@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var frame: texture_2d<f32>;

struct Corner {
    @builtin(position) position: vec4<f32>,
    // 0 to 1 across the viewport, y downwards.
    @location(0) across: vec2<f32>,
}

// One triangle that covers the whole viewport.
@vertex
fn vertex(@builtin(vertex_index) index: u32) -> Corner {
    let across = vec2<f32>(f32((index << 1u) & 2u), f32(index & 2u));
    var corner: Corner;
    corner.position = vec4<f32>(across.x * 2.0 - 1.0, 1.0 - across.y * 2.0, 0.0, 1.0);
    corner.across = across;
    return corner;
}

fn value_at(col: i32, row: i32, last: vec2<i32>) -> f32 {
    let at = clamp(vec2<i32>(col, row), vec2<i32>(0, 0), last);
    return textureLoad(frame, at, 0).r;
}

@fragment
fn fragment(corner: Corner) -> @location(0) vec4<f32> {
    let screen = vec3<f32>(corner.across * params.sizes.xy, 1.0);
    // Voxel coordinates, counted from voxel centres.
    let voxel = vec2<f32>(dot(params.to_col.xyz, screen), dot(params.to_row.xyz, screen));
    let size = params.sizes.zw;
    let black = vec4<f32>(0.0, 0.0, 0.0, 1.0);
    if voxel.x < -0.5 || voxel.y < -0.5 || voxel.x > size.x - 0.5 || voxel.y > size.y - 0.5 {
        return black;
    }
    let corner_voxel = floor(voxel);
    let part = voxel - corner_voxel;
    let col = i32(corner_voxel.x);
    let row = i32(corner_voxel.y);
    let last = vec2<i32>(size) - vec2<i32>(1, 1);
    let upper = mix(value_at(col, row, last), value_at(col + 1, row, last), part.x);
    let lower = mix(value_at(col, row + 1, last), value_at(col + 1, row + 1, last), part.x);
    let value = mix(upper, lower, part.y);
    let grey = clamp((value - params.window.x) / params.window.y, 0.0, 1.0);
    return vec4<f32>(grey, grey, grey, 1.0);
}
