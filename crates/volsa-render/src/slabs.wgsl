// A volume longer than one texture may be is held in up to four, cut along its slices. Each
// texture but the last ends on the slice the next begins with, so a value blended between
// two slices always lies within one texture. Read with the shader's own `values` texture,
// the first, and its `blended` sampler.

@group(0) @binding(10) var values_1: texture_3d<f32>;
@group(0) @binding(11) var values_2: texture_3d<f32>;
@group(0) @binding(12) var values_3: texture_3d<f32>;

// The value at a place in the whole volume, each part from 0 at one face to 1 at the other.
// `slabs` is the volume's slices, the slices from the first of one texture to the first of
// the next, and how many textures; `depths` is the slices in each.
fn value_in_slabs(at: vec3<f32>, slabs: vec4<f32>, depths: vec4<f32>) -> f32 {
    if slabs.z < 1.5 {
        return textureSampleLevel(values, blended, at, 0.0).r;
    }
    // In slices, counted from the first slice's centre.
    let slice = at.z * slabs.x - 0.5;
    let slab = clamp(floor(max(slice, 0.0) / slabs.y), 0.0, slabs.z - 1.0);
    let which = i32(slab);
    let within = vec3<f32>(at.xy, (slice - slab * slabs.y + 0.5) / depths[which]);
    switch which {
        case 0: {
            return textureSampleLevel(values, blended, within, 0.0).r;
        }
        case 1: {
            return textureSampleLevel(values_1, blended, within, 0.0).r;
        }
        case 2: {
            return textureSampleLevel(values_2, blended, within, 0.0).r;
        }
        default: {
            return textureSampleLevel(values_3, blended, within, 0.0).r;
        }
    }
}
