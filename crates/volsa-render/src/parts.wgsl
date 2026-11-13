// Draws labelled parts as lit solids by following a line of sight through a label volume for
// every pixel of the picture. The volume holds a part number at every voxel, on its own grid
// (0 is nothing); a small table holds each part's colour, with how solid it is as its alpha.
// One part may be drawn through the rest, as if they were glass: the line of sight goes on
// past the other solids to find it, and it is laid over them, fainter where it is behind.

struct Scene {
    // The screen pixel (x, y), z millimetres beyond the camera's target, lies in the patient
    // at top_left + x * across + y * along + z * forward, in millimetres. The fourth number
    // of top_left is 1 when blocks that hold no part are to be stepped over. The fourth
    // number of forward is the part drawn through the rest, 0 for none.
    across: vec4<f32>,
    along: vec4<f32>,
    forward: vec4<f32>,
    top_left: vec4<f32>,
    // Where a point of the patient is in the label volume, in voxels counted from voxel
    // centres, along its columns, rows and slices: dot(row.xyz, point) + row.w.
    to_col: vec4<f32>,
    to_row: vec4<f32>,
    to_slice: vec4<f32>,
    // Picture width and height in pixels, the step along a line of sight in millimetres, and
    // the number of the part selected (0 for none).
    view: vec4<f32>,
    // Columns, rows and slices of the label volume, and how many of the cuts are in use.
    grid: vec4<f32>,
    // Planes in the patient, each as a direction of length 1 and how far along it the plane
    // lies from the patient origin. A cut removes everything on the side it points to.
    cuts: array<vec4<f32>, 3>,
}

@group(0) @binding(0) var<uniform> scene: Scene;
@group(0) @binding(1) var parts: texture_3d<u32>;
@group(0) @binding(2) var looks: texture_2d<f32>;
// One texel for each block of BLOCK voxels a side: 1 where the block holds a part, 0 where
// it holds none. Read only when the scene says so (top_left.w).
@group(0) @binding(3) var filled: texture_3d<f32>;

// The side of a block, in voxels. The same as PART_BLOCK in on_device/filled.rs.
const BLOCK: i32 = 8;

// The most steps along one line of sight; a longer line is followed in longer steps.
const MAX_STEPS: f32 = 2048.0;
// How much of the light must be stopped before nothing behind can be seen.
const SOLID: f32 = 0.98;
// The least light a surface is shown in, where it has turned right away from the viewer.
const SHADOW: f32 = 0.35;
// How far the selected part's surface is paled where it turns right away from the viewer.
const RIM: f32 = 0.75;
// How far the selected part is paled on a flat face: a cut's, or where another part ends.
const ON_A_FACE: f32 = 0.3;
// How strong the part drawn through the rest is where other solids are wholly in front of
// it, as a share of its strength where nothing is.
const BEHIND: f32 = 0.55;
const FAR: f32 = 1.0e30;
// The least a line of sight must lean towards a plane to be taken as meeting it.
const ALONGSIDE: f32 = 1.0e-6;

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

// A direction in the patient as a step through the label volume, in voxels.
fn in_voxels(patient: vec3<f32>) -> vec3<f32> {
    return vec3<f32>(
        dot(scene.to_col.xyz, patient),
        dot(scene.to_row.xyz, patient),
        dot(scene.to_slice.xyz, patient),
    );
}

// How far along a line, which starts at `start` and moves `step` for each millimetre, it
// lies between the two faces of the volume across one axis `count` voxels long: half a voxel
// before the first voxel's centre and half a voxel after the last's.
fn between_faces(start: f32, step: f32, count: f32) -> vec2<f32> {
    let low = -0.5;
    let high = count - 0.5;
    if abs(step) < 1.0e-9 {
        if start < low || start > high {
            return vec2<f32>(FAR, -FAR);
        }
        return vec2<f32>(-FAR, FAR);
    }
    let one = (low - start) / step;
    let other = (high - start) / step;
    return vec2<f32>(min(one, other), max(one, other));
}

// The part of one voxel, or 0 outside the volume.
fn part_of(voxel: vec3<i32>) -> u32 {
    let size = vec3<i32>(scene.grid.xyz);
    if any(voxel < vec3<i32>(0)) || any(voxel >= size) {
        return 0u;
    }
    return textureLoad(parts, voxel, 0).r;
}

// A part's colour, with how solid it is as alpha. Part 0, nothing, is clear.
fn look_of(part: u32) -> vec4<f32> {
    return textureLoad(looks, vec2<i32>(i32(part % 256u), i32(part / 256u)), 0);
}

// 1 where a voxel holds a part that is shown and is among those `only` picks, 0 where not:
// with `only` 0, any part but the one drawn through the rest; otherwise that part alone.
fn shown(voxel: vec3<i32>, only: u32) -> f32 {
    let part = part_of(voxel);
    let picked = select(part != u32(scene.forward.w), part == only, only != 0u);
    return select(0.0, 1.0, picked && look_of(part).a > 0.0);
}

// How much of what is round a place, given in voxels, is shown, of the parts `only` picks:
// the eight voxels about it, blended by how near each is. A half is where the surface lies.
fn share_shown(at: vec3<f32>, only: u32) -> f32 {
    let base = floor(at);
    let part = at - base;
    let c = vec3<i32>(base);
    let near = mix(
        mix(shown(c, only), shown(c + vec3<i32>(1, 0, 0), only), part.x),
        mix(shown(c + vec3<i32>(0, 1, 0), only), shown(c + vec3<i32>(1, 1, 0), only), part.x),
        part.y,
    );
    let far = mix(
        mix(shown(c + vec3<i32>(0, 0, 1), only), shown(c + vec3<i32>(1, 0, 1), only), part.x),
        mix(shown(c + vec3<i32>(0, 1, 1), only), shown(c + vec3<i32>(1, 1, 1), only), part.x),
        part.y,
    );
    return mix(near, far, part.z);
}

// Which way, in the patient, the solids thicken fastest about a place given in voxels. It is
// measured across four voxels each way, and again across two: label voxels make a surface of
// small steps, and a slope taken over one step would light every step as a wall.
fn into_the_solid(at: vec3<f32>, only: u32) -> vec3<f32> {
    var slope = vec3<f32>(0.0);
    for (var reach = 1.0; reach < 2.5; reach += 1.0) {
        let col = vec3<f32>(reach, 0.0, 0.0);
        let row = vec3<f32>(0.0, reach, 0.0);
        let slice = vec3<f32>(0.0, 0.0, reach);
        let across_col = share_shown(at + col, only) - share_shown(at - col, only);
        let across_row = share_shown(at + row, only) - share_shown(at - row, only);
        let across_slice = share_shown(at + slice, only) - share_shown(at - slice, only);
        slope += (across_col * scene.to_col.xyz
            + across_row * scene.to_row.xyz
            + across_slice * scene.to_slice.xyz) / reach;
    }
    return slope;
}

@fragment
fn fragment(corner: Corner) -> @location(0) vec4<f32> {
    let black = vec4<f32>(0.0, 0.0, 0.0, 1.0);
    // The line of sight through this pixel, in millimetres from where it crosses the plane
    // of the camera's target, and the same line as a walk through the label volume.
    let pixel = corner.across * scene.view.xy;
    let on_plane = scene.top_left.xyz + pixel.x * scene.across.xyz + pixel.y * scene.along.xyz;
    let start = in_voxels(on_plane) + vec3<f32>(scene.to_col.w, scene.to_row.w, scene.to_slice.w);
    let step = in_voxels(scene.forward.xyz);

    let columns = between_faces(start.x, step.x, scene.grid.x);
    let rows = between_faces(start.y, step.y, scene.grid.y);
    let slices = between_faces(start.z, step.z, scene.grid.z);
    var enter = max(max(columns.x, rows.x), slices.x);
    var leave = min(min(columns.y, rows.y), slices.y);

    // The line starts where it comes out of the removed side of the last cut it crosses.
    var on_a_cut = false;
    for (var c = 0; c < i32(scene.grid.w); c++) {
        let cut = scene.cuts[c];
        let removed_by = dot(cut.xyz, on_plane) - cut.w;
        let leaning = dot(cut.xyz, scene.forward.xyz);
        if abs(leaning) < ALONGSIDE {
            if removed_by > 0.0 {
                return black;
            }
            continue;
        }
        let meets = -removed_by / leaning;
        if leaning > 0.0 {
            leave = min(leave, meets);
        } else if meets > enter {
            enter = meets;
            on_a_cut = true;
        }
    }
    if leave <= enter {
        return black;
    }

    let stride = max(scene.view.z, (leave - enter) / MAX_STEPS);
    let steps = i32(ceil((leave - enter) / stride));
    let chosen = u32(scene.view.w);
    let through = u32(scene.forward.w);
    let stepping_over = scene.top_left.w > 0.5;
    var light = vec3<f32>(0.0);
    var stopped = 0.0;
    // The part drawn through the rest, where the line first meets it, and how strongly.
    var seen_through = vec3<f32>(0.0);
    var through_strength = 0.0;
    var found = through == 0u;
    // The part the line was last in, so that a part is drawn once where the line enters it.
    // The part drawn through the rest is passed over here: a line that goes through it from
    // one part back into the same is taken as never having left.
    var within = 0u;
    for (var i = 0; i < steps; i++) {
        let along = min(enter + (f32(i) + 0.5) * stride, leave);
        let at = start + along * step;
        let voxel = vec3<i32>(floor(at + 0.5));
        let block = voxel / BLOCK;
        // Whether the block is empty is only noted inside the test of `stepping_over`; the
        // step over it is taken out here, at the loop's own level. With the step taken inside
        // that test, Windows' software device (WARP) drew nothing at all where blocks were not
        // stepped over, though other devices drew the same shader right.
        var empty = false;
        if stepping_over {
            empty = textureLoad(filled, block, 0).r == 0.0;
        }
        if empty {
            // Nothing in this block: go on from the first step that lies beyond it. Every
            // step in between would have found nothing, and been passed.
            within = 0u;
            let near = vec3<f32>(block * BLOCK) - 0.5;
            let far = near + f32(BLOCK);
            let out = (select(near, far, step > vec3<f32>(0.0)) - at) / step;
            let ways = select(vec3<f32>(FAR), out, abs(step) > vec3<f32>(1.0e-12));
            let beyond = along + max(min(min(ways.x, ways.y), ways.z), 0.0);
            i = max(i, i32(floor((beyond - enter) / stride - 0.5)));
            continue;
        }
        let part = part_of(voxel);
        let look = look_of(part);
        if look.a <= 0.0 {
            within = 0u;
            continue;
        }
        let first_on_a_cut = on_a_cut && i == 0;
        let outside = max(along - stride, enter);
        if part == through {
            if !found {
                // Lit by its own surface alone, and fainter the more is in front of it.
                found = true;
                var shade = 1.0;
                if !first_on_a_cut {
                    shade = lit(start, step, outside, along, through);
                }
                var colour = look.rgb;
                if part == chosen {
                    colour = mix(colour, vec3<f32>(1.0), ON_A_FACE);
                }
                seen_through = colour * shade;
                through_strength = look.a * mix(1.0, BEHIND, stopped);
            }
            continue;
        }
        if part == within {
            continue;
        }
        var shade = 1.0;
        var colour = look.rgb;
        if !first_on_a_cut && within == 0u {
            // Coming in from outside the solids: lit by which way the surface faces. A cut's
            // face, and a step from one part straight into the next, are flat and unlit.
            shade = lit(start, step, outside, along, 0u);
            if part == chosen {
                let turned = 1.0 - (shade - SHADOW) / (1.0 - SHADOW);
                colour = mix(colour, vec3<f32>(1.0), RIM * turned * turned);
            }
        } else if part == chosen {
            // A flat face has no rim to pale: the whole of it is paled a little.
            colour = mix(colour, vec3<f32>(1.0), ON_A_FACE);
        }
        within = part;
        light += (1.0 - stopped) * look.a * colour * shade;
        stopped += (1.0 - stopped) * look.a;
        if stopped > SOLID && found {
            break;
        }
    }
    return vec4<f32>(mix(light, seen_through, through_strength), 1.0);
}

// How the surface the line comes in through, between `outside` and `inside` along it, is lit,
// among the parts `only` picks (as `shown` takes it): the surface is where half of what is
// round the line is shown, and it is lit by which way it faces. 1, fully lit, where no way
// can be told.
fn lit(start: vec3<f32>, step: vec3<f32>, outside: f32, inside: f32, only: u32) -> f32 {
    var out = outside;
    var into = inside;
    for (var halving = 0; halving < 4; halving++) {
        let between = 0.5 * (out + into);
        if share_shown(start + between * step, only) >= 0.5 {
            into = between;
        } else {
            out = between;
        }
    }
    let slope = into_the_solid(start + into * step, only);
    let steep = length(slope);
    if steep <= 1.0e-6 {
        return 1.0;
    }
    let facing = max(dot(slope / steep, scene.forward.xyz), 0.0);
    return SHADOW + (1.0 - SHADOW) * facing;
}
