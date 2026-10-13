//! The window's icon, drawn here so that no picture file is needed: a ring with a bead on
//! it, like the made-up study's ball seen in a slice.

/// The side of the icon, in pixels.
const SIDE: u32 = 64;
/// The ring's colour: the interface's accent.
const RING: [u8; 3] = [64, 156, 255];
/// The bead's colour.
const BEAD: [u8; 3] = [255, 255, 255];

/// How much of a pixel `distance` from the middle of a shape lies inside it, the shape
/// reaching `reach` from its middle: 1 well inside, 0 well outside, and in between across
/// one pixel at the edge.
fn inside(distance: f32, reach: f32) -> f32 {
    (reach - distance + 0.5).clamp(0.0, 1.0)
}

/// The icon: a ring, clear inside and out, with a white bead on it to the upper right.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "held between 0 and 255 first"
)]
pub fn icon() -> egui::IconData {
    let middle = (SIDE as f32 - 1.0) / 2.0;
    let mut rgba = Vec::with_capacity((SIDE * SIDE * 4) as usize);
    for y in 0..SIDE {
        for x in 0..SIDE {
            let (dx, dy) = (x as f32 - middle, y as f32 - middle);
            let from_middle = dx.hypot(dy);
            // The ring lies between 20 and 28 pixels out.
            let ring = inside(from_middle, 28.0) * (1.0 - inside(from_middle, 20.0));
            let bead = inside((dx - 17.0).hypot(dy + 17.0), 7.0);
            let colour = if bead > 0.0 { BEAD } else { RING };
            let cover = ring.max(bead);
            rgba.extend(colour);
            rgba.push((cover * 255.0).round().clamp(0.0, 255.0) as u8);
        }
    }
    egui::IconData {
        rgba,
        width: SIDE,
        height: SIDE,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cover(icon: &egui::IconData, x: u32, y: u32) -> u8 {
        icon.rgba[((y * icon.width + x) * 4 + 3) as usize]
    }

    #[test]
    fn the_icon_is_a_ring_with_a_bead_on_nothing() {
        let icon = icon();
        assert_eq!((icon.width, icon.height), (64, 64));
        assert_eq!(icon.rgba.len(), 64 * 64 * 4);
        for (x, y) in [(0, 0), (63, 0), (0, 63), (63, 63)] {
            assert_eq!(cover(&icon, x, y), 0, "clear at the corner {x}, {y}");
        }
        assert_eq!(cover(&icon, 32, 32), 0, "clear in the middle");
        assert_eq!(cover(&icon, 56, 32), 255, "the ring");
        assert_eq!(cover(&icon, 48, 15), 255, "the bead");
        let bead = ((15 * 64 + 48) * 4) as usize;
        assert_eq!(icon.rgba[bead..bead + 3], BEAD);
    }
}
