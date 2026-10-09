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
}

@group(0) @binding(0) var<uniform> shown: View;
@group(0) @binding(1) var values: texture_3d<f32>;
@group(0) @binding(2) var blended: sampler;

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
    let value = textureSampleLevel(values, blended, at, 0.0).r;
    let outside = any(at < vec3<f32>(0.0)) || any(at > vec3<f32>(1.0));
    let grey = clamp((value - shown.view.z) / shown.view.w, 0.0, 1.0);
    return vec4<f32>(vec3<f32>(select(grey, 0.0, outside)), 1.0);
}
