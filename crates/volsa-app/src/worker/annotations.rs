//! Reading and writing the kept annotations off the interface's thread.

use std::sync::Arc;
use std::sync::atomic::Ordering;

use volsa_core::SeriesId;

use super::{Workers, lock};
use crate::action::Action;
use crate::annotations::{Annotation, load_annotations, save_annotations};
use crate::views::series_key;

impl Workers {
    /// Read the annotations kept for a series and report them. With nowhere to keep settings
    /// there is nothing to read and nothing is reported.
    pub(super) fn load_annotations(&self, series: SeriesId) {
        let Some(folder) = self.shared.settings.clone() else {
            return;
        };
        let shared = Arc::clone(&self.shared);
        self.spawn(move || {
            // Not while the file is half written.
            let _written = lock(&shared.noted);
            let items = load_annotations(&folder, &series_key(&series));
            shared.report(Action::AnnotationsLoaded { series, items });
        });
    }

    /// Write the annotations of a series to the file. Of two writes asked for close together,
    /// the later is the one left in the file, whichever a worker gets to first.
    pub(super) fn store_annotations(&self, series: &SeriesId, items: Vec<Annotation>) {
        let Some(folder) = self.shared.settings.clone() else {
            return;
        };
        let number = self.shared.notes.fetch_add(1, Ordering::SeqCst) + 1;
        let (shared, key) = (Arc::clone(&self.shared), series_key(series));
        self.spawn(move || {
            let mut newest_written = lock(&shared.noted);
            if number < *newest_written {
                return;
            }
            *newest_written = number;
            if let Err(why) = save_annotations(&folder, &key, &items) {
                let reason = why.to_string();
                shared.report(Action::AnnotationsNotStored { reason });
            }
        });
    }
}
