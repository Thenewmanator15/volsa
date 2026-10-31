//! One run of an added plugin on a volume, from the plugin's grid to its labels: the grid and
//! its box or blocks, the statistics and the scaling, every block sampled, scaled and sent to a
//! helper capped at the place's measured need, each answer's labels put in the map, and the
//! map made a label set with the notes a reader should see. A stop ends the helper at once.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, PoisonError};
use std::thread;
use std::time::Duration;

use voxa_core::Volume;
use voxa_labels::{LabelGrid, Table};
use voxa_models::{Device, RunError, Stop, Supported};

use crate::finish::FinishError;
use crate::helper::{Client, HelperError, OpenJob, Where};
use crate::{
    BlockPlan, DescribeError, Description, Fit, Fits, Grid, HelperStarter, Input, LabelMap,
    LabelOutput, Measured, Output, Outside, Over, PluginError, Region, Scale, ScaleError, Scaling,
    Statistics, Wiring, check_folder, gather, outside_value, read_measured, sample, voxels_of,
};

#[cfg(test)]
mod tests;

mod notes;

pub use notes::BLOCKS_NOTE;
use notes::notes;

/// How often a run looks at its stop.
const LOOK_EVERY: Duration = Duration::from_millis(50);

/// A plugin as Voxa keeps it: its copy, checked again when it is read.
#[derive(Debug, Clone, PartialEq)]
pub struct Installed {
    /// The folder of Voxa's copy, named by the first 16 digits of its description's sum.
    pub folder: PathBuf,
    /// Its description.
    pub description: Description,
    /// The SHA-256 of its description.
    pub sum: String,
    /// How its network is wired to its description.
    pub wiring: Wiring,
    /// What it was measured to need, once it has been.
    pub measured: Option<Measured>,
}

/// How far a run has come.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    /// The grid, the statistics and the scaling are being worked out.
    Preparing,
    /// The network is about to be given block `block` of `of`.
    Running {
        /// Which block, the first being 1.
        block: usize,
        /// How many blocks there are.
        of: usize,
    },
    /// The labels are being made a label set.
    Finishing,
}

/// What a run made.
#[derive(Debug, Clone, PartialEq)]
pub struct Ran {
    /// The labels on the plugin's grid, along the scan's own axes.
    pub grid: LabelGrid,
    /// The table of their names.
    pub table: Table,
    /// What a reader should know of how they were made, each a sentence.
    pub notes: Vec<String>,
}

/// Why a run made nothing. Each sentence is fit to be shown.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PluginRunError {
    /// The helper ended, or the network file changed.
    #[error(transparent)]
    Helper(HelperError),
    /// The scan could not be scaled.
    #[error(transparent)]
    Scale(ScaleError),
    /// The answer could not be finished to labels.
    #[error(transparent)]
    Finish(FinishError),
    /// The network could not be run, there was not the memory, or the run was stopped.
    #[error(transparent)]
    Run(RunError),
    /// The plugin is of a kind this version cannot run.
    #[error(transparent)]
    Describe(DescribeError),
}

impl From<HelperError> for PluginRunError {
    fn from(why: HelperError) -> Self {
        match why {
            HelperError::Run(why) => PluginRunError::Run(why),
            HelperError::Answer(why) => PluginRunError::Finish(why),
            other => PluginRunError::Helper(other),
        }
    }
}

/// Reads an added plugin's copy and checks it again: every file against its description, and
/// the description against the sum the folder is named by.
///
/// # Errors
///
/// [`PluginError::Changed`] when the copy is not what was added; other [`PluginError`]s as
/// [`check_folder`] gives them.
pub fn installed(folder: &Path, supported: Supported) -> Result<Installed, PluginError> {
    let offered = check_folder(folder, supported).map_err(|why| match why {
        PluginError::NoDescription
        | PluginError::Missing(_)
        | PluginError::NotAFile(_)
        | PluginError::WrongSize(_)
        | PluginError::WrongSum(_) => PluginError::Changed,
        other => other,
    })?;
    let named = folder
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    if named.len() != 16 || !offered.sum.starts_with(named) {
        return Err(PluginError::Changed);
    }
    Ok(Installed {
        folder: folder.to_owned(),
        description: offered.description,
        sum: offered.sum,
        wiring: offered.wiring,
        measured: read_measured(folder),
    })
}

/// What a run works with once the grid, the statistics and the scaling are known.
struct Prepared {
    grid: Grid,
    region: Region,
    plan: Option<BlockPlan>,
    block: [usize; 3],
    scale: Scale,
    outside: f32,
}

/// Works out the grid, its box or blocks, the statistics and the scaling.
fn prepare(volume: &Volume, input: &Input) -> Result<Prepared, PluginRunError> {
    let grid = Grid::of(volume, input);
    let (region, plan, block) = match input.fit {
        Fit::Whole(size) => (grid.whole_box(size), None, size),
        Fit::Blocks { size, step } => {
            let plan = BlockPlan {
                grid: grid.size,
                size,
                step,
            };
            (grid.whole(), Some(plan), size)
        }
    };
    let over = match input.scaling {
        Scaling::Percentiles {
            over: Over::Grid, ..
        }
        | Scaling::ZScore {
            over: Over::Grid, ..
        } => Some(region),
        _ => None,
    };
    let needed = !matches!(
        (input.scaling, input.outside),
        (Scaling::Fixed { .. } | Scaling::None, Outside::Zero)
    );
    let stats = if needed {
        gather(volume, &grid, over, voxels_of(&input.scaling)).map_err(PluginRunError::Run)?
    } else {
        Statistics::none()
    };
    let scale = Scale::new(&input.scaling, &stats).map_err(PluginRunError::Scale)?;
    let outside = outside_value(input.outside, &stats);
    Ok(Prepared {
        grid,
        region,
        plan,
        block,
        scale,
        outside,
    })
}

/// Where a network runs, as the helper is told.
fn where_of(device: Device) -> Where {
    match device {
        Device::Processor => Where::Processor,
        Device::Card(card) => Where::Card {
            id: card.0,
            name: String::new(),
        },
    }
}

/// Runs an added label-map plugin on a volume, at a place it was measured to fit (`place.cap`
/// above 0), with the runtime in `runtime`, its helper started by `starter`. `said` is told each
/// stage before it begins. When `stop` is set the helper is ended at once.
///
/// # Errors
///
/// [`PluginRunError`] for why nothing was made: [`RunError::Stopped`] when stopped.
pub fn run_plugin(
    plugin: &Installed,
    volume: &Volume,
    place: Fits,
    runtime: &Path,
    starter: &dyn HelperStarter,
    stop: &Stop,
    said: &mut dyn FnMut(Stage),
) -> Result<Ran, PluginRunError> {
    let description = &plugin.description;
    description.runnable().map_err(PluginRunError::Describe)?;
    let Output::Labels(output) = &description.output else {
        return Err(PluginRunError::Finish(FinishError::WrongAnswer));
    };
    if place.cap == 0 {
        let why = "the plugin has not been measured on this device".to_owned();
        return Err(PluginRunError::Run(RunError::Failed(why)));
    }
    said(Stage::Preparing);
    let prepared = prepare(volume, &description.input)?;
    let job = job_of(plugin, output, &prepared, place, runtime);
    let mut map = LabelMap::new(prepared.region).map_err(PluginRunError::Finish)?;
    let outcome = on_helper(starter, place.cap, stop, |client| {
        client.open(&job)?;
        run_blocks(&prepared, volume, client, &mut map, said, stop)?;
        client.finish()?;
        Ok(())
    });
    match outcome {
        Err(_) if stop.is_set() => return Err(PluginRunError::Run(RunError::Stopped)),
        other => other?,
    }
    said(Stage::Finishing);
    let (grid, table) = map
        .finish(output, &description.name, &prepared.grid, volume)
        .map_err(PluginRunError::Finish)?;
    Ok(Ran {
        grid,
        table,
        notes: notes(description, &prepared, volume),
    })
}

/// The job the helper is given.
fn job_of(
    plugin: &Installed,
    output: &LabelOutput,
    prepared: &Prepared,
    place: Fits,
    runtime: &Path,
) -> OpenJob {
    job_for(plugin, output, prepared.block, place.device, runtime)
}

/// The job for a plugin's network on a device, its blocks of `block`.
fn job_for(
    plugin: &Installed,
    output: &LabelOutput,
    block: [usize; 3],
    device: Device,
    runtime: &Path,
) -> OpenJob {
    let description = &plugin.description;
    let listed = description
        .files
        .iter()
        .find(|file| file.name == description.network);
    OpenJob {
        runtime: runtime.to_owned(),
        network: plugin.folder.join(&description.network),
        sha256: listed.map(|file| file.sha256.clone()).unwrap_or_default(),
        device: where_of(device),
        free: plugin.wiring.free.clone(),
        output: Some(plugin.wiring.output.clone()),
        layout: description.input.layout,
        block,
        finish: output.clone(),
    }
}

/// The job a plugin is measured with on a device: one block of the size its network is always
/// given. `None` for a plugin of a kind this version cannot run.
pub fn measuring_job(plugin: &Installed, device: Device, runtime: &Path) -> Option<OpenJob> {
    let Output::Labels(output) = &plugin.description.output else {
        return None;
    };
    let block = match plugin.description.input.fit {
        Fit::Whole(size) | Fit::Blocks { size, .. } => size,
    };
    Some(job_for(plugin, output, block, device, runtime))
}

/// Does `work` with a helper capped at `cap`, ending the helper when `stop` is set and when the
/// work is done.
fn on_helper(
    starter: &dyn HelperStarter,
    cap: u64,
    stop: &Stop,
    work: impl FnOnce(
        &mut Client<Box<dyn std::io::Read + Send>, Box<dyn std::io::Write + Send>>,
    ) -> Result<(), PluginRunError>,
) -> Result<(), PluginRunError> {
    let pipes = starter.start(Some(cap)).map_err(|_| HelperError::Ended)?;
    let helper = Mutex::new(pipes);
    let mut client = helper
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .client()
        .ok_or(HelperError::Ended)?;
    let done = AtomicBool::new(false);
    let outcome = thread::scope(|scope| {
        scope.spawn(|| {
            while !done.load(Ordering::SeqCst) {
                if stop.is_set() {
                    helper.lock().unwrap_or_else(PoisonError::into_inner).stop();
                    return;
                }
                thread::sleep(LOOK_EVERY);
            }
        });
        let outcome = work(&mut client);
        done.store(true, Ordering::SeqCst);
        outcome
    });
    helper.lock().unwrap_or_else(PoisonError::into_inner).stop();
    outcome
}

/// Samples, scales and sends each block, and puts its labels in the map.
fn run_blocks(
    prepared: &Prepared,
    volume: &Volume,
    client: &mut Client<Box<dyn std::io::Read + Send>, Box<dyn std::io::Write + Send>>,
    map: &mut LabelMap,
    said: &mut dyn FnMut(Stage),
    stop: &Stop,
) -> Result<(), PluginRunError> {
    let signed = |start: [usize; 3]| start.map(|side| isize::try_from(side).unwrap_or(isize::MAX));
    let blocks: Vec<([isize; 3], Region)> = match &prepared.plan {
        None => vec![(prepared.region.from, prepared.region)],
        Some(plan) => plan
            .starts()
            .into_iter()
            .map(|start| (signed(start), plan.owned(start)))
            .collect(),
    };
    let of = blocks.len();
    for (place, (start, owned)) in blocks.into_iter().enumerate() {
        if stop.is_set() {
            return Err(PluginRunError::Run(RunError::Stopped));
        }
        said(Stage::Running {
            block: place + 1,
            of,
        });
        let block = Region {
            from: start,
            size: prepared.block,
        };
        let mut values =
            sample(volume, &prepared.grid, block, prepared.outside).map_err(PluginRunError::Run)?;
        prepared.scale.apply(&mut values);
        let labels = client.block(&values)?;
        map.put(start, prepared.block, owned, &labels);
    }
    Ok(())
}
