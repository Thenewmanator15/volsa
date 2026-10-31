//! The bars laid out in the smallest window with the interface at twice its usual size.

use egui::{Id, Pos2, RawInput, Rect, WidgetRect, WidgetRects, vec2};

use super::{Kept, show};
use crate::prefs::SMALLEST_WINDOW;
use crate::state::{AppState, Notice, NoticeKind};
use crate::ui::search::Search;
use crate::ui::theme;

/// What was laid out: where the disclaimer is and its id, where the pictures go, and every
/// widget of the pass.
struct Laid {
    disclaimer: Rect,
    id: Id,
    pictures: Rect,
    widgets: WidgetRects,
}

/// Lay the frame out in `screen` at interface size `zoom`, with a long notice on the bottom
/// line, and the pictures in what is left, as the window does: a few passes, for the panels
/// to take their sizes.
fn laid_out(screen: Rect, zoom: f32) -> Laid {
    let ctx = egui::Context::default();
    theme::apply(&ctx);
    ctx.options_mut(|options| options.zoom_factor = zoom);
    let mut state = AppState::default();
    state.notices.push(Notice {
        kind: NoticeKind::Warning,
        text: concat!(
            "A notice long enough to run across the whole of a narrow window and on past ",
            "the disclaimer, which it must not cover"
        )
        .to_owned(),
        details: Vec::new(),
        key: None,
    });
    let mut laid = (Rect::NOTHING, Id::NULL, Rect::NOTHING);
    let (mut search, mut kept) = (Search::default(), Kept::default());
    for _ in 0..4 {
        let input = RawInput {
            screen_rect: Some(screen),
            ..RawInput::default()
        };
        let mut output = ctx.run_ui(input, |ui| {
            let mut actions = Vec::new();
            let (_, disclaimer) = show(ui, &state, &mut search, &mut kept, &mut actions);
            let pictures = egui::CentralPanel::default().show(ui, |ui| {
                ui.allocate_space(ui.available_size());
            });
            laid = (disclaimer.rect, disclaimer.id, pictures.response.rect);
        });
        // No renderer is behind these passes to take the font texture.
        output.textures_delta.clear();
    }
    let widgets = ctx.viewport(|viewport| viewport.prev_pass.widgets.clone());
    let (disclaimer, id, pictures) = laid;
    Laid {
        disclaimer,
        id,
        pictures,
        widgets,
    }
}

/// Whether `widget` is a part of the window that holds `rect` as it is laid out, not a thing
/// drawn over it: a region that takes no clicks or drags and holds all of `rect`.
fn holds(widget: &WidgetRect, rect: Rect) -> bool {
    !widget.sense.interactive() && widget.interact_rect.contains_rect(rect)
}

#[test]
fn the_disclaimer_is_whole_and_uncovered_in_the_smallest_window_at_twice_the_size() {
    // The smallest window, 640 by 480, holds 320 by 240 points of an interface drawn at 200%.
    let [wide, high] = SMALLEST_WINDOW;
    let screen = Rect::from_min_size(Pos2::ZERO, vec2(wide, high) / 2.0);
    let laid = laid_out(screen, 2.0);
    assert!(
        laid.disclaimer.width() > 0.0 && screen.contains_rect(laid.disclaimer),
        "the disclaimer at {:?} is not inside the window",
        laid.disclaimer
    );
    let shown = laid.widgets.get(laid.id).map(|widget| widget.interact_rect);
    assert_eq!(shown, Some(laid.disclaimer), "none of it is cut off");
    for (_, widgets) in laid.widgets.layers() {
        for widget in widgets {
            if widget.id == laid.id || holds(widget, laid.disclaimer) {
                continue;
            }
            let over = widget.interact_rect.intersect(laid.disclaimer);
            assert!(
                !over.is_positive(),
                "a widget at {:?} is over the disclaimer at {:?}",
                widget.interact_rect,
                laid.disclaimer
            );
        }
    }
    // And the bar across the top leaves room for the pictures.
    assert!(
        laid.pictures.height() > 0.3 * screen.height(),
        "the pictures have {:?} of the window",
        laid.pictures
    );
}
