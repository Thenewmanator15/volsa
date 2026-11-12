//! How the pictures in the middle of the window are arranged: the slices alone, the slices
//! beside the 3D view, the three planes with the 3D view, or the 3D view alone.

use crate::scene::SceneStatus;

/// How the pictures in the middle of the window are arranged. Volsa opens on the slices: a
/// newcomer measures on a slice, and the first slice shows at once even on a large scan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Pictures {
    /// The 2D viewports alone: one, two or four of them.
    #[default]
    Slices,
    /// The 2D viewports at the left and the 3D view at the right.
    SlicesAndScene,
    /// The axial, coronal and sagittal planes through one point and the 3D view, two by two.
    Planes,
    /// The 3D view alone.
    Scene,
}

impl Pictures {
    /// Every arrangement, in the order the strip above the pictures offers them.
    pub const ALL: [Self; 4] = [
        Self::Slices,
        Self::SlicesAndScene,
        Self::Planes,
        Self::Scene,
    ];

    /// What is shown: what was asked for, except that a scan with no 3D view has its slices
    /// alone, as there is nothing else of it to see.
    #[must_use]
    pub fn given(self, status: &SceneStatus) -> Self {
        if matches!(status, SceneStatus::Unavailable(_)) {
            Self::Slices
        } else {
            self
        }
    }

    /// Whether the 2D viewports are among the pictures.
    pub const fn has_slices(self) -> bool {
        matches!(self, Self::Slices | Self::SlicesAndScene)
    }

    /// Whether the 3D view is among the pictures.
    pub const fn has_scene(self) -> bool {
        !matches!(self, Self::Slices)
    }

    /// Whether the three planes are among the pictures.
    pub const fn has_planes(self) -> bool {
        matches!(self, Self::Planes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn without_a_3d_view_every_arrangement_shows_slices() {
        let no_view = SceneStatus::Unavailable(vec!["why".to_owned()]);
        for asked in Pictures::ALL {
            assert_eq!(asked.given(&no_view), Pictures::Slices, "{asked:?}");
        }
        let building = SceneStatus::Building { done: 1, of: 2 };
        for status in [SceneStatus::Empty, building, SceneStatus::Ready] {
            for asked in Pictures::ALL {
                assert_eq!(
                    asked.given(&status),
                    asked,
                    "{status:?}: what was asked stands"
                );
            }
        }
    }

    #[test]
    fn each_arrangement_says_what_it_shows() {
        let shows = |pictures: Pictures| {
            (
                pictures.has_slices(),
                pictures.has_scene(),
                pictures.has_planes(),
            )
        };
        assert_eq!(shows(Pictures::Slices), (true, false, false));
        assert_eq!(shows(Pictures::SlicesAndScene), (true, true, false));
        assert_eq!(shows(Pictures::Planes), (false, true, true));
        assert_eq!(shows(Pictures::Scene), (false, true, false));
        assert_eq!(
            Pictures::default(),
            Pictures::Slices,
            "Volsa opens on the slices"
        );
    }
}
