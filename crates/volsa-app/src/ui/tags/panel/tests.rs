//! The tag browser's panel asked for a file by its entry, as a document's tags are.

use std::sync::mpsc::channel;
use std::time::{Duration, Instant};

use super::*;
use crate::session::Session;
use crate::testing::documents::{PDF, REPORT, document_files};

/// Draw the panel's bookkeeping again until its listing satisfies `done`.
fn until(panel: &mut Panel, session: &mut Session, done: impl Fn(&Listing) -> bool) {
    let ctx = egui::Context::default();
    let started = Instant::now();
    loop {
        session.pump();
        panel.refresh(session.state(), session.workers(), &ctx);
        if done(&panel.listing) {
            return;
        }
        assert!(started.elapsed() < Duration::from_secs(60), "still waiting");
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn value_of<'a>(listing: &'a Listing, keyword: &str) -> Option<&'a str> {
    let rows = listing.as_ref().ok()?;
    let row = rows.iter().find(|row| row.keyword == keyword);
    row.map(|row| row.value.as_str())
}

#[test]
fn the_tags_of_a_file_named_by_its_entry_are_listed_and_follow_the_entry() {
    let folder = tempfile::tempdir().expect("a temporary folder");
    for (name, bytes) in document_files() {
        let path = folder.path().join(name);
        std::fs::create_dir_all(path.parent().expect("inside")).expect("makes folders");
        std::fs::write(path, bytes).expect("writes");
    }
    let (sender, reports) = channel();
    let mut session = Session::new(sender, reports, 1 << 24, None);
    session.dispatch(crate::action::Action::OpenPath(folder.path().to_owned()));
    let started = Instant::now();
    while session.state().document.is_none() {
        assert!(started.elapsed() < Duration::from_secs(60), "still waiting");
        session.wait(Duration::from_millis(50));
    }
    let mut panel = Panel::default();
    // Closed, it asks for nothing.
    panel.refresh(
        session.state(),
        session.workers(),
        &egui::Context::default(),
    );
    assert_eq!(panel.asked, None);
    panel.open_on(TagsOf::Entry(REPORT.to_owned()));
    until(&mut panel, &mut session, |listing| {
        let class = value_of(listing, "SOPClassUID");
        class.is_some_and(|class| class.starts_with("1.2.840.10008.5.1.4.1.1.88."))
    });
    // Another entry takes its place.
    panel.open_on(TagsOf::Entry(PDF.to_owned()));
    until(&mut panel, &mut session, |listing| {
        value_of(listing, "SOPClassUID") == Some("1.2.840.10008.5.1.4.1.1.104.1")
    });
    // An entry that is not there is said, not listed.
    panel.open_on(TagsOf::Entry("DOCS/nothing-like-it".to_owned()));
    until(&mut panel, &mut session, Result::is_err);
}
