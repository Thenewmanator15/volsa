//! What a picture of the window can ask for about the pictures: how they are arranged, and how
//! the 3D scene and the labelled parts are posed once there is something to pose.

use volsa_render::Look;

use super::asks_for;
use crate::action::Action;
use crate::pictures::Pictures;

/// What to do to the 3D scene, once its volume is ready, for a picture that asks for it by
/// the words in `wanted`: a look, a turn, a cut, the slice drawn in it.
pub fn pose(wanted: &str) -> Vec<Action> {
    let asked = |word: &str| asks_for(wanted, word);
    let poses = [
        ("vr", Action::SceneLook(Look::Vr)),
        ("mip", Action::SceneLook(Look::Mip)),
        ("turned", Action::SceneOrbit { by: [70.0, 35.0] }),
        ("cut", Action::CutAdd),
        ("slice", Action::ShowSliceInScene(true)),
        ("filled", Action::SetOverlay(volsa_render::Overlay::Filled)),
        ("smooth", Action::SetSmooth(true)),
        ("stepped", Action::StepVolume { viewport: 0, by: 1 }),
    ];
    let wanted = poses.into_iter().filter(|(word, _)| asked(word));
    wanted.map(|(_, action)| action).collect()
}

/// How a picture that asks for it by the words in `wanted` has the pictures arranged: the
/// three planes with the 3D view for `planes` or `quad`, the 3D view alone for `3d`, and the
/// slices beside it for `slices3d`; the last of those asked for. `None` for the slices alone,
/// as Volsa opens. It is set as the window opens, as a person would choose it, not once the
/// volume is ready as a pose is.
pub fn arrangement_asked(wanted: &str) -> Option<Pictures> {
    let words = [
        ("planes", Pictures::Planes),
        ("quad", Pictures::Planes),
        ("3d", Pictures::Scene),
        ("slices3d", Pictures::SlicesAndScene),
    ];
    let mut asked = wanted.split(',').filter_map(|part| {
        let word = part.trim();
        words.iter().find(|(said, _)| *said == word)
    });
    asked.next_back().map(|(_, pictures)| *pictures)
}

/// What to do to the labelled parts, once labels have been read, for a picture that asks
/// for it by the words in `wanted`: the parts as solids in the scene, the first part faded,
/// the first part selected.
pub fn label_pose(wanted: &str) -> Vec<Action> {
    let asked = |word: &str| asks_for(wanted, word);
    let faded = Action::PartOpacity {
        part: 0,
        opacity: 0.3,
    };
    let poses = [
        ("solids", Action::SceneLook(Look::Parts)),
        ("faded", faded),
        ("chosen", Action::PartSelect(Some(0))),
    ];
    let wanted = poses.into_iter().filter(|(word, _)| asked(word));
    wanted.map(|(_, action)| action).collect()
}
