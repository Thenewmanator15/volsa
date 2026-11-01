//! What a picture of the window can ask for about labelling a scan: the strip in the Tools tab
//! as it is on a computer that can label, whichever this one is (`labelling`), and a result
//! that Volsa made itself (`labelled`), for which the picture starts a run and waits for it.

use std::time::Duration;

use volsa_models::{Card, CardId, CardKind};

use super::{SHOW_VARIABLE, VARIABLE, asks_for as asked};
use crate::action::Action;
use crate::labelling::{LabellingAction, Ready, can_label};
use crate::labels::LabelsStatus;
use crate::state::AppState;

/// How long a picture waits for a run of labelling, in place of the usual patience: a run
/// takes minutes.
const RUN_PATIENCE: Duration = Duration::from_mins(30);

/// The look at this computer a picture of the strip is taken on, if it asks for one: this
/// computer is said to be able to label, and to have the two cards of [`cards`]. It stands in
/// for the first look, which is not made, so it answers the look numbered 0.
pub fn made_up_look() -> Option<LabellingAction> {
    std::env::var_os(VARIABLE)?;
    looked(&std::env::var(SHOW_VARIABLE).ok()?)
}

/// The made-up look the words `wanted` ask for, if they do.
pub(super) fn looked(wanted: &str) -> Option<LabellingAction> {
    asked(wanted, "labelling").then(|| LabellingAction::LabellingLooked {
        look: 0,
        ready: Ready::Yes,
        cards: cards(),
    })
}

/// Two made-up cards: one that shares the computer's memory, and one with memory of its own.
fn cards() -> Vec<Card> {
    let card = |number: u32, name: &str, kind, own_bytes| Card {
        id: CardId(u64::from(number)),
        index: number,
        name: name.to_owned(),
        kind,
        own_bytes,
    };
    vec![
        card(0, "Made-up shared graphics", CardKind::Integrated, 1 << 30),
        card(1, "Made-up card A", CardKind::Discrete, 8 << 30),
    ]
}

/// A run of labelling that a picture waits for, asked for by the word `labelled`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Run {
    /// The picture asks for none.
    Not,
    /// It asks for one that has not been started: labelling is not known to be able to run yet.
    ToStart,
    /// One has been started.
    On,
}

impl Run {
    /// What the words `wanted` ask for.
    pub fn asked(wanted: &str) -> Self {
        if asked(wanted, "labelled") {
            Self::ToStart
        } else {
            Self::Not
        }
    }

    /// What starts the run, once labelling can run: given once. When this computer is known
    /// not to be able to, the picture is of the strip as it is, and none is started.
    pub fn start(&mut self, state: &AppState) -> Option<Action> {
        if *self != Self::ToStart {
            return None;
        }
        if can_label(state).is_ok() {
            *self = Self::On;
            // A picture asks for no card of its own: it holds nothing of one.
            return Some(Action::Labelling(LabellingAction::LabelScan { held: 0 }));
        }
        if matches!(state.labelling.ready, Ready::No(_)) {
            *self = Self::Not;
        }
        None
    }

    /// Whether the picture can be taken: it asks for no run, or the run has ended and its
    /// labels are in.
    pub fn is_over(self, state: &AppState) -> bool {
        match self {
            Self::Not => true,
            Self::ToStart => false,
            Self::On => {
                state.labelling.running.is_none() && state.labels.status != LabelsStatus::Reading
            }
        }
    }

    /// How long the picture waits before it is taken whatever is on the screen.
    pub fn patience(self, usual: Duration) -> Duration {
        if self == Self::Not {
            usual
        } else {
            RUN_PATIENCE
        }
    }
}
