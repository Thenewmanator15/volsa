// Draws a volume by following a line of sight through it for every pixel of the picture.
// The volume's values are in a 3D texture, from 0 at the smallest to 1 at the largest, and the
// graphics device blends the eight values around each point that is read.

struct Scene {
    // The screen pixel (x, y), z millimetres beyond the camera's target, lies in the patient
    // at top_left + x * across + y * along + z * forward, in millimetres.
    across: vec4<f32>,
    along: vec4<f32>,
    forward: vec4<f32>,
    top_left: vec4<f32>,
    // How far a point of the patient is along the volume's columns, rows and slices, from 0
    // at one face to 1 at the other: dot(row.xyz, point) + row.w.
    to_u: vec4<f32>,
    to_v: vec4<f32>,
    to_w: vec4<f32>,
    // Picture width and height in pixels, the step along a line of sight in millimetres, and
    // the look: 0 see-through, 1 brightest value, 2 a lit surface.
    view: vec4<f32>,
    // The texture value shown as black; the span of texture values from black to white; how
    // much of the light a millimetre of solid tissue stops; and the share of the window, up
    // from black, over which tissue goes from stopping no light to being solid.
    window: vec4<f32>,
    // One voxel along the volume's columns, rows and slices, as a step through the texture.
    voxel: vec4<f32>,
}

@group(0) @binding(0) var<uniform> scene: Scene;
@group(0) @binding(1) var values: texture_3d<f32>;
@group(0) @binding(2) var blended: sampler;

// The most steps along one line of sight; a longer line is followed in longer steps.
const MAX_STEPS: f32 = 4096.0;
// How much of the light must be stopped before nothing behind can be seen.
const SOLID: f32 = 0.98;
// The least light a surface is shown in, where it has turned right away from the viewer.
const SHADOW: f32 = 0.35;
// The least slope, as a share of the window for each millimetre, that counts as a surface
// with a direction to it.
const LEAST_SLOPE: f32 = 0.01;
const FAR: f32 = 1.0e30;

struct Corner {
    @builtin(position) position: vec4<f32>,
    // 0 to 1 across the picture, y downwards.
    @location(0) across: vec2<f32>,
}

// One triangle that covers the whole picture.
@vertex
fn vertex(@builtin(vertex_index) index: u32) -> Corner {
    let across = vec2<f32>(f32((index << 1u) & 2u), f32(index & 2u));
    var corner: Corner;
    corner.position = vec4<f32>(across.x * 2.0 - 1.0, 1.0 - across.y * 2.0, 0.0, 1.0);
    corner.across = across;
    return corner;
}

// A direction in the patient as a step through the texture.
fn turned_into_texture(patient: vec3<f32>) -> vec3<f32> {
    return vec3<f32>(
        dot(scene.to_u.xyz, patient),
        dot(scene.to_v.xyz, patient),
        dot(scene.to_w.xyz, patient),
    );
}

// How far along a line, which starts at `start` and moves `step` for each millimetre, it
// lies between the two faces of the volume across one axis: at 0 and at 1.
fn between_faces(start: f32, step: f32) -> vec2<f32> {
    if abs(step) < 1.0e-9 {
        // The line runs along the faces: it is between them all the way or not at all.
        if start < 0.0 || start > 1.0 {
            return vec2<f32>(FAR, -FAR);
        }
        return vec2<f32>(-FAR, FAR);
    }
    let one = (0.0 - start) / step;
    let other = (1.0 - start) / step;
    return vec2<f32>(min(one, other), max(one, other));
}

// A texture value as the grey the window makes of it, 0 to 1.
fn windowed(value: f32) -> f32 {
    return clamp((value - scene.window.x) / scene.window.y, 0.0, 1.0);
}

// How solid a texture value is, from 0, which stops no light, to 1.
fn solidity(value: f32) -> f32 {
    return clamp((value - scene.window.x) / (scene.window.y * scene.window.w), 0.0, 1.0);
}

fn value_at(at: vec3<f32>) -> f32 {
    return textureSampleLevel(values, blended, at, 0.0).r;
}

// Which way, in the patient, the values rise fastest at a point of the texture, and how
// fast: texture value for each millimetre.
fn slope_at(at: vec3<f32>) -> vec3<f32> {
    let u = vec3<f32>(scene.voxel.x, 0.0, 0.0);
    let v = vec3<f32>(0.0, scene.voxel.y, 0.0);
    let w = vec3<f32>(0.0, 0.0, scene.voxel.z);
    // The rise across two voxels along each axis of the volume, then each axis as it lies
    // in the patient.
    return (value_at(at + u) - value_at(at - u)) / (2.0 * scene.voxel.x) * scene.to_u.xyz
        + (value_at(at + v) - value_at(at - v)) / (2.0 * scene.voxel.y) * scene.to_v.xyz
        + (value_at(at + w) - value_at(at - w)) / (2.0 * scene.voxel.z) * scene.to_w.xyz;
}

// A fixed pattern over the pixels, 0 to 1, with no two neighbours alike. Each line of sight
// starts this share of a step in, so that the steps of neighbouring lines do not fall
// together and show as rings.
fn scattered(pixel: vec2<f32>) -> f32 {
    return fract(52.9829189 * fract(dot(floor(pixel), vec2<f32>(0.06711056, 0.00583715))));
}

@fragment
fn fragment(corner: Corner) -> @location(0) vec4<f32> {
    let black = vec4<f32>(0.0, 0.0, 0.0, 1.0);
    // The line of sight through this pixel, in millimetres from where it crosses the plane
    // of the camera's target, and the same line as a walk through the texture.
    let pixel = corner.across * scene.view.xy;
    let on_plane = scene.top_left.xyz + pixel.x * scene.across.xyz + pixel.y * scene.along.xyz;
    let start = turned_into_texture(on_plane) + vec3<f32>(scene.to_u.w, scene.to_v.w, scene.to_w.w);
    let step = turned_into_texture(scene.forward.xyz);

    // In the texture the volume is a box from 0 to 1 each way, however its slices are tilted.
    let columns = between_faces(start.x, step.x);
    let rows = between_faces(start.y, step.y);
    let slices = between_faces(start.z, step.z);
    let enter = max(max(columns.x, rows.x), slices.x);
    let leave = min(min(columns.y, rows.y), slices.y);
    if leave <= enter {
        return black;
    }

    let stride = max(scene.view.z, (leave - enter) / MAX_STEPS);
    let steps = i32(ceil((leave - enter) / stride));
    let brightest_value = scene.view.w > 0.5 && scene.view.w < 1.5;
    let lit = scene.view.w > 1.5;
    let first = scattered(pixel);
    var brightest = 0.0;
    var light = 0.0;
    var stopped = 0.0;
    // How much light the surface last met is in: all of it facing the viewer, down to SHADOW
    // turned away. It carries on into the even tissue behind the surface.
    var shade = 1.0;
    for (var i = 0; i < steps; i++) {
        let at = start + min(enter + (f32(i) + first) * stride, leave) * step;
        let value = value_at(at);
        if brightest_value {
            brightest = max(brightest, value);
            continue;
        }
        let solid = solidity(value);
        if solid <= 0.0 {
            continue;
        }
        if lit {
            let slope = slope_at(at);
            let steep = length(slope);
            if steep > scene.window.y * LEAST_SLOPE {
                let facing = max(dot(slope / steep, scene.forward.xyz), 0.0);
                shade = SHADOW + (1.0 - SHADOW) * facing;
            }
        }
        // More solid tissue stops more of the light behind it. The share stopped follows the
        // length of the step, so the picture does not change with the step.
        let stops = 1.0 - exp(-scene.window.z * solid * stride);
        light += (1.0 - stopped) * stops * windowed(value) * shade;
        stopped += (1.0 - stopped) * stops;
        if stopped > SOLID {
            break;
        }
    }
    if brightest_value {
        light = windowed(brightest);
    }
    return vec4<f32>(light, light, light, 1.0);
}
