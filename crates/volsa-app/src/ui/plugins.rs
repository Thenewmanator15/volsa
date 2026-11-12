//! What the person sees of plugins they added: the section "Added by you" with an entry for
//! each, the card a folder offered is shown on before it is added, the warning before a run on
//! a scan whose description names another contrast, and the question before a plugin is
//! removed. Every word a plugin's description holds is drawn as plain text, never as a link.
//! The interface redesign moves the section into its Tools tab; until then the Parts tab draws
//! it, and these functions draw nothing of the plugins anywhere else.

use egui::{Id, Modal, RichText, Ui};

use super::theme;
use crate::action::Action;
use crate::plugins::{PluginAction, Plugins};

mod card;
mod entry;
mod warning;

#[cfg(test)]
mod pressed_tests;
#[cfg(test)]
mod tests;

pub use card::plugin_card;
pub use entry::plugin_section;
pub use warning::{contrast_warning, remove_question};

/// The widest a card or a question is, in points.
const WIDEST: f32 = 440.0;

/// Draw what plugins have open over the window: the card of a folder offered, the warning
/// before a run, or the question before one is removed. Gives what was pressed.
pub fn windows(ctx: &egui::Context, plugins: &Plugins) -> Option<Action> {
    let card = plugins
        .offered
        .as_ref()
        .and_then(|card| plugin_card(ctx, card));
    let warned = plugins.warning.as_ref();
    let warning = warned.and_then(|warning| contrast_warning(ctx, warning));
    let removing = plugins.removing.as_deref().and_then(|id| {
        let entry = plugins.entries.iter().find(|entry| entry.id == id);
        let name = entry.map_or_else(|| id.to_owned(), |entry| named(&entry.name, &entry.version));
        remove_question(ctx, &name)
    });
    card.or(warning).or(removing)
}

/// A plugin's name and version, as one name.
fn named(name: &str, version: &str) -> String {
    format!("{name} {version}").trim().to_owned()
}

/// The words for a quiet line.
fn quiet(text: &str) -> RichText {
    RichText::new(text).small().color(theme::QUIET_TEXT)
}

/// One of a plugin's words without the full stop it may end with, so that none is doubled.
fn bare(text: &str) -> &str {
    text.trim().trim_end_matches('.')
}

/// Who made a plugin, where it came from and its licence, in a sentence each, as text.
fn origin(made_by: &str, source: &str, licence: &str) -> String {
    format!(
        "Made by {}. Source: {}. Licence: {}.",
        bare(made_by),
        bare(source),
        bare(licence)
    )
}

/// A question over the window, with `title` at its top, `body` beneath, and two buttons:
/// `yes`, which gives `on_yes`, and Cancel. A click outside it, or Escape, is Cancel too.
fn question(
    ctx: &egui::Context,
    title: &str,
    body: impl FnOnce(&mut Ui),
    yes: &str,
    on_yes: PluginAction,
) -> Option<Action> {
    let widest = (ctx.content_rect().width() - 48.0).clamp(200.0, WIDEST);
    let shown = Modal::new(Id::new(("plugin question", title))).show(ctx, |ui| {
        ui.set_width(widest);
        ui.heading(title);
        body(ui);
        ui.separator();
        ui.horizontal(|ui| {
            let yes = ui.button(yes).clicked().then_some(on_yes);
            let no = ui.button("Cancel").clicked();
            yes.or(no.then_some(PluginAction::Cancel))
        })
        .inner
    });
    let closed = shown.should_close().then_some(PluginAction::Cancel);
    shown.inner.or(closed).map(Action::Plugins)
}
