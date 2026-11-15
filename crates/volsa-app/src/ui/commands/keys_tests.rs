//! The keys that arrange the pictures: S for the slices, T for the three planes, D for 3D.

use egui::Key;

use super::super::keys::{self, Held};
use super::*;
use crate::pictures::Pictures;
use crate::scene::SceneStatus;
use crate::testing::thick_scan_ready;

/// What pressing `key` alone does in `state`, while its action can be used.
fn pressed(key: Key, state: &AppState) -> Option<Action> {
    match keys::pressed(key, Held::default(), state)? {
        Does::Act(act) => act(state),
        Does::Ask(_) => None,
    }
}

#[test]
fn s_t_and_d_arrange_the_pictures_as_slices_three_planes_and_3d() {
    let mut state = thick_scan_ready();
    state.scene.status = SceneStatus::Ready;
    let mut flat = thick_scan_ready();
    flat.scene.status = SceneStatus::Unavailable(Vec::new());
    for (key, id, pictures) in [
        (Key::S, "pictures.slices", Pictures::Slices),
        (Key::T, "pictures.planes", Pictures::Planes),
        (Key::D, "pictures.scene", Pictures::Scene),
    ] {
        let set = Some(Action::SetPictures(pictures));
        assert_eq!(pressed(key, &state), set, "{key:?}");
        let shortcut = find(id).and_then(|command| command.key);
        assert_eq!(shortcut.map(|shortcut| shortcut.key), Some(key), "{id}");
        // With no 3D view, the arrangements that show it are left as they are.
        let wanted = set.filter(|_| pictures == Pictures::Slices);
        assert_eq!(pressed(key, &flat), wanted, "{key:?} with no 3D view");
    }
}
