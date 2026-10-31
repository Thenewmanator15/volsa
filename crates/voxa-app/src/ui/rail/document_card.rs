//! The card of a document in the rail: a page, what kind of document it is, its title and the
//! date it was made. A click opens it in the middle of the window; its menu opens it or shows
//! the tags of its file. A document's card never shows a picture of it, for it holds none.
//!
//! The words on a card are cut short with an ellipsis to fit; resting the pointer on it shows
//! them whole. Words from the file are drawn without the characters that would turn, hide or
//! break them.

use egui::{
    Button, Color32, CornerRadius, Id, Popup, Rect, Sense, Ui, WidgetInfo, WidgetType, pos2, vec2,
};
use voxa_dicom::IndexedDocument;

use super::card::{self, Asks, DOTS, PADDING};
use super::walk;
use crate::action::Action;
use crate::documents::{Mask, kind_words};
use crate::naming::printable;
use crate::ui::browser::{THUMBNAIL, date_in_words};
use crate::ui::commands::{self, Request, TagsOf};

/// What a card says of a document that gives no title.
pub const NO_TITLE: &str = "No title";
/// What a card says of a document that gives no date.
pub const NO_DATE: &str = "No date";
/// The item of a document card's menu that opens it.
pub const OPEN: &str = "Open";

/// A document's title as a card says it: as the file wrote it but for the characters that would
/// turn or hide text and, while the patient's details are hidden, the patient's name and
/// identifier (see [`Mask`]), or [`NO_TITLE`].
pub fn title_of(document: &IndexedDocument, mask: &Mask) -> String {
    let title = printable(&document.title);
    let title = mask.apply(&title);
    let title = title.trim();
    if title.is_empty() {
        NO_TITLE.to_owned()
    } else {
        title.to_owned()
    }
}

/// A document's date as a card says it, in words, or [`NO_DATE`].
pub fn date_of(document: &IndexedDocument) -> String {
    let date = document.date.as_deref().and_then(date_in_words);
    date.unwrap_or_else(|| NO_DATE.to_owned())
}

/// What a screen reader calls the card of a document: what kind it is, then its title.
pub fn card_name(document: &IndexedDocument, mask: &Mask) -> String {
    format!(
        "{}, {}",
        kind_words(&document.kind),
        title_of(document, mask)
    )
}

/// What resting the pointer on a card shows: everything the card may have had no room for.
pub fn hover_of(document: &IndexedDocument, mask: &Mask) -> String {
    let lines = [
        kind_words(&document.kind),
        title_of(document, mask),
        date_of(document),
    ];
    lines.join("\n")
}

/// Draw the card of a document in `rect`. A click opens it; a click with the other button, or
/// on its `⋯`, opens its menu. `selected` is whether it is the document open. Gives whether the
/// card, or the button of its menu, has the keyboard.
pub fn show(
    ui: &mut Ui,
    rect: Rect,
    document: &IndexedDocument,
    selected: bool,
    asks: &mut Asks<'_>,
) -> bool {
    let mask = Mask::for_document(asks.state, document);
    let name = card_name(document, &mask);
    let id = id_of(document);
    let response = ui.interact(rect, id, Sense::click());
    let said = card::said(&name, selected, false);
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, true, &said));
    let picture = Rect::from_min_size(
        rect.min + vec2(PADDING, PADDING),
        vec2(THUMBNAIL, THUMBNAIL),
    );
    let dots = Rect::from_min_size(
        pos2(rect.right() - 4.0 - DOTS, rect.top() + 4.0),
        vec2(DOTS, DOTS),
    );
    if ui.is_rect_visible(rect) {
        let painter = ui.painter_at(rect);
        let (lit, focused) = (response.hovered(), response.has_focus());
        card::ground(&painter, rect, (lit, focused), selected);
        painter.rect_filled(picture, CornerRadius::same(4), Color32::BLACK);
        card::page(&painter, picture);
        let lines = [
            kind_words(&document.kind),
            title_of(document, &mask),
            date_of(document),
        ];
        let at = picture.right_top() + vec2(PADDING, 0.0);
        let ends = [dots.left() - 2.0, rect.right() - PADDING];
        card::words(ui, &painter, lines, at, ends);
    }
    let dots = card::dots_button(ui, dots, id, &name);
    let response = response.on_hover_text(hover_of(document, &mask));
    if response.clicked() {
        asks.actions
            .push(Action::OpenDocument(document.entry.clone()));
    }
    response.context_menu(|ui| menu(ui, &document.entry, asks));
    Popup::menu(&dots).show(|ui| menu(ui, &document.entry, asks));
    walk::holds_keys(ui, &response, &dots)
}

/// The identifier of the card of `document`.
pub(super) fn id_of(document: &IndexedDocument) -> Id {
    Id::new(("voxa: the card of the document", &document.entry))
}

/// A document card's menu: open it, or show the tags of its file.
fn menu(ui: &mut Ui, entry: &str, asks: &mut Asks<'_>) {
    if ui.button(OPEN).clicked() {
        asks.actions.push(Action::OpenDocument(entry.to_owned()));
        ui.close();
    }
    let tags = commands::button_made(ui, "tags.show", asks.state, "", |command| {
        Button::new(command.name)
    });
    if let Some((_, Some(_))) = tags {
        let of = TagsOf::Entry(entry.to_owned());
        asks.requests.push(Request::Tags(of));
        ui.close();
    }
}
