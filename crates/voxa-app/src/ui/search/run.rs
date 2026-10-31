//! What choosing a result does.

use super::entries::Target;
use crate::action::Action;
use crate::state::{AppState, Notice, NoticeKind};
use crate::ui::commands::{self, Command, Request, Tab, Why};

/// What is said of an action that is listed but not offered for the scan shown.
const NOT_OFFERED: &str = "This is not offered for the scan shown.";

/// The key of the notice that says why a chosen action could not be done: a new one takes the
/// old one's place.
const NOTICE_KEY: &str = "search";

/// Why `command` cannot be used now, in a sentence, if it cannot.
pub fn unavailable(command: &Command, state: &AppState) -> Option<Why> {
    if commands::offered(command, state) {
        (command.why_not)(state)
    } else {
        Some(Why::Borrowed(NOT_OFFERED))
    }
}

/// Do what choosing `target` asks in `state`: changes of state go onto `actions`, and what is
/// asked of the window is given back.
///
/// An action that cannot be used now says why in a notice, in place of doing anything. A part
/// is shown if it was hidden, then selected, and the Labels tab is shown. A scan is shown in
/// the active viewport. A help topic is opened.
pub fn run(target: &Target, state: &AppState, actions: &mut Vec<Action>) -> Option<Request> {
    match target {
        Target::Command(id) => {
            let command = commands::find(id)?;
            match unavailable(command, state) {
                Some(why) => {
                    actions.push(Action::Post(Notice {
                        kind: NoticeKind::Info,
                        text: why.into_owned(),
                        details: Vec::new(),
                        key: Some(NOTICE_KEY),
                    }));
                    None
                }
                None => command.does.clone().run(state, actions),
            }
        }
        Target::Part(part) => {
            let hidden = state.labels.shown.get(*part).is_some_and(|how| !how.shown);
            if hidden {
                actions.push(Action::PartShow {
                    part: *part,
                    shown: true,
                });
            }
            actions.push(Action::PartSelect(Some(*part)));
            Some(Request::ShowTab(Tab::Labels))
        }
        Target::Scan(series) => {
            actions.push(Action::SelectSeries {
                viewport: state.active,
                series: series.clone(),
            });
            None
        }
        Target::Help(topic) => Some(Request::Help(Some(*topic))),
    }
}
