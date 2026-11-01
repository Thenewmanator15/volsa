//! Measuring on a slice by keyboard. With a measuring tool in hand and the keyboard on the
//! active viewport, the arrows move a point on its slice and Enter places it: this takes those
//! keys before the keys of the actions look at them, keeps Esc for what is half drawn, and tells
//! a screen reader what the viewport is for and what the point measures. The point itself, and
//! what Enter makes of it, are in `tools/keyed.rs`.

use egui::{Context, Event, Key, Response, Ui, WidgetInfo, WidgetType};

use super::said::said;
use super::tools::Drafts;
use crate::annotations::Tool;
use crate::state::AppState;
use crate::ui::{keys, typing};

/// How many pixels an arrow moves the point with Shift held.
const WITH_SHIFT: u32 = 10;

/// One key of the point, in the order pressed.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum Step {
    /// An arrow: move `by` pixels of the slice towards the way it points on the screen.
    Move {
        /// Where the arrow points: `[-1.0, 0.0]` for left, `[0.0, 1.0]` for down.
        toward: [f32; 2],
        /// How many pixels.
        by: u32,
    },
    /// Enter: place a point of the annotation at the keyboard point.
    Place,
}

/// What the keys did over a viewport this frame.
#[derive(Debug, Default, PartialEq)]
pub(super) struct Keyed {
    /// Whether the keyboard point is in use: a measuring tool is in hand, this is the active
    /// viewport and it has the keyboard, and nothing else (a text field, the search, a question
    /// or a menu) has the keys.
    pub in_use: bool,
    /// The keys of the point, in the order they were pressed.
    pub steps: Vec<Step>,
}

/// What a key event is to the point, if it is a key of it: an arrow (a pixel, or ten with Shift)
/// or Enter, pressed with no Ctrl, Command or Alt. A held Enter is placed once.
fn step_of(event: &Event) -> Option<Step> {
    let Event::Key {
        key,
        pressed: true,
        repeat,
        modifiers,
        ..
    } = event
    else {
        return None;
    };
    if modifiers.ctrl || modifiers.command || modifiers.alt {
        return None;
    }
    let by = if modifiers.shift { WITH_SHIFT } else { 1 };
    match key {
        Key::ArrowLeft => Some(Step::Move {
            toward: [-1.0, 0.0],
            by,
        }),
        Key::ArrowRight => Some(Step::Move {
            toward: [1.0, 0.0],
            by,
        }),
        Key::ArrowUp => Some(Step::Move {
            toward: [0.0, -1.0],
            by,
        }),
        Key::ArrowDown => Some(Step::Move {
            toward: [0.0, 1.0],
            by,
        }),
        // A key held down places once.
        Key::Enter if !*repeat => Some(Step::Place),
        _ => None,
    }
}

/// Whether the point takes this key, even when it makes nothing of it: the system's repeats of a
/// held Enter are taken, and nothing is made of them.
fn is_ours(event: &Event) -> bool {
    match event {
        Event::Key {
            key: Key::Enter,
            pressed: true,
            modifiers,
            ..
        } => !(modifiers.ctrl || modifiers.command || modifiers.alt),
        other => step_of(other).is_some(),
    }
}

/// Take the keys of the point from the frame's events, when the point is in use, so that
/// neither egui nor the keys of the actions see them: Up and Down then move the point and not
/// the slice.
pub(super) fn take(ui: &Ui, response: &Response, state: &AppState, at: usize) -> Keyed {
    let measuring = at == state.active && state.tool != Tool::Navigate;
    let in_use = measuring && response.has_focus() && !typing::keys_are_taken(ui.ctx());
    if !in_use {
        return Keyed::default();
    }
    let steps = ui.input_mut(|input| {
        let mut steps = Vec::new();
        input.events.retain(|event| {
            let ours = is_ours(event);
            steps.extend(step_of(event));
            !ours
        });
        steps
    });
    Keyed { in_use, steps }
}

/// Keep the arrows for a viewport that has the keyboard, and Esc too while something is half
/// drawn on it: the first Esc gives that up, the next leaves the viewport.
pub(super) fn keep(ctx: &Context, response: &Response, drafts: &Drafts, at: usize) {
    keys::hold_the_arrows(ctx, response, drafts.in_progress(at));
}

/// Tell a screen reader what viewport `at` is: what it has always said, and, while the point
/// is in use, its value, which says the tool and how to use it until the point is moved, and
/// then what the point measures.
pub(super) fn announce(
    response: &Response,
    state: &AppState,
    at: usize,
    drafts: &Drafts,
    keyed: &Keyed,
) {
    let value = keyed.in_use.then(|| {
        let said = drafts.said(at);
        said.map_or_else(|| how_to(state.tool), str::to_owned)
    });
    let name = said(state, at);
    response.widget_info(|| WidgetInfo {
        current_text_value: value.clone(),
        ..WidgetInfo::labeled(WidgetType::Image, true, name.clone())
    });
}

/// How to use the tool in hand with the keys, in one short sentence.
fn how_to(tool: Tool) -> String {
    let name = match tool {
        Tool::Navigate => return String::new(),
        Tool::Ruler => "Ruler",
        Tool::Angle => "Angle",
        Tool::Region => "Region",
        Tool::Marker => "Marker",
    };
    format!("{name}: arrows move the point, Enter places it")
}

#[cfg(test)]
mod tests {
    use egui::Modifiers;

    use super::*;

    fn pressed(key: Key, modifiers: Modifiers, repeat: bool) -> Event {
        Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat,
            modifiers,
        }
    }

    #[test]
    fn an_arrow_moves_a_pixel_and_ten_with_shift_and_enter_places_once() {
        let plain = |key| step_of(&pressed(key, Modifiers::NONE, false));
        assert_eq!(
            plain(Key::ArrowLeft),
            Some(Step::Move {
                toward: [-1.0, 0.0],
                by: 1
            })
        );
        let shifted = step_of(&pressed(Key::ArrowDown, Modifiers::SHIFT, false));
        assert_eq!(
            shifted,
            Some(Step::Move {
                toward: [0.0, 1.0],
                by: 10
            })
        );
        assert_eq!(plain(Key::Enter), Some(Step::Place));
        // Held down, an arrow goes on moving and Enter places once; both are the point's.
        let held = |key| step_of(&pressed(key, Modifiers::NONE, true));
        assert!(matches!(held(Key::ArrowRight), Some(Step::Move { .. })));
        assert_eq!(held(Key::Enter), None);
        assert!(is_ours(&pressed(Key::Enter, Modifiers::NONE, true)));
        // Other keys, and these with Ctrl, are not.
        assert_eq!(plain(Key::PageDown), None);
        assert!(!is_ours(&pressed(Key::PageDown, Modifiers::NONE, false)));
        assert!(!is_ours(&pressed(Key::ArrowUp, Modifiers::CTRL, false)));
        assert!(!is_ours(&pressed(Key::Enter, Modifiers::COMMAND, false)));
    }

    #[test]
    fn each_tool_says_how_it_is_used_with_the_keys() {
        for (tool, name) in [
            (Tool::Ruler, "Ruler"),
            (Tool::Angle, "Angle"),
            (Tool::Region, "Region"),
            (Tool::Marker, "Marker"),
        ] {
            let said = format!("{name}: arrows move the point, Enter places it");
            assert_eq!(how_to(tool), said);
        }
    }
}
