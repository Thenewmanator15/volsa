//! Asking before work is thrown away: closing labels, deleting the labels kept for a series and
//! deleting a saved view. One question at a time, in a dialog over the window, with one plain
//! sentence and two buttons; the safe one is in hand first and Esc takes it. Only the other
//! button does the deed, so the work is lost on purpose or not at all, whether it was asked
//! for with a button or from the search.

use egui::{Button, Context, Id, Modal};

use super::commands::Confirm;
use crate::action::Action;
use crate::labelling::LabellingAction;
use crate::labels::LabelsStatus;
use crate::naming::printable;
use crate::state::AppState;
use crate::views::ViewsAction;

/// The dialog's own name, which it keeps its place under.
const DIALOG: &str = "volsa: a question before work is thrown away";
/// Where the window is told, for as long as the dialog is open, that it has the keyboard.
const HOLDS_KEYBOARD: &str = "volsa: a question has the keyboard";
/// The widest the dialog is, in points.
const WIDEST: f32 = 380.0;

/// What is asked before labels are closed.
pub const CLOSE_LABELS: &str = "Close these labels? Which parts you hid, faded or chose is \
                                forgotten. Labels from a file can be opened again from it.";
/// What is asked before labels that Volsa made and did not keep are closed.
pub const CLOSE_MADE: &str = "Close these labels? Volsa made them and has not kept them, so \
                              they are lost unless you save them first. Which parts you hid, \
                              faded or chose is forgotten too.";
/// What is asked before the labels kept for a series are deleted.
pub const DELETE_KEPT: &str = "Delete the labels kept for this series? They are removed from \
                               this computer and cannot be brought back. Making them again \
                               takes a few minutes.";

/// What a question says, and what saying yes does.
#[derive(Debug, Clone, PartialEq)]
pub struct Question {
    /// What is asked, in plain sentences.
    pub text: String,
    /// What the button that does it says.
    pub yes: &'static str,
    /// What the button that leaves everything as it was says.
    pub keep: &'static str,
    /// What saying yes does.
    pub then: Action,
}

/// The question to ask before `confirm`, in `state`; `None` when what it asks about is not
/// there any more, so there is nothing to throw away.
pub fn question(confirm: Confirm, state: &AppState) -> Option<Question> {
    match confirm {
        Confirm::CloseLabels => (state.labels.status == LabelsStatus::Ready).then(|| {
            // Labels that Volsa made and did not keep are gone once they are closed.
            let lost = state.labels.made.is_some() && state.labelling.kept.is_none();
            Question {
                text: (if lost { CLOSE_MADE } else { CLOSE_LABELS }).to_owned(),
                yes: "Close the labels",
                keep: "Keep them",
                then: Action::CloseLabels,
            }
        }),
        Confirm::DeleteKept => state.labelling.kept.is_some().then(|| Question {
            text: DELETE_KEPT.to_owned(),
            yes: "Delete them",
            keep: "Keep them",
            then: Action::Labelling(LabellingAction::DeleteKept),
        }),
        Confirm::DeleteView(at) => state.views.get(at).map(|view| Question {
            text: format!(
                "Delete the saved view \u{201c}{}\u{201d}? It cannot be brought back.",
                printable(&view.name)
            ),
            yes: "Delete the view",
            keep: "Keep it",
            then: Action::Views(ViewsAction::Delete(at)),
        }),
    }
}

/// Whether a question is open and so has the keyboard, which is then not for the keys of
/// actions nor for Esc to close a window.
pub fn holds_keyboard(ctx: &Context) -> bool {
    let held = ctx.data(|data| data.get_temp::<bool>(Id::new(HOLDS_KEYBOARD)));
    held.unwrap_or(false)
}

/// Note whether a question is open.
fn holds_keyboard_set(ctx: &Context, holds: bool) {
    ctx.data_mut(|data| data.insert_temp(Id::new(HOLDS_KEYBOARD), holds));
}

/// How a question was answered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Answer {
    /// Do it.
    Yes,
    /// Leave everything as it was.
    Keep,
}

/// Show the question for `asking` while there is one: gives what saying yes does, and closes
/// the question when it is answered, or when what it asks about is gone. Esc, a press outside
/// the dialog and the safe button all leave everything as it was.
pub fn show(ctx: &Context, state: &AppState, asking: &mut Option<Confirm>) -> Option<Action> {
    let Some(question) = (*asking).and_then(|confirm| question(confirm, state)) else {
        *asking = None;
        holds_keyboard_set(ctx, false);
        return None;
    };
    holds_keyboard_set(ctx, true);
    let mut answer = None;
    let shown = Modal::new(Id::new(DIALOG)).show(ctx, |ui| {
        ui.set_max_width(WIDEST);
        ui.label(&question.text);
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            let keep = ui.add(Button::new(question.keep));
            let yes = ui.add(Button::new(question.yes));
            // The safe button is in hand until the person moves to the other one.
            let in_hand = ui.memory(egui::Memory::focused);
            if in_hand != Some(yes.id) && in_hand != Some(keep.id) {
                keep.request_focus();
            }
            if yes.clicked() {
                answer = Some(Answer::Yes);
            } else if keep.clicked() {
                answer = Some(Answer::Keep);
            }
        });
    });
    if answer.is_none() && shown.should_close() {
        answer = Some(Answer::Keep);
    }
    if answer.is_some() {
        *asking = None;
        holds_keyboard_set(ctx, false);
    }
    (answer == Some(Answer::Yes)).then_some(question.then)
}

#[cfg(test)]
mod tests;
