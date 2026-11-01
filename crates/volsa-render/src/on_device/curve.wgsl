// The curve through a volume's values, worked out by the graphics device: the coefficients
// of a cubic B-spline, by the same filter the processor runs (bspline.rs), along the columns,
// then the rows, then the slices. Each invocation filters one whole line of voxels, in place.

struct Lines {
    // Which way the lines run: 0 along columns, 1 along rows, 2 along slices.
    axis: u32,
    // The first of the lines this dispatch does, counted along the second of the two other
    // ways; or, when the values are being copied in, the first slice it copies.
    first: u32,
    unused: vec2<u32>,
}

@group(0) @binding(0) var<uniform> lines: Lines;
@group(0) @binding(1) var work: texture_storage_3d<r32float, read_write>;
// The volume's values as the device holds them, from 0 to 1.
@group(0) @binding(2) var values: texture_3d<f32>;

// The pole of the cubic B-spline's filter, the square root of 3 less 2; what the values are
// multiplied by before it; and how many terms of the sum that starts it are kept.
const POLE: f32 = -0.2679492;
const GAIN: f32 = 6.0;
const TERMS: i32 = 16;

// Copy the volume's values in, to be filtered where they lie.
@compute @workgroup_size(8, 8, 1)
fn seed(@builtin(global_invocation_id) id: vec3<u32>) {
    let at = vec3<u32>(id.xy, id.z + lines.first);
    if any(at >= textureDimensions(work)) {
        return;
    }
    let value = textureLoad(values, vec3<i32>(at), 0).r;
    textureStore(work, vec3<i32>(at), vec4<f32>(value, 0.0, 0.0, 1.0));
}

// The voxel `along` a line, the line being one of those that run the way `lines.axis` says.
fn place(line: vec2<i32>, along: i32) -> vec3<i32> {
    switch lines.axis {
        case 0u: {
            return vec3<i32>(along, line.x, line.y);
        }
        case 1u: {
            return vec3<i32>(line.x, along, line.y);
        }
        default: {
            return vec3<i32>(line.x, line.y, along);
        }
    }
}

// Which voxel of a line of `count` a place before its first or after its last stands for,
// the line being mirrored about each of its ends: one before the first is the first.
fn mirrored(index: i32, count: i32) -> i32 {
    let turn = 2 * count;
    let within = ((index % turn) + turn) % turn;
    if within < count {
        return within;
    }
    return turn - 1 - within;
}

fn read(line: vec2<i32>, along: i32) -> f32 {
    return textureLoad(work, place(line, along)).r;
}

fn write(line: vec2<i32>, along: i32, value: f32) {
    textureStore(work, place(line, along), vec4<f32>(value, 0.0, 0.0, 1.0));
}

@compute @workgroup_size(8, 8, 1)
fn along(@builtin(global_invocation_id) id: vec3<u32>) {
    let size = vec3<i32>(textureDimensions(work));
    var count = size.z;
    var others = size.xy;
    if lines.axis == 0u {
        count = size.x;
        others = size.yz;
    } else if lines.axis == 1u {
        count = size.y;
        others = size.xz;
    }
    let line = vec2<i32>(i32(id.x), i32(id.y + lines.first));
    if any(line >= others) {
        return;
    }
    // Forwards. The first voxel is the sum of everything before it, mirrored, each voxel
    // further back counting for less by the pole.
    var before = 0.0;
    var weight = 1.0;
    for (var back = 0; back < TERMS; back++) {
        before += weight * GAIN * read(line, mirrored(-back, count));
        weight *= POLE;
    }
    write(line, 0, before);
    for (var at = 1; at < count; at++) {
        before = GAIN * read(line, at) + POLE * before;
        write(line, at, before);
    }
    // Backwards. The voxel beyond the last is the last, mirrored, which gives the last.
    var after = before * (POLE / (POLE - 1.0));
    write(line, count - 1, after);
    for (var at = count - 2; at >= 0; at--) {
        after = POLE * (after - read(line, at));
        write(line, at, after);
    }
}
