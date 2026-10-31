//! A pretend hand for the tests of the interface: it draws passes of something in a window
//! of its own, reads what was drawn and what a screen reader is told, and clicks on it.

use egui::{Context, Event, Modifiers, PointerButton, Pos2, RawInput, Rect, Ui, Vec2, pos2, vec2};

use super::theme;

/// What one pass drew.
#[derive(Debug, Default)]
pub struct Seen {
    /// Every piece of text, in the order it was painted.
    pub words: Vec<String>,
    /// What a screen reader is told of each widget that has a name, and where it is.
    pub names: Vec<(String, Rect)>,
}

impl Seen {
    /// Where the widget a screen reader calls `name` is, if one is.
    pub fn bounds_of(&self, name: &str) -> Option<Rect> {
        let named = self.names.iter().find(|(one, _)| one == name);
        named.map(|(_, bounds)| *bounds)
    }

    /// The names given to widgets, in the order they were told.
    pub fn named(&self) -> Vec<&str> {
        self.names.iter().map(|(name, _)| name.as_str()).collect()
    }
}

/// A window of a fixed size, and the pointer in it.
pub struct Hand {
    ctx: Context,
    size: Vec2,
}

/// The words written by a shape and the shapes within it.
fn words_of(shape: &egui::Shape, into: &mut Vec<String>) {
    match shape {
        egui::Shape::Text(text) => into.push(text.galley.text().to_owned()),
        egui::Shape::Vec(shapes) => shapes.iter().for_each(|shape| words_of(shape, into)),
        _ => {}
    }
}

/// A press or a release of the main button at `at`.
fn button(at: Pos2, pressed: bool) -> Event {
    Event::PointerButton {
        pos: at,
        button: PointerButton::Primary,
        pressed,
        modifiers: Modifiers::NONE,
    }
}

impl Hand {
    /// A window `wide` by `high` points, with Voxa's look and a screen reader listening.
    pub fn new(wide: f32, high: f32) -> Self {
        let ctx = Context::default();
        theme::apply(&ctx);
        ctx.enable_accesskit();
        Self {
            ctx,
            size: vec2(wide, high),
        }
    }

    /// One pass of `draw` with these events, and what it drew.
    pub fn pass(&self, events: Vec<Event>, draw: &mut impl FnMut(&mut Ui)) -> Seen {
        let input = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, self.size)),
            events,
            ..RawInput::default()
        };
        let mut output = self.ctx.run_ui(input, |ui| draw(ui));
        // No renderer is behind these passes to take the font texture.
        output.textures_delta.clear();
        let mut seen = Seen::default();
        for clipped in &output.shapes {
            words_of(&clipped.shape, &mut seen.words);
        }
        let update = output.platform_output.accesskit_update.into_iter();
        for (_, node) in update.flat_map(|update| update.nodes) {
            let (Some(name), Some(at)) = (node.label(), node.bounds()) else {
                continue;
            };
            #[expect(
                clippy::cast_possible_truncation,
                reason = "a widget is far smaller than the points an f32 can count"
            )]
            let rect = Rect::from_min_max(
                pos2(at.x0 as f32, at.y0 as f32),
                pos2(at.x1 as f32, at.y1 as f32),
            );
            seen.names.push((name.to_owned(), rect));
        }
        seen
    }

    /// Draw twice with nothing happening, for the panels and wrapped lines to find their
    /// sizes, and say what the second drew.
    pub fn settle(&self, draw: &mut impl FnMut(&mut Ui)) -> Seen {
        let _ = self.pass(Vec::new(), draw);
        self.pass(Vec::new(), draw)
    }

    /// Press and let go of the main button at `at`: the press in one pass and the release in
    /// the next, in which the click is seen.
    pub fn click(&self, at: Pos2, draw: &mut impl FnMut(&mut Ui)) {
        let _ = self.pass(vec![Event::PointerMoved(at), button(at, true)], draw);
        let _ = self.pass(vec![button(at, false)], draw);
    }

    /// Click on the widget a screen reader calls `name`. False if there is none.
    pub fn click_named(&self, name: &str, draw: &mut impl FnMut(&mut Ui)) -> bool {
        let seen = self.settle(draw);
        let Some(bounds) = seen.bounds_of(name) else {
            return false;
        };
        self.click(bounds.center(), draw);
        true
    }

    /// The widgets that can be clicked now, in the order they were drawn.
    fn clickable(&self) -> Vec<egui::Id> {
        let widgets = self
            .ctx
            .viewport(|viewport| viewport.prev_pass.widgets.clone());
        let in_layers = widgets.layers().flat_map(|(_, widgets)| widgets.iter());
        let clickable = in_layers.filter(|widget| widget.enabled && widget.sense.senses_click());
        clickable.map(|widget| widget.id).collect()
    }

    /// Where a widget was drawn in the last pass, if it was.
    fn centre_of(&self, id: egui::Id) -> Option<Pos2> {
        let widgets = self
            .ctx
            .viewport(|viewport| viewport.prev_pass.widgets.clone());
        widgets.get(id).map(|widget| widget.interact_rect.center())
    }

    /// Click on every button of what `draw` draws, once each, in the order they were drawn.
    /// What opens when one is clicked, such as a menu, is not clicked in.
    pub fn click_each(&self, draw: &mut impl FnMut(&mut Ui)) {
        let _ = self.settle(draw);
        for id in self.clickable() {
            if let Some(at) = self.centre_of(id) {
                self.click(at, draw);
            }
        }
    }
}
