//! The bar across the top of the window.

use egui::Ui;
use voxa_core::WindowLevel;
use voxa_volume::PRESETS;

use crate::action::Action;
use crate::state::{AppState, Cine, Layout};

/// The kinds of thing a scan can be opened from. The system's own dialogs choose either a
/// folder or a file, never both, so there is a button for each.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pick {
    /// A folder of DICOM files.
    Folder,
    /// A zip file, a DICOMDIR or a single DICOM file.
    File,
}

/// What the person asked for that is not a change of state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "one for each window a button of the bar opens"
)]
pub struct Asked {
    /// To choose a scan to open, and which kind.
    pub open: Option<Pick>,
    /// To see the About window.
    pub about: bool,
    /// To see the list of keys and what the mouse does.
    pub shortcuts: bool,
    /// To open or close the tag browser.
    pub tags: bool,
    /// To open or close the measurements window.
    pub measure: bool,
}

/// The layouts offered, with the label and hover text of each button.
const LAYOUTS: [(Layout, &str, &str); 3] = [
    (Layout::One, "1", "One viewport"),
    (Layout::TwoAcross, "2", "Two viewports side by side"),
    (Layout::TwoByTwo, "4", "Four viewports"),
];

/// Draw the toolbar: open, the layouts, the window of the active viewport, the display
/// switches, playback, the measurements, the tag browser, the list of keys and About. In a narrow window the
/// bar wraps onto a second line.
pub fn show(ui: &mut Ui, state: &AppState, actions: &mut Vec<Action>) -> Asked {
    let mut asked = Asked::default();
    ui.horizontal_wrapped(|ui| {
        ui.label("Open");
        let folder = ui
            .button("Folder…")
            .on_hover_text("Open a folder of DICOM files");
        let file = ui
            .button("File…")
            .on_hover_text("Open a zip file or a DICOMDIR");
        if folder.clicked() {
            asked.open = Some(Pick::Folder);
        } else if file.clicked() {
            asked.open = Some(Pick::File);
        }
        ui.separator();
        ui.label("Layout");
        for (layout, label, hint) in LAYOUTS {
            let chosen = state.layout == layout;
            if ui
                .selectable_label(chosen, label)
                .on_hover_text(hint)
                .clicked()
                && !chosen
            {
                actions.push(Action::SetLayout(layout));
            }
        }
        ui.separator();
        window(ui, state, actions);
        ui.separator();
        switches(ui, state, actions);
        ui.separator();
        playback(ui, state, actions);
        ui.separator();
        let measure = ui.button("Measure");
        asked.measure = measure
            .on_hover_text("Rulers, angles, regions and markers, and the list of them (M)")
            .clicked();
        let tags = ui.button("Tags");
        asked.tags = tags
            .on_hover_text("Every element of the file behind the slice shown")
            .clicked();
        asked.shortcuts = ui.button("?").on_hover_text("Keys and mouse").clicked();
        asked.about = ui.button("About").clicked();
    });
    asked
}

/// The window and level of the active viewport, as numbers that can be typed or dragged,
/// and the presets.
fn window(ui: &mut Ui, state: &AppState, actions: &mut Vec<Action>) {
    let viewport = state.active;
    let Some(shown) = state.viewports.get(viewport) else {
        return;
    };
    ui.label("Window");
    let (mut width, mut level) = (shown.wl.width, shown.wl.center);
    let step = f64::from((shown.wl.width / 100.0).max(0.1));
    let width_field = egui::DragValue::new(&mut width)
        .speed(step)
        .prefix("W ")
        .range(1.0..=f32::MAX);
    let level_field = egui::DragValue::new(&mut level).speed(step).prefix("L ");
    let width_changed = ui
        .add(width_field)
        .on_hover_text("Width: the span of values from black to white")
        .changed();
    let level_changed = ui
        .add(level_field)
        .on_hover_text("Level: the value shown as mid-grey")
        .changed();
    if width_changed || level_changed {
        actions.push(Action::SetWindow {
            viewport,
            wl: WindowLevel::new(level, width),
        });
    }
    for (preset, named) in PRESETS.iter().enumerate() {
        let hint = format!(
            "From the {} to the {} percentile of the slice shown",
            named.low_pct, named.high_pct
        );
        if ui.button(named.name).on_hover_text(hint).clicked() {
            actions.push(Action::ApplyPreset { viewport, preset });
        }
    }
}

/// What the Link switch says when pointed at.
pub const LINK_SAYS: &str = "Keep the slices of this study on the planes' position, and the position on \
                             the slice scrolled to. Linked views take it that the patient did not \
                             move between series.";

/// The display convention, whether the patient's details are shown, and whether the slice
/// views follow the planes' position.
fn switches(ui: &mut Ui, state: &AppState, actions: &mut Vec<Action>) {
    let convention = "On: the patient's left is on the right of the screen, as a radiologist reads it.\n\
                      Off: neurological, the patient's left on the left.";
    if ui
        .selectable_label(state.radiological, "Radiological")
        .on_hover_text(convention)
        .clicked()
    {
        actions.push(Action::ToggleRadiological);
    }
    let patient = "Show the patient's name and identifier in the viewports and the tag browser";
    if ui
        .selectable_label(state.show_patient, "Patient details")
        .on_hover_text(patient)
        .clicked()
    {
        actions.push(Action::TogglePatient);
    }
    let linked = state.planes.linked;
    let link = ui.selectable_label(linked, "Link");
    if link.on_hover_text(LINK_SAYS).clicked() {
        actions.push(Action::SetLinked(!linked));
    }
}

/// Film-like playback through the slices of the active viewport.
fn playback(ui: &mut Ui, state: &AppState, actions: &mut Vec<Action>) {
    let viewport = state.active;
    let cine = state.viewports.get(viewport).and_then(|shown| shown.cine);
    let cine = cine.unwrap_or(Cine::new(Cine::DEFAULT_FPS, true, false));
    let label = if cine.playing { "Pause" } else { "Play" };
    if ui
        .button(label)
        .on_hover_text("Play through the slices (Space)")
        .clicked()
    {
        actions.push(Action::CineToggle { viewport });
    }
    let (mut fps, mut looping) = (cine.fps, cine.looping);
    let speed = egui::DragValue::new(&mut fps)
        .speed(0.5)
        .range(Cine::MIN_FPS..=Cine::MAX_FPS)
        .suffix(" /s");
    let speed_changed = ui
        .add(speed)
        .on_hover_text("Slices shown each second")
        .changed();
    let loop_hint = "Start again from the first slice after the last";
    let loop_changed = ui
        .checkbox(&mut looping, "Loop")
        .on_hover_text(loop_hint)
        .changed();
    if speed_changed || loop_changed {
        actions.push(Action::CineSet {
            viewport,
            fps,
            looping,
        });
    }
}

#[cfg(test)]
mod tests {
    use egui::{Event, Modifiers, PointerButton, Pos2, RawInput, Rect, pos2, vec2};

    use super::*;

    fn pass(ctx: &egui::Context, state: &AppState, events: Vec<Event>) -> (Vec<Action>, Asked) {
        let mut actions = Vec::new();
        let mut asked = Asked::default();
        let screen = Rect::from_min_size(Pos2::ZERO, vec2(1700.0, 600.0));
        let input = RawInput {
            screen_rect: Some(screen),
            events,
            ..RawInput::default()
        };
        let mut output = ctx.run_ui(input, |ui| asked = show(ui, state, &mut actions));
        // No renderer is behind these passes to take the font texture.
        output.textures_delta.clear();
        (actions, asked)
    }

    /// Press and release the main button at a point.
    fn click(ctx: &egui::Context, state: &AppState, at: Pos2) -> (Vec<Action>, Asked) {
        let button = |pressed| Event::PointerButton {
            pos: at,
            button: PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        };
        let _ = pass(ctx, state, vec![Event::PointerMoved(at), button(true)]);
        pass(ctx, state, vec![button(false)])
    }

    /// Click all the way along the bar and gather everything that was asked for.
    fn sweep(state: &AppState) -> (Vec<Action>, Asked) {
        let (actions, all, _) = sweep_with_picks(state);
        (actions, all)
    }

    /// As [`sweep`], with every kind of thing asked to be opened, in the order met.
    fn sweep_with_picks(state: &AppState) -> (Vec<Action>, Asked, Vec<Pick>) {
        let ctx = egui::Context::default();
        let _ = pass(&ctx, state, Vec::new());
        let (mut actions, mut all, mut picks) = (Vec::new(), Asked::default(), Vec::new());
        for x in (3..1700u16).step_by(5) {
            let (asked_for, asked) = click(&ctx, state, pos2(f32::from(x), 12.0));
            for action in asked_for {
                if !actions.contains(&action) {
                    actions.push(action);
                }
            }
            picks.extend(asked.open.filter(|pick| !picks.contains(pick)));
            all.open = all.open.or(asked.open);
            all.about |= asked.about;
            all.shortcuts |= asked.shortcuts;
            all.tags |= asked.tags;
            all.measure |= asked.measure;
        }
        (actions, all, picks)
    }

    #[test]
    fn the_bar_offers_each_thing_once_from_left_to_right() {
        let (actions, asked, picks) = sweep_with_picks(&AppState::default());
        assert_eq!(picks, [Pick::Folder, Pick::File]);
        let preset = |preset| Action::ApplyPreset {
            viewport: 0,
            preset,
        };
        let expected = vec![
            Action::SetLayout(Layout::TwoAcross),
            Action::SetLayout(Layout::TwoByTwo),
            preset(0),
            preset(1),
            preset(2),
            Action::ToggleRadiological,
            Action::TogglePatient,
            Action::SetLinked(false),
            Action::CineToggle { viewport: 0 },
            Action::CineSet {
                viewport: 0,
                fps: Cine::DEFAULT_FPS,
                looping: false,
            },
        ];
        assert_eq!(
            actions, expected,
            "the layout in use is not asked for again"
        );
        assert_eq!(
            asked,
            Asked {
                open: Some(Pick::Folder),
                about: true,
                shortcuts: true,
                tags: true,
                measure: true
            }
        );
    }

    #[test]
    fn the_link_says_what_it_takes_for_granted() {
        let taken = "Linked views take it that the patient did not move between series.";
        assert!(LINK_SAYS.ends_with(taken), "{LINK_SAYS}");
        let mut state = AppState::default();
        state.planes.linked = false;
        let (actions, _) = sweep(&state);
        assert!(actions.contains(&Action::SetLinked(true)), "{actions:?}");
    }

    #[test]
    fn the_window_controls_act_on_the_active_viewport() {
        let state = AppState {
            layout: Layout::TwoByTwo,
            active: 2,
            ..AppState::default()
        };
        let (actions, _) = sweep(&state);
        let layouts = [Layout::One, Layout::TwoAcross].map(Action::SetLayout);
        assert!(
            layouts.iter().all(|layout| actions.contains(layout)),
            "{actions:?}"
        );
        assert!(!actions.contains(&Action::SetLayout(Layout::TwoByTwo)));
        for preset in 0..3 {
            assert!(
                actions.contains(&Action::ApplyPreset {
                    viewport: 2,
                    preset
                }),
                "{actions:?}"
            );
        }
        assert!(
            !actions
                .iter()
                .any(|action| matches!(action, Action::SetWindow { .. })),
            "a click changes no number"
        );
    }
}
