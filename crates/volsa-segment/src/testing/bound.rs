//! The bound on the ordinary memory a whole run on the real networks may ask the system for,
//! which the tests that run them hold the run to. The figures are `volsa-models`' own
//! ([`asks`]); what is added here is what the test itself holds beside Volsa.

use volsa_models::{Card, CardKind, Cut, Device, Rung, Wanted, asks, cards};

#[cfg(test)]
mod tests;

/// Bytes in a number of 32 bits, which is what the arrays a test reads are made of.
const BYTES_A_NUMBER: u64 = 4;

/// What a whole run on one device may ask the system for in ordinary memory, at its peak: what
/// [`asks`] says for the rung that ran, and what the test itself holds beside Volsa.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bound {
    /// How many voxels the networks are given.
    voxels: u64,
    /// What is wanted of the run.
    wanted: Wanted,
    /// What the test holds while the networks run, apart from Volsa's own: each array it read
    /// before the run, by name, with its size in bytes.
    beside: Vec<(&'static str, u64)>,
}

impl Bound {
    /// The bound of a run on an image of `voxels`, beside which the test holds the arrays
    /// named, each with how many numbers it has. Their sizes are counted from those lengths.
    pub fn new(voxels: usize, wanted: Wanted, arrays: &[(&'static str, usize)]) -> Self {
        let bytes = |count: usize| {
            u64::try_from(count).map_or(u64::MAX, |c| c.saturating_mul(BYTES_A_NUMBER))
        };
        Self {
            voxels: u64::try_from(voxels).unwrap_or(u64::MAX),
            wanted,
            beside: arrays.iter().map(|&(name, n)| (name, bytes(n))).collect(),
        }
    }

    /// What the whole scan, run on this device with the scores network there too, may ask the
    /// system for, without what the test holds: [`asks`] for that rung.
    fn run(&self, device: Device) -> u64 {
        let rung = Rung {
            device,
            cut: Cut::Whole,
            scores_on: device,
        };
        asks(self.voxels, self.wanted, rung)
    }

    /// How many bytes the test holds beside Volsa.
    fn held_beside(&self) -> u64 {
        self.beside
            .iter()
            .fold(0, |sum, &(_, bytes)| sum.saturating_add(bytes))
    }

    /// What a run on this device may ask the system for, in bytes: the run itself and what
    /// the test holds beside it.
    pub fn allowed(&self, device: Device) -> u64 {
        self.run(device).saturating_add(self.held_beside())
    }

    /// Holds what the process asked the system for at its peak, `asked`, to the bound for a
    /// run on this device. Gives the words to print, or the words of the failure. On
    /// integrated graphics, whose memory is ordinary memory and was not measured apart, the
    /// comparison is shown and never held to. Where the system keeps no such count the bound
    /// cannot be held, and that is a failure where it should keep one (on Windows).
    ///
    /// # Errors
    ///
    /// The process asked for more than the bound allows, or its count could not be read.
    pub fn hold(&self, device: Device, asked: Option<u64>) -> Result<String, String> {
        self.held(device, asked, shares_memory(device))
    }

    /// [`hold`](Self::hold), told whether the device shares the computer's memory.
    fn held(&self, device: Device, asked: Option<u64>, shared: bool) -> Result<String, String> {
        let (run, beside) = (self.run(device), self.held_beside());
        let named = self
            .beside
            .iter()
            .map(|(name, bytes)| format!("{name} {bytes}"));
        let named: Vec<String> = named.collect();
        let allowed = self.allowed(device);
        let bound = format!(
            "{allowed} bytes: {run} for the run, by `asks`, and {beside} that the test itself \
             holds beside it ({})",
            named.join(", ")
        );
        match asked {
            Some(asked) if asked <= allowed => Ok(format!(
                "asked the system for {asked} bytes, within the bound of {bound}"
            )),
            Some(asked) if shared => Ok(format!(
                "asked the system for {asked} bytes, over the bound of {bound}; integrated \
                 graphics are shown the comparison and not held to it"
            )),
            Some(asked) => Err(format!(
                "The process asked the system for {asked} bytes, over the bound of {bound}"
            )),
            None if shared || !cfg!(windows) => Ok(
                "the bound was not held: no count of what the process asked for was read"
                    .to_owned(),
            ),
            None => Err("What the process asked the system for could not be read.".to_owned()),
        }
    }
}

/// Whether a device is graphics that share the computer's ordinary memory.
fn shares_memory(device: Device) -> bool {
    let Device::Card(id) = device else {
        return false;
    };
    let same = |card: &Card| card.id == id && card.kind == CardKind::Integrated;
    cards().iter().any(same)
}
