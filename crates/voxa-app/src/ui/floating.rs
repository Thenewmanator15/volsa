//! Windows that float over the pictures: About, the help and a file's tags.
//!
//! Nothing floats over the column of scans. Each of these windows opens inside the area of
//! the pictures and is kept there when it is moved, never wider or taller than that area; and
//! Esc closes the one on top.

use egui::{Context, Id, Key, Modifiers, Rect, Window, vec2};

use super::{VoxaApp, about, help, search};
use crate::action::Action;

/// Where the area of the pictures is, which the windows are told each frame.
const AREA: &str = "voxa: where the pictures are, for the windows that float over them";
/// How far inside the area a window starts from its top and left edges, in points.
const INSET: f32 = 12.0;
/// Where the windows that are open are kept, in the order they were opened.
const OPENED: &str = "voxa: the floating windows that are open, oldest first";

/// Tell the floating windows where the pictures are, once the pictures have their place in a
/// frame and before the windows are shown.
pub fn keep_to(ctx: &Context, pictures: Rect) {
    ctx.data_mut(|data| data.insert_temp(Id::new(AREA), pictures));
}

/// The area the floating windows keep to: the pictures', once the window has said where they
/// are, and until then the whole window.
pub fn area(ctx: &Context) -> Rect {
    let told = ctx.data(|data| data.get_temp::<Rect>(Id::new(AREA)));
    told.unwrap_or_else(|| ctx.content_rect())
}

/// `window` kept to the area of the pictures: starting inside it, moved only within it, and
/// never larger than it.
pub fn over<'a>(window: Window<'a>, ctx: &Context) -> Window<'a> {
    let inside = area(ctx);
    window
        .constrain_to(inside)
        .max_size((inside.size() - vec2(INSET, INSET)).max(vec2(0.0, 0.0)))
        .default_pos(inside.min + vec2(INSET, INSET))
}

/// Whether each of the floating windows is open.
pub struct Open<'a> {
    /// The About window.
    pub about: &'a mut bool,
    /// The help: its topics, the keys and what the mouse does.
    pub help: &'a mut bool,
    /// The tags of a file.
    pub tags: &'a mut bool,
}

/// The floating windows, which Esc closes one at a time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Which {
    About,
    /// The licences and notices window, which About opens.
    Licences,
    Help,
    Tags,
}

impl Open<'_> {
    /// Whether this window is open.
    fn is_open(&self, ctx: &Context, which: Which) -> bool {
        match which {
            Which::About => *self.about,
            Which::Licences => about::notices_are_open(ctx),
            Which::Help => *self.help,
            Which::Tags => *self.tags,
        }
    }

    /// Close this window.
    fn close(&mut self, ctx: &Context, which: Which) {
        match which {
            Which::About => *self.about = false,
            Which::Licences => about::close_notices(ctx),
            Which::Help => *self.help = false,
            Which::Tags => *self.tags = false,
        }
    }
}

/// Close the floating window opened last when Esc is pressed, and take the key so that it does
/// nothing else. Esc is left alone while a text field or the search box has the keyboard,
/// which have their own use for it. Called every frame, to note which windows are open.
pub fn close_on_esc(ctx: &Context, mut open: Open<'_>) {
    let kept = Id::new(OPENED);
    let mut order: Vec<Which> = ctx.data(|data| data.get_temp(kept)).unwrap_or_default();
    order.retain(|which| open.is_open(ctx, *which));
    for which in [Which::About, Which::Licences, Which::Help, Which::Tags] {
        if open.is_open(ctx, which) && !order.contains(&which) {
            order.push(which);
        }
    }
    let typing = ctx.egui_wants_keyboard_input() || search::holds_keyboard(ctx);
    let escaped = !typing && ctx.input(|input| input.key_pressed(Key::Escape));
    if let Some(last) = order.last().copied().filter(|_| escaped) {
        open.close(ctx, last);
        order.pop();
        ctx.input_mut(|input| input.consume_key(Modifiers::NONE, Key::Escape));
    }
    ctx.data_mut(|data| data.insert_temp(kept, order));
}

impl VoxaApp {
    /// Show the windows that float over the pictures, which are in `pictures`, once the
    /// pictures have their place in the frame. Esc closes the one on top.
    pub(super) fn windows(&mut self, ctx: &Context, pictures: Rect, actions: &mut Vec<Action>) {
        keep_to(ctx, pictures);
        let open = Open {
            about: &mut self.about_open,
            help: &mut self.help_open,
            tags: &mut self.tags.open,
        };
        close_on_esc(ctx, open);
        about::show(ctx, &mut self.about_open);
        help::show(ctx, &mut self.help_open);
        self.tags.show(ctx, self.session.state(), actions);
    }
}

#[cfg(test)]
mod tests;
