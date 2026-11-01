//! The window's icon, drawn here so that no picture file is needed: a ring with a bead on
//! it, like the made-up study's ball seen in a slice. The drawing itself is in `drawing`,
//! which the build script also draws `volsa.exe`'s icon from.

mod drawing;

use drawing::LAID_OUT;

/// The icon: a ring, clear inside and out, with a white bead on it to the upper right.
pub fn icon() -> egui::IconData {
    egui::IconData {
        rgba: drawing::rgba(LAID_OUT),
        width: LAID_OUT,
        height: LAID_OUT,
    }
}

#[cfg(test)]
mod tests {
    use super::drawing::{BEAD, rgba};
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

    #[test]
    fn the_icon_is_the_same_drawing_at_every_size() {
        for side in [16, 32, 48, 256] {
            let icon = egui::IconData {
                rgba: rgba(side),
                width: side,
                height: side,
            };
            assert_eq!(icon.rgba.len(), (side * side * 4) as usize);
            let at = |x: u32, y: u32| cover(&icon, x * side / 64, y * side / 64);
            for (x, y) in [(0, 0), (63, 0), (0, 63), (63, 63)] {
                assert_eq!(at(x, y), 0, "{side}: clear at the corner {x}, {y}");
            }
            assert_eq!(at(32, 32), 0, "{side}: clear in the middle");
            assert!(at(56, 32) > 200, "{side}: the ring");
            assert!(at(48, 15) > 200, "{side}: the bead");
        }
    }
}
