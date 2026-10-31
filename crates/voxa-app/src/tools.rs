//! What the Tools tab says of a tool that is not part of Voxa.
//!
//! The plugin engine, built in another session, makes one [`ToolInfo`] for each tool a person
//! has added and hands the list to the window; the Tools tab lists them under "Added by you".
//! Nothing here runs a tool or reads its files: it is only what is said of one, so that the tab
//! can show who made it, where it came from, on what terms, whether its files passed their
//! check, and what it takes and gives back.

/// Said wherever the result or the entry of a tool that is not part of Voxa is shown, word for
/// word, so that nothing a tool made reads as Voxa's own.
pub const ADDED_BY_YOU: &str = "Added by you, not part of Voxa";

/// Who made a tool.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MadeBy {
    /// A tool that comes with Voxa.
    Voxa,
    /// A tool the person has added, which Voxa did not make and does not vouch for.
    AddedByYou,
}

/// What the Tools tab says of one tool.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolInfo {
    /// What the tool is called.
    pub name: String,
    /// Who made it.
    pub made_by: MadeBy,
    /// Where it came from: the address or folder it was added from.
    pub source: String,
    /// The licence it comes under, in a few words.
    pub licence: String,
    /// Whether its files passed their check: `Some(true)` if they did, `Some(false)` if they
    /// did not, `None` if they have not been checked.
    pub checked: Option<bool>,
    /// What it accepts, in a few words: a kind of scan, say.
    pub accepts: String,
    /// What it returns, in a few words: labels, say.
    pub returns: String,
}
