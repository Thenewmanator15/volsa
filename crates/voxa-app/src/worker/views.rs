//! Reading and writing the saved views off the interface's thread.

use std::sync::Arc;
use std::sync::atomic::Ordering;

use voxa_core::SeriesId;

use super::{Workers, lock};
use crate::action::Action;
use crate::settings;
use crate::views::{SavedView, series_key};

impl Workers {
    /// Read the views kept for a series and report them. With nowhere to keep settings there
    /// is nothing to read and nothing is reported.
    pub(super) fn load_views(&self, series: SeriesId) {
        let Some(folder) = self.shared.settings.clone() else {
            return;
        };
        let shared = Arc::clone(&self.shared);
        self.spawn(move || {
            // Not while the file is half written.
            let _written = lock(&shared.stored);
            let views = settings::load(&folder, &series_key(&series));
            shared.report(Action::ViewsLoaded { series, views });
        });
    }

    /// Write the views of a series to the file. Of two writes asked for close together, the
    /// later is the one left in the file, whichever a worker gets to first.
    pub(super) fn store_views(&self, series: &SeriesId, views: Vec<SavedView>) {
        let Some(folder) = self.shared.settings.clone() else {
            return;
        };
        let number = self.shared.stores.fetch_add(1, Ordering::SeqCst) + 1;
        let (shared, key) = (Arc::clone(&self.shared), series_key(series));
        self.spawn(move || {
            let mut newest_written = lock(&shared.stored);
            if number < *newest_written {
                return;
            }
            *newest_written = number;
            if let Err(why) = settings::save(&folder, &key, &views) {
                let reason = why.to_string();
                shared.report(Action::ViewsNotStored { reason });
            }
        });
    }
}
