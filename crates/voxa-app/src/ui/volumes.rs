//! Which volume of a series is shown, and the two buttons that step between its volumes.

use egui::Ui;
use voxa_core::SeriesInfo;

use super::commands::{self, Request};
use crate::action::Action;
use crate::state::AppState;

/// Which volume this is of how many, and what sets it apart when the files say:
/// "Volume 2 of 5 · b 1000". `None` for a series of one volume.
pub fn label(info: &SeriesInfo) -> Option<String> {
    let set = info.set.as_ref()?;
    let place = format!("Volume {} of {}", set.at + 1, set.count);
    Some(if set.what.is_empty() {
        place
    } else {
        format!("{place} · {}", set.what)
    })
}

/// The volume the active viewport shows, with a button either side to step to the one before
/// and the one after. Nothing unless that viewport shows a series of several. Gives what the
/// buttons ask of the window, if anything.
pub fn stepper(ui: &mut Ui, state: &AppState, actions: &mut Vec<Action>) -> Option<Request> {
    let viewport = state.active;
    let shown = state.viewports.get(viewport);
    let listed = shown
        .and_then(|shown| shown.series.as_ref())
        .and_then(|series| state.series(series));
    let info = listed.map(|listed| &listed.info)?;
    let label = label(info)?;
    // The three go onto the next row together when they must.
    if ui.available_size_before_wrap().x < width_of(ui, &label) {
        ui.end_row();
    }
    // Each button is greyed at its end of the volumes.
    let together = ui.horizontal(|ui| {
        let back = commands::button(ui, "volume.before", state, None);
        ui.label(label);
        let on = commands::button(ui, "volume.after", state, None);
        back.or(on)
    });
    together.inner.and_then(|does| does.run(state, actions))
}

/// How wide the stepper is: a button either side and the words between them, with the spaces.
fn width_of(ui: &Ui, label: &str) -> f32 {
    let wide = |text: &str, style: egui::TextStyle| {
        let font = style.resolve(ui.style());
        let laid_out = ui
            .painter()
            .layout_no_wrap(text.to_owned(), font, egui::Color32::WHITE);
        laid_out.size().x
    };
    let spacing = ui.spacing();
    let button = wide("◀", egui::TextStyle::Button) + 2.0 * spacing.button_padding.x;
    let words = wide(label, egui::TextStyle::Body);
    2.0 * button + words + 3.0 * spacing.item_spacing.x
}

#[cfg(test)]
mod tests {
    use voxa_core::{SeriesId, VolumeSet};

    use super::*;

    fn volume(at: usize, count: usize, what: &str) -> SeriesInfo {
        SeriesInfo {
            set: Some(VolumeSet {
                of: SeriesId("whole".to_owned()),
                at,
                count,
                what: what.to_owned(),
            }),
            ..SeriesInfo::default()
        }
    }

    #[test]
    fn a_volume_says_which_it_is_and_what_sets_it_apart() {
        assert_eq!(
            label(&volume(1, 5, "b 850")).as_deref(),
            Some("Volume 2 of 5 · b 850")
        );
        assert_eq!(label(&volume(0, 3, "")).as_deref(), Some("Volume 1 of 3"));
        assert_eq!(label(&SeriesInfo::default()), None);
    }

    #[test]
    fn the_stepper_goes_onto_the_next_row_whole_when_the_row_is_too_full() {
        use crate::testing::{FIRST_ECHO, id, indexed, two_echoes_files};
        const WIDE: f32 = 500.0;
        let mut state = AppState::default();
        crate::reduce(
            &mut state,
            Action::IndexProgress(indexed(two_echoes_files())),
        );
        let series = id(FIRST_ECHO);
        crate::reduce(
            &mut state,
            Action::SelectSeries {
                viewport: 0,
                series,
            },
        );
        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(WIDE, 300.0));
        let input = egui::RawInput {
            screen_rect: Some(screen),
            ..egui::RawInput::default()
        };
        let mut output = ctx.run_ui(input, |ui| {
            ui.horizontal_wrapped(|ui| {
                // Room left on the row for little more than one button.
                ui.add_space(WIDE - 80.0);
                stepper(ui, &state, &mut Vec::new());
            });
        });
        // No renderer is behind this pass to take the font texture.
        output.textures_delta.clear();
        let described = output.platform_output.accesskit_update;
        let update = described.expect("the interface is described");
        let bounds = |wanted: &str| {
            let named = update.nodes.iter().map(|(_, node)| node);
            let mut found = named.filter(|node| node.label() == Some(wanted));
            found.find_map(egui::accesskit::Node::bounds).expect(wanted)
        };
        // A screen reader is told what each button does, not its arrow.
        let (back, on) = (bounds("The volume before"), bounds("The volume after"));
        assert!((back.y0 - on.y0).abs() < 0.5, "one row: {back:?} {on:?}");
        assert!(on.x1 <= f64::from(WIDE), "inside the toolbar: {on:?}");
    }
}
