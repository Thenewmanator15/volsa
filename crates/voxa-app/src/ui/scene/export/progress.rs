//! The window that shows how far a turntable has got, with a way to stop it. It floats over the
//! pictures like the other windows that float (see [`crate::ui::floating`]), and Esc stops the
//! turntable, as the Cancel button does.

use egui::{Context, Id};

use super::Turntable;
use crate::ui::floating;

/// The window's own name, which it keeps its place under.
const WINDOW: &str = "voxa: saving a turntable";
/// Where the number of the frame the window was last drawn in is kept.
const DRAWN: &str = "voxa: the frame the turntable window was last drawn in";
/// Where the number of the frame Esc asked for the turntable to stop in is kept.
const STOPPED: &str = "voxa: the frame the turntable was asked to stop in";

/// The window's identifier.
pub fn id() -> Id {
    Id::new(WINDOW)
}

/// Whether the window is open: it was drawn in the frame before this one, or in this one.
pub fn is_open(ctx: &Context) -> bool {
    let drawn = ctx.data(|data| data.get_temp::<u64>(Id::new(DRAWN)));
    drawn.is_some_and(|frame| frame + 1 >= ctx.cumulative_frame_nr())
}

/// Ask for the turntable to stop, which Esc does when the window is the one on top. It is heard
/// by [`progress`] in the frame it is asked in.
pub fn stop(ctx: &Context) {
    let now = ctx.cumulative_frame_nr();
    ctx.data_mut(|data| data.insert_temp(Id::new(STOPPED), now));
}

/// Show how far a turntable has got, with a way to stop it, over the pictures. True when the
/// person stops it, with the button or with Esc.
pub fn progress(ctx: &Context, making: &Turntable) -> bool {
    let (done, of) = making.progress();
    let mut stop =
        ctx.data(|data| data.get_temp::<u64>(Id::new(STOPPED))) == Some(ctx.cumulative_frame_nr());
    let window = egui::Window::new("Saving a turntable")
        .id(id())
        .collapsible(false)
        .resizable(false);
    floating::over(window, ctx).show(ctx, |ui| {
        ui.label(format!("Drawing picture {} of {of}", (done + 1).min(of)));
        let share = if of == 0 {
            1.0
        } else {
            done as f32 / of as f32
        };
        ui.add(egui::ProgressBar::new(share).desired_width(240.0));
        stop |= ui.button("Cancel").clicked();
    });
    let now = ctx.cumulative_frame_nr();
    ctx.data_mut(|data| {
        if stop {
            data.remove::<u64>(Id::new(DRAWN));
        } else {
            data.insert_temp(Id::new(DRAWN), now);
        }
    });
    stop
}

#[cfg(test)]
mod tests;
