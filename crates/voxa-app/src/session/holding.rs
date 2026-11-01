//! A session made up for a test of the window: one that holds a state and a volume as if the
//! workers had made them, with no export behind it.

use std::sync::mpsc::channel;
use std::sync::{Arc, PoisonError};

use voxa_core::{HeldVolume, SeriesId};

use super::Session;
use crate::state::AppState;

impl Session {
    /// A session whose state is `state`, and whose store holds `volume` as the volume of
    /// `series`: the volume itself is what the graphics device is to hold, as for any volume
    /// the device can hold whole.
    pub(crate) fn holding(state: AppState, series: &SeriesId, volume: Arc<HeldVolume>) -> Self {
        let (sender, reports) = channel();
        let mut session = Self::new(sender, reports, 1 << 20, None);
        session.state = state;
        let mut store = session
            .volumes
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        store.put(series.clone(), volume);
        drop(store);
        session
    }
}
