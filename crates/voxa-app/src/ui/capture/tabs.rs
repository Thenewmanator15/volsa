//! What a picture of the window can ask for about the panel at the right: which of its tabs is
//! open.

use super::asks_for;
use crate::ui::panel::Tab;

/// The tab of the panel a picture asks for by the words in `wanted`: the Measure tab for
/// `measure`, the Tools tab, where the strip that labels a scan is, for `labelling`, and the
/// Labels tab, where the labelled parts are, for `parts`. With more than one asked for, the
/// last of those.
pub fn tab_asked(wanted: &str) -> Option<Tab> {
    let tabs = [
        ("measure", Tab::Measure),
        ("labelling", Tab::Tools),
        ("parts", Tab::Labels),
    ];
    let mut asked = tabs.into_iter().filter(|(word, _)| asks_for(wanted, word));
    asked.next_back().map(|(_, tab)| tab)
}
