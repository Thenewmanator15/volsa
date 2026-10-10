// The curve's coefficients copied, a slice at a time, from where they were worked out into
// the texture planes are drawn from, which holds them as the volume's values are held.

@group(0) @binding(0) var coefficients: texture_3d<f32>;

@fragment
fn stored(at: Across) -> @location(0) vec4<f32> {
    return vec4<f32>(textureLoad(coefficients, texel(at), 0).r, 0.0, 0.0, 1.0);
}
