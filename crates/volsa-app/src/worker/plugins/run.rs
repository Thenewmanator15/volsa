//! A run of a plugin on the volume kept for a series: in labelling's turn, at each place the
//! plugin was measured to fit (measured first where it was not), the next tried when a place
//! has no room or its card stops answering. Its labels are written as the series' kept result
//! for the plugin, with who made them and the stamp first among the notes, and the state is
//! told where they are.

use std::path::Path;
use std::sync::Arc;

use volsa_core::{SeriesId, Volume};
use volsa_labels::{Added, MAX_MADE_LINE, Made, write_labels, write_table};
use volsa_models::{Device, Room, RunError, RunsOn, Stop, file_sum};
use volsa_plugins::{
    CardFigures, Fits, Grid, Installed, Measured, PluginRunError, Ran, Stage, measure,
    measuring_job, own_bytes, places, run_plugin, write_measured,
};

use super::{GONE, NO_RUNTIME, NO_SETTINGS, kept_key, library, runtime_folder};
use crate::kept_labels::{keep, kept};
use crate::plugins::{PluginAction, PluginEffect};
use crate::worker::labelling::{Labeller, on_its_own, room_of};
use crate::worker::{Shared, Workers, lock, shielded};

/// What is said when the volume of the series is no longer kept.
const NO_VOLUME: &str = "The 3D view of this scan is no longer there to run the plugin on.";
/// What is said when no place has the memory.
const NO_PLACE: &str = "There is not the memory on this computer to run this plugin now.";
/// What is said when a run stops part-way inside Volsa.
const PART_WAY: &str = "The plugin stopped part-way: something went wrong inside Volsa.";
/// The most of a runtime's library read for its sum.
const MOST_LIBRARY: u64 = 1 << 31;

/// One run, as the worker carries it out.
struct Job {
    run: u64,
    id: String,
    series: SeriesId,
    stamp: Option<String>,
    runs_on: RunsOn,
    held: u64,
    drawing_on: Option<String>,
    runtime: Option<std::path::PathBuf>,
    stop: Stop,
    volume: Option<Arc<Volume>>,
}

impl Workers {
    /// Run a plugin on a thread of its own, in place of any plugin run on.
    pub(super) fn run_plugin(&self, effect: PluginEffect) {
        let PluginEffect::Run {
            run,
            id,
            series,
            stamp,
            runs_on,
            held,
            drawing_on,
            runtime,
        } = effect
        else {
            return;
        };
        let stop = Stop::new();
        if let Some(before) = lock(&self.shared.plugins.run).replace(stop.clone()) {
            before.stop();
        }
        let volume = lock(&self.shared.volumes).get(&series);
        let job = Job {
            run,
            id,
            series,
            stamp,
            runs_on,
            held,
            drawing_on,
            runtime,
            stop,
            volume,
        };
        let shared = Arc::clone(&self.shared);
        let started = on_its_own(move || {
            if !shielded(|| shared.carry_out_plugin(&job)) {
                shared.plugin_failed(&job, PART_WAY);
            }
        });
        if !started {
            let why = "The plugin could not start: the computer would not give it a thread.";
            self.shared.tell_plugins(PluginAction::Failed {
                run,
                why: why.to_owned(),
            });
        }
    }
}

impl Shared {
    /// Say that a run failed, unless it was stopped.
    fn plugin_failed(&self, job: &Job, why: &str) {
        if !job.stop.is_set() {
            let (run, why) = (job.run, why.to_owned());
            self.tell_plugins(PluginAction::Failed { run, why });
        }
    }

    /// Carry out a run once it has labelling's turn.
    fn carry_out_plugin(&self, job: &Job) {
        let Some(_turn) = self.labelling.turn(&job.stop) else {
            return;
        };
        let Some(volume) = job.volume.as_deref() else {
            return self.plugin_failed(job, NO_VOLUME);
        };
        let (Some(settings), Some(plugin)) = (self.settings.clone(), self.plugin(&job.id)) else {
            let why = if self.settings.is_none() {
                NO_SETTINGS
            } else {
                GONE
            };
            return self.plugin_failed(job, why);
        };
        let Some(runtime) = runtime_folder(job.runtime.clone()) else {
            return self.plugin_failed(job, NO_RUNTIME);
        };
        let labeller = self.labelling.labeller();
        let room = room_of(labeller, job.drawing_on.as_deref(), job.held);
        match self.climb_places(job, &plugin, volume, &runtime, &room) {
            Ok((ran, device)) => self.made(job, &plugin, &settings, ran, &device, labeller),
            Err(why) => self.plugin_failed(job, &why),
        }
    }

    /// Try each place the plugin fits, measuring first where it was not measured; the next
    /// when one has no room or its card stops answering.
    fn climb_places(
        &self,
        job: &Job,
        plugin: &Installed,
        volume: &Volume,
        runtime: &Path,
        room: &Room,
    ) -> Result<(Ran, String), String> {
        let runtime_sum = file_sum(&library(runtime), MOST_LIBRARY).unwrap_or_default();
        let mut measured = plugin
            .measured
            .clone()
            .filter(|measured| measured.runtime_sum == runtime_sum)
            .unwrap_or(Measured {
                runtime_sum,
                processor: None,
                cards: Vec::new(),
            });
        let input = &plugin.description.input;
        let grid = Grid::of(volume, input);
        let block = match input.fit {
            volsa_plugins::Fit::Whole(size) | volsa_plugins::Fit::Blocks { size, .. } => size,
        };
        let own = own_bytes(block, grid.size.iter().map(|side| *side as u64).product());
        let mut why_last = NO_PLACE.to_owned();
        for place in places(&measured, own, room, job.runs_on) {
            if job.stop.is_set() {
                return Err(why_last);
            }
            let place = match place.cap {
                0 => match self.measure_place(
                    &job.stop,
                    plugin,
                    place.device,
                    runtime,
                    room,
                    &mut measured,
                    own,
                ) {
                    Some(place) => place,
                    None => continue,
                },
                _ => place,
            };
            let starter = self.plugins.starter();
            let mut said = |stage: Stage| {
                self.tell_plugins(PluginAction::Stage {
                    run: job.run,
                    stage,
                });
            };
            match run_plugin(
                plugin,
                volume,
                place,
                runtime,
                starter.as_ref(),
                &job.stop,
                &mut said,
            ) {
                Ok(ran) => return Ok((ran, device_name(place.device, room))),
                Err(PluginRunError::Run(why @ (RunError::NoRoom | RunError::DeviceLost))) => {
                    why_last = why.to_string();
                }
                Err(why) => return Err(why.to_string()),
            }
        }
        Err(why_last)
    }

    /// Measure a plugin on a place it was not measured on, keep the figures, and give the
    /// place with its cap if it fits there now. Stop ends the measuring at once.
    #[expect(
        clippy::too_many_arguments,
        reason = "the run's stop, the plugin and where it is measured, and what it is told"
    )]
    fn measure_place(
        &self,
        stop: &Stop,
        plugin: &Installed,
        device: Device,
        runtime: &Path,
        room: &Room,
        measured: &mut Measured,
        own: u64,
    ) -> Option<Fits> {
        let job = measuring_job(plugin, device, runtime)?;
        let starter = self.plugins.starter();
        let (ordinary, card) =
            measure(starter.as_ref(), &job, Some(room.free_memory), stop).ok()?;
        match device {
            Device::Processor => measured.processor = Some(ordinary),
            Device::Card(id) => {
                let name = room.cards.iter().find(|card| card.id == id)?.name.clone();
                measured.cards.retain(|figures| figures.name != name);
                let card_bytes = card.unwrap_or(0);
                measured.cards.push(CardFigures {
                    name,
                    card_bytes,
                    ordinary_bytes: ordinary,
                });
            }
        }
        let _ = write_measured(&plugin.folder, measured);
        let only = match device {
            Device::Processor => RunsOn::Processor,
            Device::Card(id) => RunsOn::Card(id),
        };
        places(measured, own, room, only)
            .into_iter()
            .find(|fits| fits.cap > 0)
    }

    /// A run made labels: say who made them, keep them as the series' result for the plugin,
    /// and tell the state where they are.
    fn made(
        &self,
        job: &Job,
        plugin: &Installed,
        settings: &Path,
        ran: Ran,
        device: &str,
        labeller: &dyn Labeller,
    ) {
        let Ran {
            grid,
            mut table,
            notes,
        } = ran;
        let description = &plugin.description;
        let line = |text: String| text.chars().take(MAX_MADE_LINE).collect::<String>();
        table.made = Some(Made {
            by: line(format!("Volsa {}", env!("CARGO_PKG_VERSION"))),
            model: line(format!("{} {}", description.name, description.version)),
            model_sum: plugin.sum.clone(),
            device: line(device.to_owned()),
            when: line(labeller.today()),
            notes: job.stamp.iter().cloned().chain(notes).collect(),
            added: Some(Added {
                id: description.id.clone(),
                name: description.name.clone(),
                version: description.version.clone(),
                made_by: description.made_by.clone(),
                source: description.source.clone(),
                licence: description.licence.clone(),
                limits: description.limits.clone(),
                sum: plugin.sum.clone(),
            }),
        });
        let key = kept_key(&job.series, &description.id);
        let written = write_labels(&grid).ok().zip(write_table(&table).ok());
        let files = written
            .filter(|(labels, table)| keep(settings, &key, labels, table).is_ok())
            .and_then(|_| kept(settings, &key));
        match files {
            Some((labels, table)) if !job.stop.is_set() => {
                self.tell_plugins(PluginAction::Made {
                    run: job.run,
                    labels,
                    table,
                });
            }
            Some(_) => {}
            None => self.plugin_failed(job, "The plugin's labels could not be kept."),
        }
    }
}

/// A device as a result says it: the processor, or the card by its name.
fn device_name(device: Device, room: &Room) -> String {
    match device {
        Device::Processor => "the processor".to_owned(),
        Device::Card(id) => room
            .cards
            .iter()
            .find(|card| card.id == id)
            .map_or_else(|| "a graphics card".to_owned(), |card| card.name.clone()),
    }
}
