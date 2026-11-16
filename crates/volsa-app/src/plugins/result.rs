//! How the labels a plugin made are shown while they are open on this series: the switch
//! and slider on the plugin's entry in the Tools tab work on every part at once.

/// How the labels a plugin made, open on this series, are shown.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ResultShown {
    /// Whether any of their parts is shown.
    pub shown: bool,
    /// How solid the first of them is, from a tenth to 1.
    pub opacity: f32,
    /// How many parts they have.
    pub parts: usize,
}

impl ResultShown {
    /// How the labels open are shown, if the plugin `id` made them and they are on this
    /// series.
    pub fn of(state: &crate::state::AppState, id: &str) -> Option<Self> {
        let made_it = crate::labels::made_by_plugin(state) == Some(id);
        let shown = &state.labels.shown;
        made_it.then(|| Self {
            shown: shown.iter().any(|how| how.shown),
            opacity: shown.first().map_or(1.0, |how| how.opacity),
            parts: state.labels.parts.len(),
        })
    }

    /// The actions that show or hide every part.
    pub fn show(self, shown: bool) -> Vec<crate::action::Action> {
        (0..self.parts)
            .map(|part| crate::action::Action::PartShow { part, shown })
            .collect()
    }

    /// The actions that make every part as solid as `opacity`.
    pub fn strength(self, opacity: f32) -> Vec<crate::action::Action> {
        (0..self.parts)
            .map(|part| crate::action::Action::PartOpacity { part, opacity })
            .collect()
    }
}
