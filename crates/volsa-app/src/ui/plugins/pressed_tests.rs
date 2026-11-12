//! The plugin screens pressed through in order, as a person presses them: Add a plugin…, Add on
//! the card, Run, and Remove with its question. The state and the workers are the real ones, on a
//! made-up scan; the plugin is the test kit's example and its helper a stand-in served in this
//! process. Only the system's own window for choosing a folder is stood in for, by the action
//! that choosing one sends.

use std::sync::Arc;
use std::sync::mpsc::channel;
use std::time::{Duration, Instant};

use egui::Ui;
use serde_json::json;
use volsa_plugins::testkit::helper::{InProcessStarter, Threshold};
use volsa_plugins::testkit::onnx::{conv_network, image_input};
use volsa_plugins::testkit::{example, plugin_folder};

use crate::action::Action;
use crate::labelling::LabellingAction;
use crate::labels::LabelsStatus;
use crate::plugins::PluginAction;
use crate::session::Session;
use crate::testing::three_series_files;
use crate::ui::commands::{Request, Tab};
use crate::ui::hand::Hand;
use crate::ui::tabs;

/// Long enough for any worker on any machine; a test that waits this long has failed.
const PATIENCE: Duration = Duration::from_secs(60);
/// The example plugin's identity.
const ID: &str = "example.skull-strip";

/// Take in what the workers report until `done` holds.
fn until(session: &mut Session, what: &str, done: impl Fn(&Session) -> bool) {
    let started = Instant::now();
    while !done(session) {
        let notices = &session.state().notices;
        assert!(
            started.elapsed() < PATIENCE,
            "still waiting for {what}: {notices:?}"
        );
        session.wait(Duration::from_millis(50));
    }
}

/// Press the button a screen reader calls `name` in the Tools tab: the changes of state it
/// asks for, and what it asks of the window.
fn pressed_in_the_tab(session: &Session, name: &str) -> (Vec<Action>, Vec<Request>) {
    let state = session.state();
    let mut kept = tabs::Kept::default();
    let (mut actions, mut requests) = (Vec::new(), Vec::new());
    let found = Hand::new(300.0, 900.0).click_named(name, &mut |ui: &mut Ui| {
        requests.extend(tabs::show(ui, Tab::Tools, state, &mut kept, &mut actions));
    });
    assert!(found, "the Tools tab has a button called {name:?}");
    (actions, requests)
}

/// Press the button called `name` in the window a plugin puts over the pictures: its card, its
/// warning or its question.
fn pressed_in_the_window(session: &Session, name: &str) -> Vec<Action> {
    let state = session.state();
    let mut actions = Vec::new();
    let found = Hand::new(900.0, 900.0).click_named(name, &mut |ui: &mut Ui| {
        actions.extend(super::windows(ui.ctx(), &state.plugins));
    });
    assert!(found, "the window has a button called {name:?}");
    actions
}

/// Send the session what was pressed.
fn sent(session: &mut Session, actions: Vec<Action>) {
    assert!(!actions.is_empty(), "the press asked for something");
    for action in actions {
        session.dispatch(action);
    }
}

/// The folders a test keeps for as long as it runs: the plugin's, the runtime's, the settings'
/// and the made-up scan's.
type Folders = [tempfile::TempDir; 4];

/// The example plugin in a folder, its brightness scaled from the scan's lowest to its highest
/// value; and a folder that passes for a runtime's, holding a file of the library's name.
fn a_plugin_and_a_runtime() -> (tempfile::TempDir, tempfile::TempDir) {
    let mut description = example();
    description["input"]["scaling"] =
        json!({ "kind": "percentiles", "low": 0, "high": 100, "over": "scan" });
    let network = conv_network(&image_input(), &[0.0, 1.0], &[0.0, -0.5]);
    let source = plugin_folder(&description, &[("strip.onnx", &network)]);
    let runtime = tempfile::tempdir().expect("a folder");
    let library = if cfg!(windows) {
        "onnxruntime.dll"
    } else {
        "libonnxruntime.so"
    };
    std::fs::write(runtime.path().join(library), b"made-up runtime").expect("written");
    (source, runtime)
}

/// A session with a made-up scan open and its volume built, that keeps its settings, with a
/// runtime chosen and a stand-in helper; and the folders it needs, the plugin's first.
fn a_session_with_a_scan() -> (Session, Folders) {
    let (source, runtime) = a_plugin_and_a_runtime();
    let settings = tempfile::tempdir().expect("a folder");
    let (sender, reports) = channel();
    let kept_in = Some(settings.path().to_owned());
    let mut session = Session::new(sender, reports, 1 << 24, kept_in);
    // The stand-in helper says it held 64 MiB at most, and the computer that it has 32 GiB to
    // give, so that what this computer has free as the test runs decides nothing.
    let helper = InProcessStarter {
        opener: Threshold::default(),
        peak: 64 << 20,
        ..InProcessStarter::default()
    };
    session.workers().set_plugin_starter(Arc::new(helper));
    session
        .workers()
        .set_free_memory(|| volsa_models::FreeMemory::Promised(32 << 30));
    let chosen = LabellingAction::SetRuntimeFolder(runtime.path().to_owned());
    session.dispatch(Action::Labelling(chosen));
    let export = tempfile::tempdir().expect("a folder");
    for (name, bytes) in three_series_files() {
        let path = export.path().join(name);
        std::fs::create_dir_all(path.parent().expect("inside")).expect("makes folders");
        std::fs::write(path, bytes).expect("writes");
    }
    session.dispatch(Action::OpenPath(export.path().to_owned()));
    until(&mut session, "the scan's volume", |session| {
        crate::planes::grid(session.state()).is_some()
    });
    (session, [source, runtime, settings, export])
}

#[test]
fn a_plugin_is_added_run_and_removed_by_pressing_the_buttons_in_order() {
    let (mut session, folders) = a_session_with_a_scan();
    let [source, ..] = &folders;
    // "Add a plugin…" asks the window for a folder, and adds nothing itself.
    let (actions, requests) = pressed_in_the_tab(&session, "Add a plugin…");
    assert_eq!(requests, [Request::AddPlugin]);
    assert!(actions.is_empty(), "{actions:?}");
    // The person chooses the folder: its card is shown before anything is added.
    let offer = PluginAction::Offer(source.path().to_owned());
    session.dispatch(Action::Plugins(offer));
    until(&mut session, "the plugin's card", |session| {
        session.state().plugins.offered.is_some()
    });
    assert!(session.state().plugins.entries.is_empty(), "not added yet");

    // "Add" on the card adds it, and it is listed once it has been measured.
    let add = pressed_in_the_window(&session, "Add");
    sent(&mut session, add);
    until(&mut session, "the plugin added", |session| {
        let plugins = &session.state().plugins;
        plugins.busy.is_none() && plugins.entries.iter().any(|entry| entry.id == ID)
    });
    assert!(
        session.state().plugins.offered.is_none(),
        "the card is gone"
    );

    // "Run" runs it on the scan in the scene, after "Run anyway" if it warns of the contrast.
    let (run, _) = pressed_in_the_tab(&session, "Run");
    sent(&mut session, run);
    if session.state().plugins.warning.is_some() {
        let anyway = pressed_in_the_window(&session, "Run anyway");
        sent(&mut session, anyway);
    }
    until(&mut session, "the plugin's labels", |session| {
        let labels = &session.state().labels;
        labels.status == LabelsStatus::Ready && session.state().plugins.running.is_none()
    });
    let made = session.state().labels.made.as_ref().expect("who made them");
    let added = made.added.as_ref().expect("a plugin the person added");
    assert_eq!(added.id, ID, "the labels say which plugin made them");

    // "Remove" asks first, and "Remove" on the question takes it off the list.
    let (remove, _) = pressed_in_the_tab(&session, "Remove");
    sent(&mut session, remove);
    let listed = |session: &Session| session.state().plugins.entries.len();
    assert_eq!(
        listed(&session),
        1,
        "still there until the question is answered"
    );
    let confirmed = pressed_in_the_window(&session, "Remove");
    sent(&mut session, confirmed);
    until(&mut session, "the plugin removed", |session| {
        session.state().plugins.entries.is_empty()
    });
}
