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
    // From a pixel's column, row and 1 to a place among the label voxels, counted from voxel
    // centres: the slice's own place is in the last number.
    label_u: vec4<f32>,
    label_v: vec4<f32>,
    label_w: vec4<f32>,
    // Columns, rows and slices of the label volume, and how labels are drawn: 0 not at
    // all, 1 outlined, 2 outlined and tinted inside.
    label_grid: vec4<f32>,
    // The number of the part selected (0 for none), and how far the inside is tinted.
    label_how: vec4<f32>,
}

@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var frame: texture_2d<f32>;
// The part at every voxel of the labels open, on their own grid (0 is nothing), and a table of
// each part's colour with how solid it is as alpha, 256 parts to a row: the very textures the
// planes draw from.
@group(1) @binding(0) var parts: texture_3d<u32>;
@group(1) @binding(1) var looks: texture_2d<f32>;

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

// The part at a place of the slice, in pixels: that of the nearest label voxel, or 0 where
// the labels do not reach.
fn part_at(pixel: vec2<f32>) -> u32 {
    let here = vec3<f32>(pixel, 1.0);
    let voxel = vec3<f32>(
        dot(params.label_u.xyz, here),
        dot(params.label_v.xyz, here),
        dot(params.label_w.xyz, here),
    );
    let nearest = vec3<i32>(floor(voxel + 0.5));
    let size = vec3<i32>(params.label_grid.xyz);
    if any(nearest < vec3<i32>(0)) || any(nearest >= size) {
        return 0u;
    }
    return textureLoad(parts, nearest, 0).r;
}

// A part's colour, with how solid it is as alpha. Part 0, nothing, is clear.
fn look_of(part: u32) -> vec4<f32> {
    return textureLoad(looks, vec2<i32>(i32(part % 256u), i32(part / 256u)), 0);
}

// The line between a pixel's part and a neighbouring pixel's, drawn over `under`, as the
// planes draw it: in the pixel's own part's colour, or the neighbour's where its own is not
// shown, and in white where either is the part selected.
fn edged(under: vec3<f32>, here: u32, there: u32) -> vec3<f32> {
    if here == there {
        return under;
    }
    let mine = look_of(here);
    let theirs = look_of(there);
    let chosen = u32(params.label_how.x);
    let picked = chosen != 0u
        && ((here == chosen && mine.a > 0.0) || (there == chosen && theirs.a > 0.0));
    let line = select(theirs, mine, mine.a > 0.0);
    return mix(under, select(line.rgb, vec3<f32>(1.0), picked), line.a);
}

// The labels over the screen pixel that shows `pixel` of the slice and is `under` without
// them. `across` and `down` are one screen pixel in pixels of the slice, so that lines are a
// screen pixel wide however far the slice is zoomed.
fn labelled(under: vec3<f32>, pixel: vec2<f32>, across: vec2<f32>, down: vec2<f32>) -> vec3<f32> {
    let here = part_at(pixel);
    var colour = under;
    if params.label_grid.w > 1.5 {
        let mine = look_of(here);
        colour = mix(colour, mine.rgb, params.label_how.y * mine.a);
    }
    colour = edged(colour, here, part_at(pixel + across));
    return edged(colour, here, part_at(pixel + down));
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
    var colour = vec3<f32>(grey);
    if params.label_grid.w > 0.5 {
        let across = vec2<f32>(params.to_col.x, params.to_row.x);
        let down = vec2<f32>(params.to_col.y, params.to_row.y);
        colour = labelled(colour, voxel, across, down);
    }
    return vec4<f32>(colour, 1.0);
}
