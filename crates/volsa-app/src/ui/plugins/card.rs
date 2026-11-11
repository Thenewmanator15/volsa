//! The card a folder offered is shown on before it is added: Volsa's own words first, in bold;
//! then what the plugin says of itself, as text; every setting Volsa will prepare a scan with
//! for it; and Add or Cancel.

use egui::{RichText, ScrollArea, Ui};

use super::{named, origin, question, quiet};
use crate::action::Action;
use crate::plugins::{OfferCard, PluginAction};
use crate::ui::theme;

/// What the card says first.
pub const CARD: &str = "Added by you, not part of Volsa. Volsa has not checked what this model \
                        does. Not for diagnostic use.";
/// The share of the window's height the plugin's words may take before they scroll.
const TALLEST: f32 = 0.5;

/// Draw the card of a folder offered. Gives Add or Cancel when pressed.
pub fn plugin_card(ctx: &egui::Context, card: &OfferCard) -> Option<Action> {
    let tallest = ctx.content_rect().height() * TALLEST;
    let body = |ui: &mut Ui| {
        ui.label(RichText::new(CARD).strong());
        ui.add_space(4.0);
        let scrolled = ScrollArea::vertical().max_height(tallest);
        scrolled.show(ui, |ui| words(ui, card));
    };
    question(ctx, "Add a plugin", body, "Add", PluginAction::Add)
}

/// What the plugin says of itself, then how Volsa will prepare a scan for it.
fn words(ui: &mut Ui, card: &OfferCard) {
    let entry = &card.entry;
    ui.label(RichText::new(named(&entry.name, &entry.version)).strong());
    ui.label(&entry.about);
    for limit in &entry.limits {
        ui.label(limit);
    }
    ui.label(quiet(&origin(
        &entry.made_by,
        &entry.source,
        &entry.licence,
    )));
    ui.label(quiet(&format!("For {}.", entry.accepts_words)));
    ui.label(quiet(&entry.returns));
    ui.add_space(4.0);
    ui.label(
        RichText::new("How Volsa prepares a scan for it")
            .small()
            .strong(),
    );
    for line in &card.settings {
        ui.label(quiet(line));
    }
    if let Some(old) = &card.replaces {
        let replaces = format!("This replaces version {old} of this plugin.");
        ui.label(RichText::new(replaces).color(theme::WARNING));
    }
}
