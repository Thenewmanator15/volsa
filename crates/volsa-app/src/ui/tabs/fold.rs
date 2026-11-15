//! The folds of the tabs: "More", near the foot of a tab, which holds what is seldom needed and
//! is closed until it is opened; and the Tools tab's fold of the plugins the person added.
//! Whether each is open is kept in egui's memory under a name of its own, not under the place
//! it is drawn, so that search can open the fold an action it found lives in.

use egui::{CollapsingHeader, Context, Id, Ui};

use crate::plugins::ADD_ACTION;
use crate::ui::panel::Tab;

/// What the fold of seldom-needed controls is called, in every tab that has one.
pub const MORE: &str = "More";

/// What the fold of the plugins the person added is called.
pub const ADDED: &str = "Added by you";

/// A fold of a tab.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fold {
    /// The "More" of a tab.
    More(Tab),
    /// The plugins the person added, in the Tools tab.
    Added,
}

/// The actions drawn in a fold, by their ids, with the fold.
const FOLDED: [(&str, Fold); 9] = [
    ("planes.smooth", Fold::More(Tab::View)),
    ("planes.link", Fold::More(Tab::View)),
    ("measure.angle", Fold::More(Tab::Measure)),
    ("measure.region", Fold::More(Tab::Measure)),
    ("measure.save_json", Fold::More(Tab::Measure)),
    ("labels.save", Fold::More(Tab::Labels)),
    ("labels.names", Fold::More(Tab::Labels)),
    ("labels.overlay", Fold::More(Tab::Labels)),
    (ADD_ACTION, Fold::Added),
];

impl Fold {
    /// The name its state is kept under.
    const fn key(self) -> &'static str {
        match self {
            Self::More(Tab::View) => "volsa: the View tab's More",
            Self::More(Tab::Scene) => "volsa: the 3D tab's More",
            Self::More(Tab::Measure) => "volsa: the Measure tab's More",
            Self::More(Tab::Labels) => "volsa: the Labels tab's More",
            Self::More(Tab::Tools) => "volsa: the Tools tab's More",
            Self::Added => "volsa: the plugins added by you",
        }
    }

    /// What it is called.
    pub const fn title(self) -> &'static str {
        match self {
            Self::More(_) => MORE,
            Self::Added => ADDED,
        }
    }

    /// The fold the action known by `id` is drawn in, if it is in one. The three ways of
    /// drawing the labels over the planes share the start of their ids.
    pub fn of(id: &str) -> Option<Self> {
        let found = FOLDED.iter().find(|(start, _)| {
            id == *start
                || id
                    .strip_prefix(start)
                    .is_some_and(|rest| rest.starts_with('.'))
        });
        found.map(|(_, fold)| *fold)
    }

    /// Whether it is open: as it was left, or `unless_left` if it never was.
    pub fn is_open(self, ctx: &Context, unless_left: bool) -> bool {
        let left = ctx.data_mut(|data| data.get_persisted::<bool>(Id::new(self.key())));
        left.unwrap_or(unless_left)
    }

    /// Leave it open, or closed.
    pub fn set(self, ctx: &Context, open: bool) {
        ctx.data_mut(|data| data.insert_persisted(Id::new(self.key()), open));
    }
}

/// Draw `fold`: its heading, which opens and closes it, and beneath it, while it is open, what
/// `add` adds. It is open as it was left, or as `unless_left` says if it never was. What `add`
/// gave, if it was drawn.
pub fn show<R>(
    ui: &mut Ui,
    fold: Fold,
    unless_left: bool,
    add: impl FnOnce(&mut Ui) -> R,
) -> Option<R> {
    let open = fold.is_open(ui.ctx(), unless_left);
    let shown = CollapsingHeader::new(fold.title())
        .id_salt(fold.key())
        .default_open(open)
        .open(Some(open))
        .show(ui, add);
    if shown.header_response.clicked() {
        fold.set(ui.ctx(), !open);
    }
    shown.body_returned
}

/// Where the action known by `id` lives, in a few words for a person looking for it, from
/// `said`, where its place says it lives: with the fold it is in, if it is in one.
pub fn said_of(id: &str, said: &str) -> String {
    match Fold::of(id) {
        Some(fold) => format!("{said}, under {}", fold.title()),
        None => said.to_owned(),
    }
}

/// Open the fold the action known by `id` is drawn in, if it is in one, for what it did to be
/// in sight when its tab is looked at.
pub fn open_for(ctx: &Context, id: &str) {
    if let Some(fold) = Fold::of(id) {
        fold.set(ctx, true);
    }
}

/// Open every fold of every tab: for a picture of the window that asks for them open.
pub fn open_all(ctx: &Context) {
    for tab in Tab::ALL {
        Fold::More(tab).set(ctx, true);
    }
    Fold::Added.set(ctx, true);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::commands;

    #[test]
    fn every_action_said_to_be_in_a_fold_is_one_in_the_list() {
        for (id, _) in FOLDED {
            let found = commands::all()
                .iter()
                .any(|command| Fold::of(command.id).is_some() && command.id.starts_with(id));
            assert!(found, "{id} is an action");
        }
    }

    #[test]
    fn the_ways_of_drawing_labels_over_the_planes_are_under_more_and_others_are_not() {
        for way in ["off", "outline", "filled"] {
            let id = format!("labels.overlay.{way}");
            assert_eq!(Fold::of(&id), Some(Fold::More(Tab::Labels)), "{id}");
        }
        assert_eq!(Fold::of("labels.overlayed"), None);
        assert_eq!(Fold::of("measure.ruler"), None);
        assert_eq!(
            said_of("measure.ruler", "In the Measure tab"),
            "In the Measure tab"
        );
        let angle = said_of("measure.angle", "In the Measure tab");
        assert_eq!(angle, "In the Measure tab, under More");
    }

    #[test]
    fn a_fold_is_closed_until_it_is_opened_and_stays_as_it_was_left() {
        let ctx = Context::default();
        let fold = Fold::More(Tab::View);
        assert!(!fold.is_open(&ctx, false));
        assert!(Fold::Added.is_open(&ctx, true), "unless it says otherwise");
        open_for(&ctx, "planes.link");
        assert!(fold.is_open(&ctx, false));
        assert!(
            !Fold::More(Tab::Measure).is_open(&ctx, false),
            "each tab its own"
        );
        fold.set(&ctx, false);
        assert!(!fold.is_open(&ctx, true));
    }
}
