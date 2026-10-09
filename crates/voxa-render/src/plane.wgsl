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
}

@group(0) @binding(0) var<uniform> shown: View;
@group(0) @binding(1) var values: texture_3d<f32>;
@group(0) @binding(2) var blended: sampler;
// The coefficients of the cubic B-spline through the values, stored as the values are, and
// a sampler that reads beyond a face what lies as far inside it.
@group(0) @binding(3) var curve: texture_3d<f32>;
@group(0) @binding(4) var mirrored: sampler;

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
    var value = textureSampleLevel(values, blended, at, 0.0).r;
    if shown.grid.w > 0.5 {
        value = smoothed(at);
    }
    let outside = any(at < vec3<f32>(0.0)) || any(at > vec3<f32>(1.0));
    let grey = clamp((value - shown.view.z) / shown.view.w, 0.0, 1.0);
    return vec4<f32>(vec3<f32>(select(grey, 0.0, outside)), 1.0);
}
