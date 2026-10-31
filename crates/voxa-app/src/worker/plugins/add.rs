//! A folder offered, checked and shown on its card; added, measured on the processor and on
//! each discrete card, and its figures kept beside it; or removed.

use std::path::{Path, PathBuf};

use voxa_models::{CardKind, Device, SUPPORTED, file_sum};
use voxa_plugins::{
    CardFigures, Installed, Measured, check_folder, measure, measuring_job, write_measured,
};

use super::{GONE, NO_RUNTIME, NO_SETTINGS, library, runs_on_words, runtime_folder};
use crate::plugins::{OfferCard, PluginAction, PluginEntry, add, remove, same_id};
use crate::worker::labelling::room_of;
use crate::worker::{Shared, lock};

/// The most of a runtime's library read for its sum.
const MOST_LIBRARY: u64 = 1 << 31;

impl Shared {
    /// Check a folder offered, and show its card; keep it until it is added.
    pub(super) fn offer(&self, folder: &Path) {
        let card = match check_folder(folder, SUPPORTED) {
            Ok(offered) => {
                let added: Vec<PluginEntry> = self
                    .added()
                    .into_iter()
                    .filter_map(Result::ok)
                    .map(|plugin| PluginEntry::of_installed(&plugin, ""))
                    .collect();
                let card = OfferCard::of(&offered, &added);
                *lock(&self.plugins.offered) = Some(offered);
                Ok(card)
            }
            Err(why) => Err(why.to_string()),
        };
        self.tell_plugins(PluginAction::Offered(card));
    }

    /// Add the folder last offered: copy it, measure it, keep its figures, and put it in place
    /// of a plugin of the same identity. A plugin that cannot run on the processor is not
    /// added.
    pub(super) fn add_offered(&self, chosen: Option<PathBuf>) {
        let Some(offered) = lock(&self.plugins.offered).take() else {
            return;
        };
        let added = self.add_and_measure(&offered, chosen);
        self.tell_plugins(PluginAction::Added(added));
    }

    /// The work of [`add_offered`](Self::add_offered): the new plugin's entry, or why it was
    /// not added.
    fn add_and_measure(
        &self,
        offered: &voxa_plugins::Offered,
        chosen: Option<PathBuf>,
    ) -> Result<PluginEntry, String> {
        let folder = self.plugins_folder().ok_or(NO_SETTINGS)?;
        let runtime = runtime_folder(chosen).ok_or(NO_RUNTIME)?;
        let before: Vec<Installed> = self.added().into_iter().filter_map(Result::ok).collect();
        let installed = add(offered, &folder, SUPPORTED).map_err(|why| why.to_string())?;
        let measured = match self.measure_everywhere(&installed, &runtime) {
            Ok(measured) => measured,
            Err(why) => {
                let _ = remove(&installed.folder);
                return Err(format!(
                    "The plugin was not added: it could not be run to measure what it needs. {why}"
                ));
            }
        };
        let _ = write_measured(&installed.folder, &measured);
        if let Some(old) = same_id(&before, offered).filter(|old| old.sum != installed.sum) {
            let _ = remove(&old.folder);
        }
        Ok(PluginEntry::of_installed(
            &installed,
            &runs_on_words(Some(&measured)),
        ))
    }

    /// Measure a plugin on the processor, which must work, and on each discrete card, where
    /// it may not.
    fn measure_everywhere(&self, plugin: &Installed, runtime: &Path) -> Result<Measured, String> {
        let starter = self.plugins.starter();
        let labeller = self.labelling.labeller();
        let room = room_of(labeller, None, 0);
        let runtime_sum = file_sum(&library(runtime), MOST_LIBRARY).unwrap_or_default();
        let job = |device| measuring_job(plugin, device, runtime).ok_or("This plugin cannot run.");
        self.tell_plugins(PluginAction::Busy(
            "Measuring what the plugin needs on the processor.".to_owned(),
        ));
        let cap = Some(room.free_memory);
        let (processor, _) = measure(starter.as_ref(), &job(Device::Processor)?, cap)
            .map_err(|why| why.to_string())?;
        let mut cards = Vec::new();
        for card in room
            .cards
            .iter()
            .filter(|card| card.kind == CardKind::Discrete)
        {
            let busy = format!("Measuring what the plugin needs on {}.", card.name);
            self.tell_plugins(PluginAction::Busy(busy));
            if let Ok((ordinary_bytes, card_bytes)) =
                measure(starter.as_ref(), &job(Device::Card(card.id))?, cap)
            {
                cards.push(CardFigures {
                    name: card.name.clone(),
                    card_bytes: card_bytes.unwrap_or(0),
                    ordinary_bytes,
                });
            }
        }
        Ok(Measured {
            runtime_sum,
            processor: Some(processor),
            cards,
        })
    }

    /// Remove the plugin of this identity, or the broken copy of this name.
    pub(super) fn remove_plugin(&self, id: &str) {
        let copy = self.added().into_iter().find_map(|plugin| match plugin {
            Ok(plugin) if plugin.description.id == id => Some(plugin.folder),
            Err((name, _)) if name == id => self.plugins_folder().map(|folder| folder.join(name)),
            _ => None,
        });
        let removed = match copy {
            Some(copy) => remove(&copy)
                .map(|()| id.to_owned())
                .map_err(|_| "The plugin could not be removed.".to_owned()),
            None => Err(GONE.to_owned()),
        };
        self.tell_plugins(PluginAction::Removed(removed));
    }
}
