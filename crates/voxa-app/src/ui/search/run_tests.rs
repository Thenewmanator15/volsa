//! What choosing a result does, and that nothing typed is kept.

use super::entries::Target;
use super::run::{run, unavailable};
use super::*;
use crate::action::Action;
use crate::annotations::Shape;
use crate::reducer::reduce;
use crate::testing::{labelled_head, thick_scan_ready};
use crate::ui::commands::{self, Command, Request, Tab};

#[test]
fn an_action_not_offered_for_the_scan_shown_says_so() {
    let state = thick_scan_ready();
    let fit = commands::find("view.fit").expect("listed");
    let for_ct = Command {
        profiles: &[crate::profile::Profile::Ct],
        ..*fit
    };
    // The made-up scans are MR.
    assert_eq!(
        unavailable(&for_ct, &state),
        Some("This is not offered for the scan shown.")
    );
    assert_eq!(unavailable(fit, &state), None);
}

#[test]
fn another_export_with_as_many_series_is_a_different_list() {
    let first = thick_scan_ready();
    let mut second = first.clone();
    let index = second.index.as_mut().expect("an export is open");
    for series in &mut index.series {
        series.info.id = crate::testing::id(&format!("other.{}", series.info.number));
    }
    assert_ne!(
        super::entries::stamp(&first),
        super::entries::stamp(&second)
    );
    assert_eq!(
        super::entries::stamp(&first),
        super::entries::stamp(&first.clone())
    );
}

/// What running `target` gives in `state`.
fn ran(target: &Target, state: &AppState) -> (Vec<Action>, Option<Request>) {
    let mut actions = Vec::new();
    let asked = run(target, state, &mut actions);
    (actions, asked)
}

#[test]
fn choosing_a_result_does_what_its_kind_does() {
    let (state, _) = labelled_head();
    // An action: what its entry in the list does.
    let (actions, asked) = ran(&Target::Command("layout.two"), &state);
    assert_eq!(
        actions,
        [Action::SetLayout(crate::state::Layout::TwoAcross)]
    );
    assert_eq!(asked, None);
    let (actions, asked) = ran(&Target::Command("help.keys"), &state);
    assert_eq!((actions, asked), (Vec::new(), Some(Request::Help(None))));
    // A part: selected, and the Labels tab shown.
    let (actions, asked) = ran(&Target::Part(1), &state);
    assert_eq!(actions, [Action::PartSelect(Some(1))]);
    assert_eq!(asked, Some(Request::ShowTab(Tab::Labels)));
    // A scan: shown in the active viewport.
    let series = crate::testing::id(crate::testing::FINE_SCAN);
    let (actions, asked) = ran(&Target::Scan(series.clone()), &state);
    let shown = Action::SelectSeries {
        viewport: state.active,
        series,
    };
    assert_eq!((actions, asked), (vec![shown], None));
    // A help topic: opened at it.
    let (actions, asked) = ran(&Target::Help("measure"), &state);
    assert_eq!(
        (actions, asked),
        (Vec::new(), Some(Request::Help(Some("measure"))))
    );
}

#[test]
fn a_hidden_part_is_shown_before_it_is_selected() {
    let (mut state, _) = labelled_head();
    reduce(
        &mut state,
        Action::PartShow {
            part: 1,
            shown: false,
        },
    );
    let (actions, _) = ran(&Target::Part(1), &state);
    let expected = [
        Action::PartShow {
            part: 1,
            shown: true,
        },
        Action::PartSelect(Some(1)),
    ];
    assert_eq!(actions, expected);
    for action in actions {
        reduce(&mut state, action);
    }
    assert_eq!(state.labels.selected, Some(1));
}

#[test]
fn clearing_the_measurements_through_search_can_be_taken_back() {
    let mut state = thick_scan_ready();
    reduce(
        &mut state,
        Action::AnnotationAdd(Shape::Marker { at: [0.0; 3] }),
    );
    assert_eq!(state.annotations.items.len(), 1);
    let (actions, asked) = ran(&Target::Command("measure.clear"), &state);
    assert_eq!(
        (actions.as_slice(), asked),
        ([Action::AnnotationsClear].as_slice(), None)
    );
    for action in actions {
        reduce(&mut state, action);
    }
    assert_eq!(state.annotations.items.len(), 0);
    reduce(&mut state, Action::Undo);
    assert_eq!(state.annotations.items.len(), 1, "Undo brings it back");
    // And its hint says so, wherever it is read.
    let hint = commands::hint(commands::find("measure.clear").expect("listed"));
    assert!(hint.contains("undone"), "{hint}");
}

#[test]
fn every_action_in_the_list_runs_through_search_without_a_panic() {
    let states = [AppState::default(), thick_scan_ready(), labelled_head().0];
    for state in &states {
        for command in commands::all() {
            let (actions, asked) = ran(&Target::Command(command.id), state);
            let reason = unavailable(command, state);
            let posted = matches!(actions.as_slice(), [Action::Post(_)]);
            assert_eq!(posted, reason.is_some(), "{}", command.id);
            if posted {
                assert_eq!(
                    asked, None,
                    "{} asks nothing of the window while unavailable",
                    command.id
                );
            }
        }
    }
}

#[test]
fn nothing_typed_is_kept() {
    // The box has no way to be written out: it derives no serde traits.
    let source = include_str!("../search.rs");
    for line in source.lines().filter(|line| line.contains("derive(")) {
        assert!(
            !line.contains("Serialize") && !line.contains("Deserialize"),
            "{line}"
        );
    }
    assert!(!source.contains("use serde"));
    // And what is remembered between runs has no part of it.
    let kept = serde_json::to_value(crate::prefs::Prefs::default()).expect("written out");
    let kept = kept.as_object().expect("an object");
    for key in kept.keys() {
        let key = key.to_lowercase();
        assert!(!key.contains("search") && !key.contains("query"), "{key}");
    }
}

#[test]
fn labelling_is_found_by_search_and_runs_as_the_button_does() {
    use super::entries::entries_for;
    use super::matching::ranked;
    use crate::ui::tabs::made_up::{able, sweep_tools};
    let state = able();
    let entries = entries_for(&state);
    let labelling = Target::Command("tools.label");
    for query in ["Label this brain scan", "segment", "synthseg", "parts"] {
        let top = ranked(query, &entries);
        let among_first_eight = top
            .iter()
            .take(8)
            .any(|&at| entries[at].target == labelling);
        assert!(among_first_eight, "{query:?} does not find the labelling");
    }
    // Choosing it asks the window for what the button in the Tools tab asks it for.
    let (actions, asked) = ran(&labelling, &state);
    let (_, button_asked) = sweep_tools(&state);
    assert_eq!((actions, asked), (vec![], Some(Request::LabelScan)));
    assert_eq!(button_asked, [Request::LabelScan]);
    // Where the scan cannot be labelled it says so in a notice and asks for nothing.
    let (actions, asked) = ran(&labelling, &AppState::default());
    assert_eq!(asked, None);
    assert!(
        matches!(actions.as_slice(), [Action::Post(notice)] if notice.text.contains("cannot label"))
    );
}
