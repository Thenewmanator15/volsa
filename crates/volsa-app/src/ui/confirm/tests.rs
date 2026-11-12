//! Asking before work is thrown away: the questions, what each answer does, and that the
//! keyboard is the question's while it is open.

use egui::{Context, Event, Key};

use super::*;
use crate::testing::thick_scan_ready;
use crate::ui::seen::{self, Seen, click_at};
use crate::ui::tabs::made_up::{labelled_by, named};
use crate::views::SavedView;
use volsa_core::WindowLevel;
use volsa_labels::Made;

/// A state with labels that Volsa made open on the series, a result kept for it, and a saved
/// view of its scene.
fn with_work() -> AppState {
    let made = Made {
        by: "Volsa 9.8.7".to_owned(),
        model: "Made-up model".to_owned(),
        model_sum: String::new(),
        device: "Made-up card".to_owned(),
        when: "2099-01-02".to_owned(),
        added: None,
        notes: Vec::new(),
    };
    let mut state = labelled_by(None, crate::labels::NamesFrom::Model, Some(made));
    state.labelling.kept = Some("2099-01-02".to_owned());
    let window = WindowLevel::new(1.0, 2.0);
    state.views = vec![SavedView::of("Front", 1, &state.scene, window)];
    state
}

/// One pass of the question for `asking` in `state`, with these events.
fn pass(
    ctx: &Context,
    state: &AppState,
    asking: &mut Option<Confirm>,
    events: Vec<Event>,
) -> (Seen, Option<Action>) {
    let mut answer = None;
    let ((), seen) = seen::pass(ctx, [800.0, 600.0], events, |ui| {
        answer = show(ui.ctx(), state, asking);
    });
    (seen, answer)
}

/// The question settled, as it is from the pass after it first appears.
fn settled(ctx: &Context, state: &AppState, asking: &mut Option<Confirm>) -> Seen {
    let mut last = Seen::default();
    for _ in 0..3 {
        last = pass(ctx, state, asking, Vec::new()).0;
    }
    last
}

#[test]
fn each_question_says_what_would_be_lost_in_plain_words() {
    let state = with_work();
    let ask = |confirm| question(confirm, &state).expect("there is something to throw away");
    let close = ask(Confirm::CloseLabels);
    assert_eq!(close.then, Action::CloseLabels);
    // Labels Volsa made and did not keep are lost for good; kept ones can be shown again.
    assert_eq!(close.text, CLOSE_LABELS);
    let mut unkept = with_work();
    unkept.labelling.kept = None;
    let lost = question(Confirm::CloseLabels, &unkept).expect("asked");
    assert_eq!(lost.text, CLOSE_MADE);
    let delete = ask(Confirm::DeleteKept);
    assert_eq!(delete.text, DELETE_KEPT);
    assert_eq!(delete.then, Action::Labelling(LabellingAction::DeleteKept));
    let view = ask(Confirm::DeleteView(0));
    assert_eq!(
        view.text,
        "Delete the saved view \u{201c}Front\u{201d}? It cannot be brought back."
    );
    assert_eq!(view.then, Action::Views(ViewsAction::Delete(0)));
    // Each offers the safe answer first.
    for asked in [close, delete, view] {
        assert!(asked.keep.starts_with("Keep"), "{}", asked.keep);
    }
}

#[test]
fn nothing_is_asked_where_there_is_nothing_to_throw_away() {
    let nothing = thick_scan_ready();
    for confirm in [
        Confirm::CloseLabels,
        Confirm::DeleteKept,
        Confirm::DeleteView(0),
    ] {
        assert_eq!(question(confirm, &nothing), None, "{confirm:?}");
    }
    // A question that is open when its work goes away closes itself, and lets go of the keys.
    let ctx = seen::context(1.0);
    let mut asking = Some(Confirm::DeleteView(0));
    let _ = settled(&ctx, &with_work(), &mut asking);
    assert!(holds_keyboard(&ctx));
    let (_, answer) = pass(&ctx, &nothing, &mut asking, Vec::new());
    assert_eq!((asking, answer), (None, None));
    assert!(!holds_keyboard(&ctx));
}

#[test]
fn only_the_button_that_says_so_does_the_deed() {
    let state = named();
    let confirm = Confirm::CloseLabels;
    let ctx = seen::context(1.0);
    let mut asking = Some(confirm);
    let seen = settled(&ctx, &state, &mut asking);
    let yes = question(confirm, &state).expect("asked").yes;
    let place = seen.place_of(yes).expect("the button that does it");
    let [press, release] = click_at(place.center());
    let _ = pass(&ctx, &state, &mut asking, press);
    let (_, answer) = pass(&ctx, &state, &mut asking, release);
    assert_eq!(answer, Some(Action::CloseLabels));
    assert_eq!(asking, None, "answered");
}

#[test]
fn keeping_it_esc_and_a_press_outside_leave_everything_as_it_was() {
    let state = with_work();
    let confirm = Confirm::DeleteKept;
    let keep = question(confirm, &state).expect("asked").keep;
    // The safe button.
    let ctx = seen::context(1.0);
    let mut asking = Some(confirm);
    let seen = settled(&ctx, &state, &mut asking);
    let place = seen.place_of(keep).expect("the safe button");
    let [press, release] = click_at(place.center());
    let _ = pass(&ctx, &state, &mut asking, press);
    let (_, answer) = pass(&ctx, &state, &mut asking, release);
    assert_eq!((answer, asking), (None, None));
    // Esc.
    let ctx = seen::context(1.0);
    let mut asking = Some(confirm);
    let _ = settled(&ctx, &state, &mut asking);
    let (_, answer) = pass(&ctx, &state, &mut asking, vec![seen::key(Key::Escape)]);
    assert_eq!((answer, asking), (None, None));
    // A press on the backdrop, away from the dialog.
    let ctx = seen::context(1.0);
    let mut asking = Some(confirm);
    let _ = settled(&ctx, &state, &mut asking);
    let [press, release] = click_at(egui::pos2(10.0, 10.0));
    let _ = pass(&ctx, &state, &mut asking, press);
    let (_, answer) = pass(&ctx, &state, &mut asking, release);
    assert_eq!((answer, asking), (None, None));
}

#[test]
fn the_safe_button_is_in_hand_first_so_that_enter_keeps_the_work() {
    let state = with_work();
    let confirm = Confirm::DeleteKept;
    let ctx = seen::context(1.0);
    let mut asking = Some(confirm);
    let _ = settled(&ctx, &state, &mut asking);
    let (_, answer) = pass(&ctx, &state, &mut asking, vec![seen::key(Key::Enter)]);
    assert_eq!(answer, None, "Enter on the safe button does not delete");
    assert_eq!(asking, None);
}

#[test]
fn while_a_question_is_open_the_keys_of_the_window_are_not_taken() {
    let state = with_work();
    let ctx = seen::context(1.0);
    let mut asking = Some(Confirm::CloseLabels);
    let mut taken = None;
    for _ in 0..3 {
        let events = vec![seen::key(Key::H)];
        let ((), _) = seen::pass(&ctx, [800.0, 600.0], events, |ui| {
            let _ = show(ui.ctx(), &state, &mut asking);
            taken = Some(crate::ui::keys::pressed_keys(ui.ctx()));
        });
    }
    assert_eq!(taken, Some(Vec::new()));
}
