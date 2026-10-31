//! A session made up for a test of the window: one that holds a state and a volume as if the
//! workers had made them, with no export behind it.

use std::sync::mpsc::channel;
use std::sync::{Arc, PoisonError};

use voxa_core::{SeriesId, Volume};
use voxa_render::Prepared;

use super::Session;
use crate::state::AppState;

impl Session {
    /// A session whose state is `state`, and whose store holds `volume` as the volume of
    /// `series`, with what a worker would have made ready of it for the graphics device,
    /// if anything, waiting to be taken.
    pub(crate) fn holding(
        state: AppState,
        series: &SeriesId,
        volume: Arc<Volume>,
        ready: Option<Prepared>,
    ) -> Self {
        let (sender, reports) = channel();
        let mut session = Self::new(sender, reports, 1 << 20, None);
        session.state = state;
        let mut store = session
            .volumes
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        store.put(series.clone(), volume);
        if let Some(ready) = ready {
            store.put_prepared(series, ready);
        }
        drop(store);
        session
    }
}
