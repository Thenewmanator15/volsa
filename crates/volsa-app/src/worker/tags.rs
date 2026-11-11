//! Listing the elements of a file for the tag browser.

use std::sync::Arc;

use voxa_dicom::{IndexedSeries, TagRow};

use super::{Workers, lock};

impl Workers {
    /// List the elements of the file behind one slice, on a worker, and hand them to `reply`.
    /// The rows hold the patient's details, so they go to `reply` alone and are never made
    /// into an [`Action`](crate::Action).
    pub fn list_tags(
        &self,
        listed: &IndexedSeries,
        slice: usize,
        reply: impl FnOnce(Result<Vec<TagRow>, String>) + Send + 'static,
    ) {
        let Some(at) = listed.slices.get(slice).cloned() else {
            reply(Err("There is no such slice.".to_owned()));
            return;
        };
        let shared = Arc::clone(&self.shared);
        self.spawn(move || {
            let rows = match lock(&shared.source).as_mut() {
                Some(source) => voxa_dicom::list_tags(source.as_mut(), &at),
                None => return reply(Err("No scan is open.".to_owned())),
            };
            reply(rows.map_err(|error| error.to_string()));
        });
    }
}
