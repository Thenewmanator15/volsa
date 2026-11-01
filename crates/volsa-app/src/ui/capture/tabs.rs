//! What a picture of the window can ask for about the panel at the right and the windows and
//! lists that open over the pictures: which of the panel's tabs is open, and which windows.

use super::asks_for;
use crate::ui::panel::Tab;

/// The tab of the panel a picture asks for by the words in `wanted`: `tab-view`, `tab-measure`,
/// `tab-3d`, `tab-labels` or `tab-tools`; or, as the pictures of earlier work ask, `measure`
/// for the Measure tab, `labelling` for the Tools tab, where the strip that labels a scan is,
/// and `parts` for the Labels tab, where the labelled parts are. With more than one asked for,
/// the last of those in the order the words are listed above.
pub fn tab_asked(wanted: &str) -> Option<Tab> {
    let tabs = [
        ("tab-view", Tab::View),
        ("tab-measure", Tab::Measure),
        ("tab-3d", Tab::Scene),
        ("tab-labels", Tab::Labels),
        ("tab-tools", Tab::Tools),
        ("measure", Tab::Measure),
        ("labelling", Tab::Tools),
        ("parts", Tab::Labels),
    ];
    let mut asked = tabs.into_iter().filter(|(word, _)| asks_for(wanted, word));
    asked.next_back().map(|(_, tab)| tab)
}

/// Which windows and lists a picture asks to have open over the pictures.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "a set of independent things a picture may ask to have open"
)]
pub struct Opens {
    /// The tags of a file, for `tags`.
    pub tags: bool,
    /// The keys and what the mouse does, for `keys`.
    pub keys: bool,
    /// The About window, for `about`.
    pub about: bool,
    /// The search box, for `search`.
    pub search: bool,
    /// The other files in the rail of scans, for `other`.
    pub other: bool,
}

/// The windows and lists a picture asks for by the words in `wanted`.
pub fn opens(wanted: &str) -> Opens {
    let asked = |word: &str| asks_for(wanted, word);
    Opens {
        tags: asked("tags"),
        keys: asked("keys"),
        about: asked("about"),
        search: asked("search"),
        other: asked("other"),
    }
}
