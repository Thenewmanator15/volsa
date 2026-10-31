//! Adding a plugin on the workers: on a thread of its own, never the workers' shared pool; in
//! labelling's turn, so that its measuring never shares a card with a run; and stopped at once
//! by Stop.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use voxa_models::Stop;
use voxa_plugins::testkit::helper::{InProcessStarter, Threshold};
use voxa_plugins::{HelperPipes, HelperStarter};

use super::plugins_run_tests::{PEAK, lab, runtime, source, until};
use crate::action::Effect;
use crate::plugins::{PluginAction, PluginEffect};

/// A starter that notes the thread each helper was started from, and when.
#[derive(Default)]
struct Noting {
    inner: InProcessStarter,
    noted: Mutex<Vec<(String, Instant)>>,
}

impl HelperStarter for Noting {
    fn start(&self, cap: Option<u64>) -> std::io::Result<Box<dyn HelperPipes>> {
        let thread = std::thread::current().name().unwrap_or_default().to_owned();
        let mut noted = self.noted.lock().expect("noted");
        noted.push((thread, Instant::now()));
        self.inner.start(cap)
    }
}

#[test]
fn adding_measures_on_a_thread_of_its_own_once_labelling_gives_its_turn() {
    let lab = lab(Threshold::default());
    let noting = Arc::new(Noting {
        inner: InProcessStarter {
            peak: PEAK,
            ..InProcessStarter::default()
        },
        ..Noting::default()
    });
    lab.workers
        .set_plugin_starter(Arc::clone(&noting) as Arc<dyn HelperStarter>);
    let held = Stop::new();
    let turn = lab.workers.shared.labelling.turn(&held).expect("the turn");
    let (source, runtime) = (source(), runtime());
    lab.workers.run(
        Effect::Plugins(PluginEffect::Offer(source.path().to_owned())),
        None,
    );
    until(&lab, |said| matches!(said, PluginAction::Offered(_)));
    let asked = PluginEffect::Add {
        runtime: Some(runtime.path().to_owned()),
    };
    lab.workers.run(Effect::Plugins(asked), None);
    std::thread::sleep(Duration::from_millis(300));
    let released = Instant::now();
    drop(turn);
    let said = until(&lab, |said| matches!(said, PluginAction::Added(_)));
    assert!(
        matches!(said.last(), Some(PluginAction::Added(Ok(_)))),
        "{said:?}"
    );
    let noted = noting.noted.lock().expect("noted");
    assert!(!noted.is_empty());
    for (thread, when) in noted.iter() {
        assert!(
            !thread.starts_with("voxa-worker"),
            "on the shared pool: {thread}"
        );
        assert!(*when >= released, "measured while labelling had its turn");
    }
}

#[test]
fn adding_is_stopped_at_once_while_it_measures() {
    let lab = lab(Threshold {
        slow: true,
        ..Threshold::default()
    });
    let (source, runtime) = (source(), runtime());
    lab.workers.run(
        Effect::Plugins(PluginEffect::Offer(source.path().to_owned())),
        None,
    );
    until(&lab, |said| matches!(said, PluginAction::Offered(_)));
    let asked = PluginEffect::Add {
        runtime: Some(runtime.path().to_owned()),
    };
    lab.workers.run(Effect::Plugins(asked), None);
    until(
        &lab,
        |said| matches!(said, PluginAction::Busy(what) if what.contains("on the processor")),
    );
    let began = Instant::now();
    lab.workers.run(Effect::Plugins(PluginEffect::Stop), None);
    let said = until(&lab, |said| matches!(said, PluginAction::Added(_)));
    let why = "Adding the plugin was stopped.".to_owned();
    assert_eq!(said.last(), Some(&PluginAction::Added(Err(why))));
    assert!(
        began.elapsed() < Duration::from_millis(800),
        "{:?}",
        began.elapsed()
    );
    let plugins = lab.settings().join("plugins");
    let copies = std::fs::read_dir(&plugins).map_or(0, Iterator::count);
    assert_eq!(copies, 0, "nothing is kept of a plugin not added");
}
