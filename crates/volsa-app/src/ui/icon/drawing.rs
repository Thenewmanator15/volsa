//! The drawing of Volsa's icon, at any size: a ring with a bead on it, like the made-up study's
//! ball seen in a slice. The window's icon is drawn from it, and so is the icon Explorer shows
//! for `volsa.exe`, which the build script draws from this same file.

/// The side the drawing is laid out on, in pixels. At another side every length is scaled.
pub const LAID_OUT: u32 = 64;
/// The ring's colour: the interface's accent.
pub const RING: [u8; 3] = [64, 156, 255];
/// The bead's colour.
pub const BEAD: [u8; 3] = [255, 255, 255];

/// How much of a pixel `distance` from the middle of a shape lies inside it, the shape
/// reaching `reach` from its middle: 1 well inside, 0 well outside, and in between across
/// one pixel at the edge.
fn inside(distance: f32, reach: f32) -> f32 {
    (reach - distance + 0.5).clamp(0.0, 1.0)
}

/// The icon, `side` pixels square, as red, green, blue and coverage for each pixel, row by
/// row from the top: a ring, clear inside and out, with a white bead on it to the upper right.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "held between 0 and 255 first"
)]
pub fn rgba(side: u32) -> Vec<u8> {
    let scale = side as f32 / LAID_OUT as f32;
    let middle = (side as f32 - 1.0) / 2.0;
    let pixels = usize::try_from(side).unwrap_or(0).pow(2);
    let mut rgba = Vec::with_capacity(pixels * 4);
    for y in 0..side {
        for x in 0..side {
            let (dx, dy) = (x as f32 - middle, y as f32 - middle);
            let from_middle = dx.hypot(dy);
            // The ring lies between 20 and 28 pixels out, of 64.
            let ring =
                inside(from_middle, 28.0 * scale) * (1.0 - inside(from_middle, 20.0 * scale));
            let bead = inside((dx - 17.0 * scale).hypot(dy + 17.0 * scale), 7.0 * scale);
            let colour = if bead > 0.0 { BEAD } else { RING };
            let cover = ring.max(bead);
            rgba.extend(colour);
            rgba.push((cover * 255.0).round().clamp(0.0, 255.0) as u8);
        }
    }
    rgba
}
