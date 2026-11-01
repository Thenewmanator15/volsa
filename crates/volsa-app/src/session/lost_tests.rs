//! What was made before the graphics card stopped answering is there after it, and the work
//! goes on: the card is the window's, and nothing that is kept depends on it.

use std::path::{Path, PathBuf};

use volsa_render::DeviceWatch;

use super::Session;
use super::tests::{open_until, ready_in_3d, session_keeping, until, zip_of};
use crate::action::Action;
use crate::annotations::Shape;
use crate::kept_labels::{keep, kept};
use crate::testing::{FINE_SCAN, a_device, id, lose, three_series_files, written_labels};
use crate::ui::lost::{Card, STOPPED};
use crate::views::{ViewsAction, series_key};

fn ruler() -> Shape {
    Shape::Ruler {
        a: [0.0, 0.0, 10.0],
        b: [6.0, 0.0, 10.0],
    }
}

fn marker() -> Shape {
    Shape::Marker {
        at: [2.0, 3.0, 25.0],
    }
}

fn read(file: &Path) -> Vec<u8> {
    std::fs::read(file).expect("the file reads")
}

fn holds(file: &Path, text: &str) -> bool {
    std::fs::read_to_string(file).is_ok_and(|kept| kept.contains(text))
}

/// Make two annotations, rename one, save a view and keep a result, in a session keeping its
/// settings in `settings`; wait until the files hold them. Gives the bytes of the label file
/// and of the table that were handed to be kept.
fn make_things(session: &mut Session, settings: &Path) -> (Vec<u8>, Vec<u8>) {
    session.dispatch(Action::AnnotationAdd(ruler()));
    session.dispatch(Action::AnnotationAdd(marker()));
    let label = "the tip of it".to_owned();
    session.dispatch(Action::AnnotationRename { id: 2, label });
    session.dispatch(Action::SceneOrbit { by: [40.0, 10.0] });
    let name = "Turned a little".to_owned();
    session.dispatch(Action::Views(ViewsAction::Save { name }));
    let (labels, table) = written_labels();
    let key = series_key(&id(FINE_SCAN));
    keep(settings, &key, &labels, &table).expect("it is kept");
    until(session, |_| {
        holds(&settings.join("annotations.json"), "the tip of it")
            && holds(&settings.join("views.json"), "Turned a little")
    });
    (labels, table.into_bytes())
}

/// The four files of what is kept, as bytes: the annotations, the views, the labels and
/// their table.
fn files_kept(settings: &Path) -> [(PathBuf, Vec<u8>); 4] {
    let (labels, table) = kept(settings, &series_key(&id(FINE_SCAN))).expect("a result is kept");
    let both = [
        settings.join("annotations.json"),
        settings.join("views.json"),
        labels,
        table,
    ];
    both.map(|file| (file.clone(), read(&file)))
}

/// The graphics card stops answering, where this computer has a card to stop, and the window
/// says so, once, to `session`. No message box is opened.
fn the_card_stops_answering(session: &mut Session) {
    let Some((device, _queue)) = a_device() else {
        return;
    };
    let watch = DeviceWatch::on(&device, || {});
    lose(&device);
    let mut card = Card::new(Some(watch), Box::new(|_| {}));
    card.look(|action| session.dispatch(action));
    card.look(|action| session.dispatch(action));
    let [notice] = &session.state().notices[..] else {
        panic!("{:?}", session.state().notices);
    };
    assert_eq!(notice.text, STOPPED);
}

#[test]
fn what_was_kept_before_the_card_stopped_answering_reads_back_whole_after_it() {
    let settings = tempfile::tempdir().expect("a temporary folder");
    let (_folder, path) = zip_of(&three_series_files());
    let mut session = session_keeping(settings.path());
    open_until(&mut session, &path, |session| {
        ready_in_3d(session, FINE_SCAN)
    });
    let (labels, table) = make_things(&mut session, settings.path());
    let saved_from = session.state().scene.camera;
    let before = files_kept(settings.path());
    // What was kept is what was handed over to be kept, to the byte.
    assert_eq!((&before[2].1, &before[3].1), (&labels, &table));

    the_card_stops_answering(&mut session);

    // What was kept is as it was, and the work goes on: another annotation is kept.
    assert_eq!(files_kept(settings.path()), before);
    session.dispatch(Action::AnnotationAdd(marker()));
    let annotations = settings.path().join("annotations.json");
    until(&mut session, |_| read(&annotations) != before[0].1);
    drop(session);

    // And all of it is there the next time.
    let mut again = session_keeping(settings.path());
    open_until(&mut again, &path, |session| {
        let state = session.state();
        ready_in_3d(session, FINE_SCAN)
            && !state.views.is_empty()
            && state.annotations.items.len() == 3
            && state.labelling.kept.is_some()
    });
    let state = again.state();
    let items = &state.annotations.items;
    let names: Vec<&str> = items.iter().map(|item| item.label.as_str()).collect();
    assert_eq!(names, ["Ruler 1", "the tip of it", "Marker 3"]);
    assert_eq!((&items[0].shape, &items[1].shape), (&ruler(), &marker()));
    assert_eq!(state.views[0].name, "Turned a little");
    assert_eq!(state.views[0].camera(), saved_from);
    assert_eq!(state.labelling.kept.as_deref(), Some("2099-01-02"));
    let after = files_kept(settings.path());
    assert_eq!((&after[2].1, &after[3].1), (&labels, &table));
}
