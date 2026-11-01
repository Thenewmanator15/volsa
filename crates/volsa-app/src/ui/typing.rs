//! Who has the keyboard: a text field the person types in, a dialog that asks them something,
//! a menu that is open, or the window, whose keys they are. Volsa's keys, Esc that closes a window and Esc that
//! closes the list of notices act only when the keyboard is the window's.
//!
//! egui lets go of a field's focus at the start of the pass in which Esc is pressed, and when
//! Enter is pressed inside a single-line field, so by the time the window looks the field no
//! longer has it, and a menu that Esc closes is closed by then too. A key that ends typing or
//! closes a menu must not also do what it does for the window, so the pass before is
//! remembered: see [`note_the_pass`].

use egui::{Context, Id};

use super::{confirm, search};

/// Where it is noted that a text field had the keyboard when the last pass ended.
const TYPED_LAST_PASS: &str = "volsa: a text field had the keyboard at the end of the last pass";
/// Where it is noted that a menu was open when the last pass ended.
const MENU_LAST_PASS: &str = "volsa: a menu was open at the end of the last pass";

/// Whether the keyboard is not the window's: a text field has it or had it when the pass
/// before ended, a menu is open or was, or the search or a question to answer is open.
pub fn keys_are_taken(ctx: &Context) -> bool {
    typing(ctx)
        || a_menu_is_open(ctx)
        || search::holds_keyboard(ctx)
        || confirm::holds_keyboard(ctx)
}

/// Whether a menu is open, or was when the pass before ended: its items are the keyboard's to
/// move among and press, and the Esc that closes it is not also the window's.
fn a_menu_is_open(ctx: &Context) -> bool {
    let before = ctx.data(|data| data.get_temp::<bool>(Id::new(MENU_LAST_PASS)));
    egui::Popup::is_any_open(ctx) || before.unwrap_or(false)
}

/// Whether the person is typing in a field, or was when the pass before ended: egui has by now
/// let go of a field in which Esc or Enter was pressed.
pub fn typing(ctx: &Context) -> bool {
    let before = ctx.data(|data| data.get_temp::<bool>(Id::new(TYPED_LAST_PASS)));
    ctx.text_edit_focused() || before.unwrap_or(false)
}

/// Note, as a pass ends, whether a text field has the keyboard and whether a menu is open.
/// Called last in every pass.
pub fn note_the_pass(ctx: &Context) {
    let typing = ctx.text_edit_focused();
    let menu = egui::Popup::is_any_open(ctx);
    ctx.data_mut(|data| {
        data.insert_temp(Id::new(TYPED_LAST_PASS), typing);
        data.insert_temp(Id::new(MENU_LAST_PASS), menu);
    });
}

#[cfg(test)]
mod tests {
    use egui::{Event, Key, Modifiers, RawInput, TextEdit};

    use super::*;
    use crate::ui::floating::{Open, close_on_esc};

    /// One pass with these events in which a text field is drawn (and given the keyboard if
    /// `focus`), the window looks at its keys and at Esc for the floating windows, and the pass
    /// is noted as the window does at its end. Gives whether the window took any key and
    /// whether the tags window, which is open, is still open.
    fn pass(ctx: &Context, events: Vec<Event>, focus: bool, tags: &mut bool) -> (usize, bool) {
        let mut text = String::new();
        let mut taken = 0;
        let input = RawInput {
            events,
            ..RawInput::default()
        };
        let mut output = ctx.run_ui(input, |ui| {
            let field = ui.add(TextEdit::singleline(&mut text).id(Id::new("a field")));
            if focus {
                field.request_focus();
            }
            let (mut about, mut help) = (false, false);
            let open = Open {
                about: &mut about,
                help: &mut help,
                tags,
            };
            close_on_esc(ui.ctx(), open);
            taken = crate::ui::keys::pressed_keys(ui.ctx()).len();
            note_the_pass(ui.ctx());
        });
        // No renderer is behind these passes to take the font texture.
        output.textures_delta.clear();
        (taken, *tags)
    }

    /// A key pressed with nothing held.
    fn pressed(key: Key) -> Event {
        Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        }
    }

    #[test]
    fn esc_that_ends_typing_in_a_field_does_not_also_close_a_window_or_act_as_a_key() {
        let ctx = Context::default();
        let mut tags = true;
        // The field has the keyboard, and the pass ends.
        let _ = pass(&ctx, Vec::new(), true, &mut tags);
        let _ = pass(&ctx, Vec::new(), false, &mut tags);
        assert!(ctx.text_edit_focused(), "the field has the keyboard");
        // Esc: egui lets go of the field at the start of this pass, so the field no longer has
        // it when the window looks; the window must not take the Esc for its own.
        let (taken, open) = pass(&ctx, vec![pressed(Key::Escape)], false, &mut tags);
        assert!(!ctx.text_edit_focused(), "egui let go of the field");
        assert_eq!(
            (taken, open),
            (0, true),
            "Esc ended the typing and did no more"
        );
        // The next Esc is the window's.
        let (taken, open) = pass(&ctx, vec![pressed(Key::Escape)], false, &mut tags);
        assert_eq!(
            (taken, open),
            (0, false),
            "and it closes the window, taking the key"
        );
    }

    #[test]
    fn enter_that_ends_typing_in_a_field_is_not_a_key_of_the_window_either() {
        let ctx = Context::default();
        let mut tags = false;
        let _ = pass(&ctx, Vec::new(), true, &mut tags);
        let _ = pass(&ctx, Vec::new(), false, &mut tags);
        // Enter in a single-line field lets go of it in the pass it is pressed in.
        let (taken, _) = pass(
            &ctx,
            vec![pressed(Key::H), pressed(Key::Enter)],
            false,
            &mut tags,
        );
        assert_eq!(taken, 0, "{taken} keys of the window were taken");
        // (A key pressed again before it is let go is the system repeating it, so another key.)
        let (taken, _) = pass(&ctx, vec![pressed(Key::V)], false, &mut tags);
        assert_eq!(taken, 1, "the pass after, the keys are the window's again");
    }

    /// One pass with a menu button in it, as `pass` has a field: gives where the button is, how
    /// many keys of the window were taken, and whether the tags window is still open.
    fn menu_pass(ctx: &Context, events: Vec<Event>, tags: &mut bool) -> (egui::Rect, usize, bool) {
        let (mut at, mut taken) = (egui::Rect::NOTHING, 0);
        let input = RawInput {
            events,
            ..RawInput::default()
        };
        let mut output = ctx.run_ui(input, |ui| {
            let menu = ui.menu_button("A menu", |ui| {
                let _ = ui.button("An item");
            });
            at = menu.response.rect;
            let (mut about, mut help) = (false, false);
            let open = Open {
                about: &mut about,
                help: &mut help,
                tags,
            };
            close_on_esc(ui.ctx(), open);
            taken = crate::ui::keys::pressed_keys(ui.ctx()).len();
            note_the_pass(ui.ctx());
        });
        // No renderer is behind these passes to take the font texture.
        output.textures_delta.clear();
        (at, taken, *tags)
    }

    #[test]
    fn keys_are_not_the_windows_while_a_menu_is_open_and_the_esc_that_closes_it_is_the_menus() {
        let ctx = Context::default();
        let mut tags = true;
        let (button, taken, _) = menu_pass(&ctx, Vec::new(), &mut tags);
        assert_eq!(taken, 0);
        let (press, release) = {
            let [press, release] = crate::ui::seen::click_at(button.center());
            (press, release)
        };
        let _ = menu_pass(&ctx, press, &mut tags);
        let _ = menu_pass(&ctx, release, &mut tags);
        assert!(egui::Popup::is_any_open(&ctx), "the menu is open");
        // A letter is not for the picture behind the menu.
        let (_, taken, _) = menu_pass(&ctx, vec![pressed(Key::H)], &mut tags);
        assert_eq!(taken, 0, "H reached the window behind an open menu");
        // Esc closes the menu, and does no more: the tags window stays.
        let (_, taken, open) = menu_pass(&ctx, vec![pressed(Key::Escape)], &mut tags);
        assert!(!egui::Popup::is_any_open(&ctx), "Esc closed the menu");
        assert_eq!((taken, open), (0, true), "Esc also closed the tags window");
        // Then the keys are the window's again, from the pass after the one that noted the menu gone.
        let _ = menu_pass(&ctx, Vec::new(), &mut tags);
        // (A key pressed again before it is let go is the system repeating it, so another key.)
        let (_, taken, _) = menu_pass(&ctx, vec![pressed(Key::V)], &mut tags);
        assert_eq!(taken, 1, "the menu is closed, and V is the window's");
        let (_, _, open) = menu_pass(&ctx, vec![pressed(Key::Escape)], &mut tags);
        assert!(!open, "the next Esc closes the window");
    }
}
