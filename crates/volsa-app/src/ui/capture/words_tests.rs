//! The words a picture of the window asks for that name a tab of the panel and the windows and
//! lists that open over the pictures.

use super::*;

#[test]
fn a_picture_can_ask_for_any_tab_by_its_own_word() {
    use crate::ui::panel::Tab;
    for (word, tab) in [
        ("tab-view", Tab::View),
        ("tab-measure", Tab::Measure),
        ("tab-3d", Tab::Scene),
        ("tab-labels", Tab::Labels),
        ("tab-tools", Tab::Tools),
    ] {
        assert_eq!(tab_asked(word), Some(tab), "{word}");
        assert_eq!(
            tab_asked(&format!("vr, {word}")),
            Some(tab),
            "{word} among others"
        );
    }
    assert_eq!(tab_asked("tab-"), None);
    assert_eq!(tab_asked("tab-everything"), None);
    // `3d` is the arrangement of the pictures, not a tab.
    assert_eq!(tab_asked("3d"), None);
    // Each tab word, as each of the older ones, names a tab of the panel and no window.
    assert_eq!(
        tab_asked("tab-view, tab-tools"),
        Some(Tab::Tools),
        "the last of those asked for"
    );
}

#[test]
fn a_picture_can_ask_for_the_windows_and_lists_that_open_over_it() {
    use super::tabs::Opens;
    let nothing = Opens::default();
    assert_eq!(opens(""), nothing);
    assert_eq!(opens("vr, tab-tools, light"), nothing);
    assert_eq!(
        opens("tags, keys, about, search, other"),
        Opens {
            tags: true,
            keys: true,
            about: true,
            search: true,
            other: true,
        }
    );
    // Whole words, and only the word asked for.
    assert_eq!(
        opens("search"),
        Opens {
            search: true,
            ..nothing
        }
    );
    assert_eq!(opens("noticed, searching, notices"), nothing);
    // `measure` is a tab of the panel and opens no window.
    assert_eq!(opens("measure"), nothing);
}

#[test]
fn a_picture_can_ask_for_the_list_of_notices_and_it_opens_once_there_is_a_notice() {
    let ask = |words: &str| Capture::asking(PathBuf::new(), words, None);
    // Not asked for: nothing to open, and nothing to wait for.
    let mut plain = ask("vr, search");
    assert!(plain.has_opened_notices());
    assert!(!plain.open_notices(true));
    // Asked for: it waits while there is no notice, as there is none as the window opens.
    let mut wanting = ask("notices");
    assert!(!wanting.has_opened_notices());
    assert!(!wanting.open_notices(false));
    assert!(!wanting.has_opened_notices(), "still waiting");
    // Opened once, as the first notice arrives, and not again.
    assert!(wanting.open_notices(true));
    assert!(wanting.has_opened_notices());
    assert!(!wanting.open_notices(true), "once");
    // A word is read whole.
    assert!(ask("noticed").has_opened_notices());
}
