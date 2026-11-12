//! Decoding the slices a viewport asks for, and saying each is ready with its windows.

use std::sync::Arc;
use std::sync::atomic::Ordering;

use volsa_dicom::Frame;
use volsa_volume::PRESETS;

use super::{Batch, Request, Shared, lock};
use crate::action::Action;

impl Shared {
    /// Decode the slices of one batch, in order, unless the request has been overtaken.
    pub(super) fn decode(&self, batch: &Batch, request: &Request) {
        if !self.still_wanted(request) {
            return;
        }
        let Some(file) = self.file_of(batch, request) else {
            return;
        };
        for (slice, frame) in &batch.slices {
            if !self.still_wanted(request) {
                return;
            }
            let decoded = match &file {
                Ok(file) => file.decode(*frame).map_err(|error| error.to_string()),
                Err(error) => Err(error.clone()),
            };
            self.deliver(request, *slice, decoded);
        }
    }

    /// Hold a decoded slice for the viewport and announce it, or announce that it failed.
    fn deliver(&self, request: &Request, slice: usize, decoded: Result<Frame, String>) {
        let series = request.series.clone();
        let report = match decoded {
            Ok(frame) => {
                let windows = PRESETS.map(|preset| preset.window(&frame.values));
                let frame = Arc::new(frame);
                if !self.hold(
                    &series,
                    slice,
                    frame,
                    (request.around, request.opened),
                    true,
                ) {
                    return;
                }
                Action::FrameReady {
                    series,
                    slice,
                    windows,
                }
            }
            Err(reason) => Action::FrameFailed {
                series,
                slice,
                reason,
            },
        };
        self.report(report);
    }

    /// Whether a request is still the newest for its series, in the source still open.
    pub(super) fn still_wanted(&self, request: &Request) -> bool {
        self.opened.load(Ordering::SeqCst) == request.opened
            && lock(&self.newest).get(&request.series) == Some(&request.number)
    }
}
