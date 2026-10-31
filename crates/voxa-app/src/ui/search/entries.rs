//! What search can find: every action, the parts of the labels that are open, the scans of
//! the export that is open, and the help topics. They are listed again only when one of them
//! may have changed, which [`stamp`] tells.

use std::hash::{DefaultHasher, Hash, Hasher};

use voxa_core::SeriesId;

use crate::labels::{LabelsStatus, name_of};
use crate::naming::plain_name;
use crate::state::AppState;
use crate::ui::commands;

/// The groups the results are listed under, in the order they are listed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Heading {
    /// The things a person can do.
    Actions,
    /// The parts of the labels that are open.
    Parts,
    /// The scans of the export that is open.
    Scans,
    /// The help topics.
    Help,
}

impl Heading {
    /// What the list calls the group.
    pub const fn title(self) -> &'static str {
        match self {
            Self::Actions => "Actions",
            Self::Parts => "Parts",
            Self::Scans => "Scans",
            Self::Help => "Help",
        }
    }
}

/// What a result is, and so what choosing it does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    /// An action, by its id in the list of actions.
    Command(&'static str),
    /// A part, by its place among the parts of the labels that are open.
    Part(usize),
    /// A scan.
    Scan(SeriesId),
    /// A help topic, by its id.
    Help(&'static str),
}

/// One thing search can find.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// What it is listed under.
    pub heading: Heading,
    /// What it is called, which is what the list shows.
    pub name: String,
    /// Other words it can be found by.
    pub words: Vec<String>,
    /// What choosing it does.
    pub target: Target,
}

/// A help topic search finds: its id, its title, and the other words it is found by.
type Topic = (&'static str, &'static str, &'static [&'static str]);

/// The help topics. Task 19 writes them, and lists them here from `help::TOPICS`.
const HELP_TOPICS: &[Topic] = &[];

/// Everything search can find in `state`: the actions in the order of the list of actions,
/// then the parts of the labels that are open, the scans, and the help topics.
pub fn entries_for(state: &AppState) -> Vec<Entry> {
    let actions = commands::all().iter().map(|command| Entry {
        heading: Heading::Actions,
        name: command.name.to_owned(),
        words: owned(command.words),
        target: Target::Command(command.id),
    });
    let labels = &state.labels;
    let open = labels.status == LabelsStatus::Ready;
    let parts = labels.parts.iter().enumerate().filter(|_| open);
    let parts = parts.map(|(place, part)| Entry {
        heading: Heading::Parts,
        name: name_of(part),
        words: vec![part.group.clone()],
        target: Target::Part(place),
    });
    let series = state.index.iter().flat_map(|index| &index.series);
    let scans = series.map(|series| Entry {
        heading: Heading::Scans,
        name: plain_name(&series.info),
        words: vec![series.info.description.clone()],
        target: Target::Scan(series.info.id.clone()),
    });
    let help = HELP_TOPICS.iter().map(|(id, title, words)| Entry {
        heading: Heading::Help,
        name: (*title).to_owned(),
        words: owned(words),
        target: Target::Help(id),
    });
    actions.chain(parts).chain(scans).chain(help).collect()
}

/// Words held as owned text.
fn owned(words: &[&str]) -> Vec<String> {
    words.iter().map(|word| (*word).to_owned()).collect()
}

/// A number that differs whenever what [`entries_for`] gives for `state` may differ: it holds
/// the number of actions, what identifies and describes each series (the number of series
/// alone would not tell one export from another with as many), and the open labels' parts.
pub fn stamp(state: &AppState) -> u64 {
    let mut hasher = DefaultHasher::new();
    commands::all().len().hash(&mut hasher);
    if let Some(index) = &state.index {
        index.series.len().hash(&mut hasher);
        for series in &index.series {
            let info = &series.info;
            (&info.id, &info.description, &info.modality).hash(&mut hasher);
            (info.slices, info.scan.in_3d, &info.scan.body_part).hash(&mut hasher);
        }
    }
    let labels = &state.labels;
    (labels.status == LabelsStatus::Ready).hash(&mut hasher);
    for part in &labels.parts {
        (name_of(part), &part.group).hash(&mut hasher);
    }
    hasher.finish()
}
