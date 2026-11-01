//! What a pass of the interface drew and told a screen reader, for the tests of the bars:
//! every piece of text with where it is, every name with where it is, the colours drawn, and
//! the clicks that find a button by its name.

use egui::epaint::ColorMode;
use egui::epaint::text::ByteRangeExt as _;
use egui::{Color32, Context, Event, Modifiers, PointerButton, Pos2, RawInput, Rect, Shape, vec2};

use super::theme::{self, ThemeChoice};

/// What one pass drew.
#[derive(Default)]
pub struct Seen {
    /// Each piece of text drawn, with where it is.
    pub words: Vec<(String, Rect)>,
    /// What a screen reader is called each thing, with where it is.
    pub names: Vec<(String, Rect)>,
    /// The colour of every piece of text, line, fill and outline drawn.
    pub colours: Vec<Color32>,
    /// The colour of every piece of text drawn, with the text.
    pub inks: Vec<(String, Color32)>,
    /// Each picture drawn from a texture, by its texture, with where it is.
    pub images: Vec<(egui::TextureId, Rect)>,
    /// What was drawn of each piece of text cut short to fit, with the ellipsis that ends it.
    pub cut: Vec<String>,
}

impl Seen {
    /// Where the thing a screen reader calls `name` is.
    pub fn place_of(&self, name: &str) -> Option<Rect> {
        self.names
            .iter()
            .find_map(|(said, rect)| (said == name).then_some(*rect))
    }

    /// Where the thing a screen reader calls something that begins with `start` is.
    pub fn place_starting(&self, start: &str) -> Option<Rect> {
        self.names
            .iter()
            .find_map(|(said, rect)| said.starts_with(start).then_some(*rect))
    }

    /// The places of the things a screen reader calls something that begins with `start`.
    pub fn places_starting(&self, start: &str) -> Vec<Rect> {
        let named = self.names.iter();
        named
            .filter(|(said, _)| said.starts_with(start))
            .map(|(_, rect)| *rect)
            .collect()
    }

    /// Whether the pass drew something in this colour, give or take a shade, however much of it
    /// showed through: a window that is fading in is drawn part transparent.
    pub fn drew(&self, colour: Color32) -> bool {
        let [red, green, blue, _] = colour.to_srgba_unmultiplied();
        let near = |a: u8, b: u8| a.abs_diff(b) <= 2;
        self.colours.iter().any(|drawn| {
            let [r, g, b, alpha] = drawn.to_srgba_unmultiplied();
            alpha > 16 && near(r, red) && near(g, green) && near(b, blue)
        })
    }

    /// The text drawn in a colour that is too faint for text on `ground`, by the WCAG contrast
    /// level AA asks of text: 4.5 to 1. `except` are words drawn on a ground of their own. A
    /// window that is fading in draws its text part transparent; it is judged by its colour
    /// once it has faded in.
    pub fn hard_to_read_on(&self, ground: Color32, except: &[&str]) -> Vec<(String, Color32)> {
        let opaque = |ink: Color32| {
            let [red, green, blue, _] = ink.to_srgba_unmultiplied();
            Color32::from_rgb(red, green, blue)
        };
        let faint = self.inks.iter().filter(|(said, ink)| {
            !except.contains(&said.as_str()) && theme::contrast(opaque(*ink), ground) < 4.5
        });
        faint.cloned().collect()
    }
}

/// A context drawn in Voxa's dark look at this interface size, with a screen reader's account
/// of every pass.
pub fn context(zoom: f32) -> Context {
    context_in(zoom, ThemeChoice::Dark)
}

/// A context drawn in this theme at this interface size, with a screen reader's account of
/// every pass.
pub fn context_in(zoom: f32, choice: ThemeChoice) -> Context {
    let ctx = Context::default();
    theme::apply(&ctx, choice);
    ctx.options_mut(|options| options.zoom_factor = zoom);
    ctx.enable_accesskit();
    ctx
}

/// Run one pass of `run` in a window `size` points across, with these events; gives what it
/// returned and what it drew.
pub fn pass<R>(
    ctx: &Context,
    size: [f32; 2],
    events: Vec<Event>,
    run: impl FnOnce(&mut egui::Ui) -> R,
) -> (R, Seen) {
    let screen = Rect::from_min_size(Pos2::ZERO, vec2(size[0], size[1]));
    let input = RawInput {
        screen_rect: Some(screen),
        events,
        ..RawInput::default()
    };
    let (mut given, mut run) = (None, Some(run));
    let mut output = ctx.run_ui(input, |ui| {
        if let Some(run) = run.take() {
            given = Some(run(ui));
        }
    });
    // No renderer is behind these passes to take the font texture.
    output.textures_delta.clear();
    let mut seen = Seen::default();
    for clipped in &output.shapes {
        take_in(&clipped.shape, &mut seen);
    }
    let update = output.platform_output.accesskit_update.into_iter();
    for (_, node) in update.flat_map(|update| update.nodes) {
        if let (Some(said), Some(bounds)) = (node.label(), node.bounds()) {
            let corner = |x: f64, y: f64| Pos2::new(narrowed(x), narrowed(y));
            let rect =
                Rect::from_min_max(corner(bounds.x0, bounds.y0), corner(bounds.x1, bounds.y1));
            seen.names.push((said.to_owned(), rect));
        }
    }
    (given.expect("the pass ran"), seen)
}

/// A place in a window as a screen reader is told it, in the interface's points.
#[expect(
    clippy::cast_possible_truncation,
    reason = "a place in a window of a usual size, far inside what an f32 holds"
)]
fn narrowed(at: f64) -> f32 {
    at as f32
}

/// Note the words and colours of a shape, and of the shapes within it.
fn take_in(shape: &Shape, seen: &mut Seen) {
    match shape {
        Shape::Text(text) => {
            seen.words
                .push((text.galley.text().to_owned(), text.visual_bounding_rect()));
            if text.galley.elided {
                let rows = text.galley.rows.iter();
                seen.cut
                    .push(rows.map(|placed| placed.row.text()).collect());
            }
            for section in &text.galley.job.sections {
                let own = section.format.color;
                let drawn = if own == Color32::PLACEHOLDER {
                    text.fallback_color
                } else {
                    own
                };
                let ink = text.override_text_color.unwrap_or(drawn);
                seen.colours.push(ink);
                let words = section.byte_range.slice(text.galley.text()).to_owned();
                seen.inks.push((words, ink));
            }
        }
        Shape::Vec(shapes) => shapes.iter().for_each(|shape| take_in(shape, seen)),
        Shape::Mesh(mesh) => seen.images.push((mesh.texture_id, mesh.calc_bounds())),
        Shape::LineSegment { stroke, .. } => seen.colours.push(stroke.color),
        Shape::Path(path) => {
            seen.colours.push(path.fill);
            if let ColorMode::Solid(colour) = path.stroke.color {
                seen.colours.push(colour);
            }
        }
        Shape::Circle(circle) => {
            seen.colours.push(circle.fill);
            seen.colours.push(circle.stroke.color);
        }
        Shape::Rect(rect) => {
            seen.colours.push(rect.fill);
            seen.colours.push(rect.stroke.color);
        }
        _ => {}
    }
}

/// The two passes' events of pressing and letting go of the main button at a point.
pub fn click_at(at: Pos2) -> [Vec<Event>; 2] {
    let button = |pressed| Event::PointerButton {
        pos: at,
        button: PointerButton::Primary,
        pressed,
        modifiers: Modifiers::NONE,
    };
    [
        vec![Event::PointerMoved(at), button(true)],
        vec![button(false)],
    ]
}

/// A key pressed with nothing held.
pub fn key(key: egui::Key) -> Event {
    Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::NONE,
    }
}

/// Whether a colour is red: much redder than it is green and blue.
pub fn is_red(colour: Color32) -> bool {
    let [r, g, b, a] = colour.to_array();
    a > 0 && r > 150 && u16::from(g) * 2 < u16::from(r) && u16::from(b) * 2 < u16::from(r)
}
