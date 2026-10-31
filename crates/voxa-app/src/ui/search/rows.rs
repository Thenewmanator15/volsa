//! How search draws its results: a title for each group, and a row for each result.

use egui::text::{LayoutJob, TextFormat};
use egui::{Button, Response, RichText, Stroke, TextStyle, Ui, vec2};

use super::entries::{Entry, Heading, Target};
use super::run;
use crate::state::AppState;
use crate::ui::{commands, theme};

/// Draw the title of a group of results.
pub fn heading(ui: &mut Ui, heading: Heading) {
    ui.label(
        RichText::new(heading.title())
            .small()
            .strong()
            .color(theme::QUIET_TEXT),
    );
}

/// The line beneath a result's name, if it has one: why an action cannot be used now, a part's
/// group, a scan's own description.
fn beneath(entry: &Entry, state: &AppState, why_not: Option<&str>) -> Option<String> {
    let text = match &entry.target {
        Target::Command(_) => why_not.map(str::to_owned),
        Target::Part(_) => entry.words.first().cloned(),
        Target::Scan(id) => {
            let listed = state.series(id)?;
            Some(listed.info.description.clone())
        }
        Target::Help(_) => None,
    };
    text.filter(|text| !text.trim().is_empty())
}

/// Draw one result: its name, with a line beneath it where there is one, and the key of an
/// action beside it. An action that cannot be used now is drawn grey, with the reason
/// beneath; it can still be chosen, which says why.
pub fn row(ui: &mut Ui, entry: &Entry, state: &AppState, chosen: bool) -> Response {
    let command = match &entry.target {
        Target::Command(id) => commands::find(id),
        _ => None,
    };
    let why_not = command.and_then(|command| run::unavailable(command, state));
    let mut job = LayoutJob::default();
    let colour = if why_not.is_some() {
        theme::QUIET_TEXT
    } else {
        ui.visuals().text_color()
    };
    let name = TextFormat::simple(TextStyle::Body.resolve(ui.style()), colour);
    job.append(&entry.name, 0.0, name);
    if let Some(line) = beneath(entry, state, why_not.as_deref()) {
        let small = TextStyle::Small.resolve(ui.style());
        let format = TextFormat::simple(small, theme::QUIET_TEXT);
        job.append(&format!("\n{line}"), 0.0, format);
    }
    let mut button = Button::new(job)
        .wrap()
        .min_size(vec2(ui.available_width(), 0.0))
        .frame_when_inactive(chosen)
        .fill(theme::RAISED)
        .stroke(Stroke::new(if chosen { 1.0 } else { 0.0 }, theme::ACCENT));
    if let Some(key) = command.and_then(|command| command.key) {
        button = button.right_text(
            RichText::new(key.shown)
                .monospace()
                .color(theme::QUIET_TEXT),
        );
    }
    let response = ui.add(button);
    match command {
        Some(command) => response.on_hover_text(commands::hint(command)),
        None => response,
    }
}
