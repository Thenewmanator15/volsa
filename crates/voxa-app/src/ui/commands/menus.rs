//! The buttons that open a menu. They are not actions, so they are not in the list of
//! actions and search does not find them, but their names and hints are written once, here,
//! and every menu button is drawn through [`menu_button`] or named through [`named`].

use egui::{InnerResponse, Response, Ui};

/// One button that opens a menu.
#[derive(Debug, Clone, Copy)]
pub struct Menu {
    /// What a screen reader calls the button, in plain words.
    pub name: &'static str,
    /// What its button shows; the interface size menu adds the size now.
    pub label: &'static str,
    /// More about it, shown under its name when the pointer rests on its button.
    pub explains: &'static str,
}

/// The menu that chooses what to open.
pub const OPEN: Menu = Menu {
    name: "Open",
    label: "Open",
    explains: "Open a folder of DICOM files, a zip file or a DICOMDIR",
};
/// The menu at the right of the top bar.
pub const MORE: Menu = Menu {
    name: "More",
    label: "⋯",
    explains: "The size of the interface, full screen, the keys and mouse, where the log is, and About",
};
/// The menu that chooses the size of the interface.
pub const SIZE: Menu = Menu {
    name: "Interface size",
    label: "Interface size",
    explains: "How large the interface is drawn: its words, buttons and lists. Ctrl+= and Ctrl+- \
               make it larger and smaller, and Ctrl+0 puts it back to its usual size.",
};
/// The menu of the saved views of the 3D scene.
pub const VIEWS: Menu = Menu {
    name: "Views",
    label: "Views",
    explains: "Save the scene as it is now, and go back to a saved view",
};
/// The menu that saves the 3D view.
pub const EXPORT: Menu = Menu {
    name: "Export",
    label: "Export",
    explains: "Save the 3D view as a picture or as a turntable",
};
/// The menu beside the button that labels the scan.
pub const OPTIONS: Menu = Menu {
    name: "Labelling options",
    label: "Options",
    explains: "Which model, whether the cortex is divided into its areas, where the run goes, \
               whether the result is kept, and where the model and runtime files are",
};

/// Every menu button, for the test that walks the window and names each by this table.
#[cfg(test)]
pub const MENUS: [Menu; 6] = [OPEN, MORE, SIZE, VIEWS, EXPORT, OPTIONS];

impl Menu {
    /// What a screen reader calls the button when it writes `written` on itself: the menu's name
    /// when that holds the words (whatever their case), so that a person who speaks the words
    /// they can see is understood (WCAG 2.5.3, label in name); the words when they hold the
    /// name, as `Interface size 125%`; otherwise the words, then the name. A mark with no
    /// letter or digit in it is a picture, and the name stands alone.
    pub fn spoken(&self, written: &str) -> String {
        let written = written.trim();
        let words = written.chars().any(char::is_alphanumeric);
        let (name, shown) = (self.name.to_lowercase(), written.to_lowercase());
        if !words || name.contains(&shown) {
            self.name.to_owned()
        } else if shown.contains(&name) {
            written.to_owned()
        } else {
            format!("{written}, {}", self.name)
        }
    }

    /// What the button says when the pointer rests on it: its name, and beneath it what explains
    /// it.
    pub fn hint(&self) -> String {
        if self.explains.is_empty() {
            self.name.to_owned()
        } else {
            format!("{}\n{}", self.name, self.explains)
        }
    }
}

/// Name the button `response` of the menu `menu` for a screen reader, and give it its hint.
/// `written` is what the button writes on itself. Gives the response with the hint added.
pub fn named(ui: &Ui, menu: &Menu, written: &str, response: Response) -> Response {
    let said = menu.spoken(written);
    ui.ctx()
        .accesskit_node_builder(response.id, |node| node.set_label(said));
    response.on_hover_text(menu.hint())
}

/// Draw the button that opens the menu `menu`, writing `written` on itself, and the menu while
/// it is open, holding what `contents` draws. Gives what `contents` gave while the menu is
/// open. Inside another menu it opens a sub-menu.
pub fn menu_button<R>(
    ui: &mut Ui,
    menu: &Menu,
    written: &str,
    contents: impl FnOnce(&mut Ui) -> R,
) -> InnerResponse<Option<R>> {
    let shown = ui.menu_button(written, contents);
    InnerResponse::new(shown.inner, named(ui, menu, written, shown.response))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_menu_has_a_name_a_label_and_a_hint_and_no_two_share_a_name() {
        let mut names = std::collections::HashSet::new();
        for menu in MENUS {
            assert!(!menu.name.is_empty() && !menu.label.is_empty(), "{menu:?}");
            assert!(
                !menu.explains.is_empty(),
                "{} says nothing of itself",
                menu.name
            );
            assert!(names.insert(menu.name), "{} twice", menu.name);
        }
    }

    #[test]
    fn a_screen_reader_calls_a_menu_button_by_a_name_that_holds_the_words_it_writes() {
        // The name holds the words: the name alone, whatever the case.
        assert_eq!(OPEN.spoken("Open"), "Open");
        assert_eq!(OPTIONS.spoken("options"), "Labelling options");
        // The words hold the name: the words, which say more, as the size now.
        assert_eq!(SIZE.spoken("Interface size 125%"), "Interface size 125%");
        // A mark that is no word is a picture, and the name stands alone.
        assert_eq!(MORE.spoken("⋯"), "More");
        assert_eq!(MORE.spoken(""), "More");
        // Neither holds the other: the words, then the name.
        assert_eq!(EXPORT.spoken("Save"), "Save, Export");
    }

    #[test]
    fn a_hint_is_the_name_and_beneath_it_what_explains_it() {
        let hint = VIEWS.hint();
        let (name, explains) = hint.split_once('\n').expect("a second line");
        assert_eq!((name, explains), (VIEWS.name, VIEWS.explains));
        let plain = Menu {
            explains: "",
            ..VIEWS
        };
        assert_eq!(plain.hint(), "Views");
    }
}
