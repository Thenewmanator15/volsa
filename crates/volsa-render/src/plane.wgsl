// Draws one flat plane through a volume: for every pixel of the panel, the scan value at the
// point of the patient that pixel shows, through the window. The volume is the 3D texture
// the scene's own shader reads, with values from 0 at the smallest to 1 at the largest.

struct View {
    // The pixel (x, y) of the panel shows the point of the patient at
    // top_left + x * across + y * down, in millimetres.
    across: vec4<f32>,
    down: vec4<f32>,
    top_left: vec4<f32>,
    // How far a point of the patient is along the volume's columns, rows and slices, from 0
    // at one face to 1 at the other: dot(row.xyz, point) + row.w.
    to_u: vec4<f32>,
    to_v: vec4<f32>,
    to_w: vec4<f32>,
    // Panel width and height in pixels, the texture value shown as black, and the span of
    // texture values from black to white.
    view: vec4<f32>,
    // Columns, rows and slices of the volume, and 1 when the plane is drawn from the curve
    // through its values.
    grid: vec4<f32>,
    // Where a point of the patient is in the label volume, in voxels counted from voxel
    // centres, along its columns, rows and slices: dot(row.xyz, point) + row.w.
    label_u: vec4<f32>,
    label_v: vec4<f32>,
    label_w: vec4<f32>,
    // Columns, rows and slices of the label volume, and how labels are drawn: 0 not at
    // all, 1 outlined, 2 outlined and tinted inside.
    label_grid: vec4<f32>,
    // The number of the part selected (0 for none), and how far the inside is tinted.
    label_how: vec4<f32>,
    // How the volume is cut into textures, as `value_in_slabs` in slabs.wgsl reads it.
    slabs: vec4<f32>,
    depths: vec4<f32>,
}

@group(0) @binding(0) var<uniform> shown: View;
@group(0) @binding(1) var values: texture_3d<f32>;
@group(0) @binding(2) var blended: sampler;
// The coefficients of the cubic B-spline through the values, stored as the values are, and
// a sampler that reads beyond a face what lies as far inside it.
@group(0) @binding(3) var curve: texture_3d<f32>;
@group(0) @binding(4) var mirrored: sampler;
// The part at every voxel of a label volume, on its own grid (0 is nothing), and a table
// of each part's colour, with how solid it is as its alpha: 256 parts to a row.
@group(0) @binding(5) var parts: texture_3d<u32>;
@group(0) @binding(6) var looks: texture_2d<f32>;

struct Corner {
    @builtin(position) position: vec4<f32>,
    // 0 to 1 across the panel, y downwards.
    @location(0) across: vec2<f32>,
}

// One triangle that covers the whole panel.
@vertex
fn vertex(@builtin(vertex_index) index: u32) -> Corner {
    let across = vec2<f32>(f32((index << 1u) & 2u), f32(index & 2u));
    var corner: Corner;
    corner.position = vec4<f32>(across.x * 2.0 - 1.0, 1.0 - across.y * 2.0, 0.0, 1.0);
    corner.across = across;
    return corner;
}

// The value of the curve at a place in the volume, each part from 0 at one face to 1 at the
// other. A cubic B-spline draws on the 64 coefficients round a place; blended reads fetch
// them eight at a time, so eight reads are enough (Sigg and Hadwiger, GPU Gems 2, 2005).
fn smoothed(at: vec3<f32>) -> f32 {
    let size = shown.grid.xyz;
    // In voxels, counted from voxel centres.
    let voxel = at * size - 0.5;
    let whole = floor(voxel);
    let part = voxel - whole;
    let rest = 1.0 - part;
    // How much the voxels one before, at, one after and two after the place count for.
    let before = rest * rest * rest / 6.0;
    let here = 2.0 / 3.0 - part * part + part * part * part / 2.0;
    let after = 2.0 / 3.0 - rest * rest + rest * rest * rest / 2.0;
    let beyond = part * part * part / 6.0;
    // Each pair is one blended read, made where the blend gives the pair its shares.
    let lower = before + here;
    let upper = after + beyond;
    let low = (whole - 0.5 + here / lower) / size;
    let high = (whole + 1.5 + beyond / upper) / size;
    var sum = 0.0;
    for (var corner = 0u; corner < 8u; corner++) {
        let far = vec3<bool>((corner & 1u) != 0u, (corner & 2u) != 0u, (corner & 4u) != 0u);
        let place = select(low, high, far);
        let share = select(lower, upper, far);
        sum += share.x * share.y * share.z * textureSampleLevel(curve, mirrored, place, 0.0).r;
    }
    return sum;
}

// The part at a point of the patient: that of the nearest label voxel, or 0 where the
// labels do not reach.
fn part_at(patient: vec3<f32>) -> u32 {
    let voxel = vec3<f32>(
        dot(shown.label_u.xyz, patient) + shown.label_u.w,
        dot(shown.label_v.xyz, patient) + shown.label_v.w,
        dot(shown.label_w.xyz, patient) + shown.label_w.w,
    );
    let nearest = vec3<i32>(floor(voxel + 0.5));
    let size = vec3<i32>(shown.label_grid.xyz);
    if any(nearest < vec3<i32>(0)) || any(nearest >= size) {
        return 0u;
    }
    return textureLoad(parts, nearest, 0).r;
}

// A part's colour, with how solid it is as alpha. Part 0, nothing, is clear.
fn look_of(part: u32) -> vec4<f32> {
    return textureLoad(looks, vec2<i32>(i32(part % 256u), i32(part / 256u)), 0);
}

// The line between a pixel's part and a neighbouring pixel's, drawn over `under`: in the
// pixel's own part's colour, or the neighbour's where its own is not shown, and in white
// where either is the part selected. Nothing where the two are one part.
fn edged(under: vec3<f32>, here: u32, there: u32) -> vec3<f32> {
    if here == there {
        return under;
    }
    let mine = look_of(here);
    let theirs = look_of(there);
    let chosen = u32(shown.label_how.x);
    let picked = chosen != 0u
        && ((here == chosen && mine.a > 0.0) || (there == chosen && theirs.a > 0.0));
    let line = select(theirs, mine, mine.a > 0.0);
    return mix(under, select(line.rgb, vec3<f32>(1.0), picked), line.a);
}

// The labels over a pixel that shows `patient` and is `under` without them.
fn labelled(under: vec3<f32>, patient: vec3<f32>) -> vec3<f32> {
    let here = part_at(patient);
    var colour = under;
    if shown.label_grid.w > 1.5 {
        let mine = look_of(here);
        colour = mix(colour, mine.rgb, shown.label_how.y * mine.a);
    }
    // One pixel to the right, and one beneath.
    colour = edged(colour, here, part_at(patient + shown.across.xyz));
    return edged(colour, here, part_at(patient + shown.down.xyz));
}

@fragment
fn fragment(corner: Corner) -> @location(0) vec4<f32> {
    let pixel = corner.across * shown.view.xy;
    let patient = shown.top_left.xyz + pixel.x * shown.across.xyz + pixel.y * shown.down.xyz;
    let at = vec3<f32>(
        dot(shown.to_u.xyz, patient) + shown.to_u.w,
        dot(shown.to_v.xyz, patient) + shown.to_v.w,
        dot(shown.to_w.xyz, patient) + shown.to_w.w,
    );
    // The value is read everywhere, as a texture must be, and thrown away outside the
    // volume: there the picture is black, not the nearest voxel carried on.
    var value = value_in_slabs(at, shown.slabs, shown.depths);
    if shown.grid.w > 0.5 {
        value = smoothed(at);
    }
    let outside = any(at < vec3<f32>(0.0)) || any(at > vec3<f32>(1.0));
    let grey = clamp((value - shown.view.z) / shown.view.w, 0.0, 1.0);
    var colour = vec3<f32>(grey);
    if shown.label_grid.w > 0.5 {
        colour = labelled(colour, patient);
    }
    return vec4<f32>(select(colour, vec3<f32>(0.0), outside), 1.0);
}
