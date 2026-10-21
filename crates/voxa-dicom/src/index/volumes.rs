//! A series that holds several volumes: one image at every position for each echo, diffusion
//! weighting or time point. Each volume is listed as a series of its own that knows which
//! volume it is, so that everything made for a series works on a volume unchanged.

use std::cmp::Ordering;

use voxa_core::{SeriesId, VolumeSet};

use super::{Image, IndexedSeries, Slice, assembled, placement, slices_of};
use crate::geometry::SAME_POSITION_MM;
use crate::header::Told;

/// The series the images filed under one identifier make: one, or one for each volume when
/// every position is filled the same number of times.
pub(super) fn listed(id: &str, images: &[Image]) -> Vec<IndexedSeries> {
    let Some(first) = images.first().map(|image| &image.header) else {
        return Vec::new();
    };
    let slices: Vec<Slice<'_>> = images.iter().flat_map(slices_of).collect();
    let volumes = placement(first, images, &slices).ok().and_then(|stack| {
        let in_order = stack.order.iter().filter_map(|at| slices.get(*at).copied());
        split(&in_order.collect::<Vec<_>>(), &stack.steps_mm)
    });
    let Some(volumes) = volumes else {
        return vec![assembled(id.to_owned(), first, images, slices)];
    };
    let count = volumes.len();
    let names = what(&volumes);
    let numbered = volumes.into_iter().zip(names).enumerate();
    numbered
        .map(|(at, (slices, what))| {
            let first = slices.first().map_or(first, |slice| slice.header);
            let mut series = assembled(format!("{id}@{}", at + 1), first, images, slices);
            series.info.set = Some(VolumeSet {
                of: SeriesId(id.to_owned()),
                at,
                count,
                what,
            });
            series
        })
        .collect()
}

/// For slices in order along the stack, with the gaps between neighbours: the slices of each
/// volume, in order of position, when every position is filled the same number of times and
/// at least twice. `None` otherwise: such a series is not a set of volumes.
fn split<'a>(in_order: &[Slice<'a>], gaps_mm: &[f64]) -> Option<Vec<Vec<Slice<'a>>>> {
    let mut places: Vec<Vec<Slice<'a>>> = Vec::new();
    for (at, slice) in in_order.iter().enumerate() {
        let gap_before = at.checked_sub(1).and_then(|before| gaps_mm.get(before));
        let same_place = gap_before.is_some_and(|gap| *gap < SAME_POSITION_MM);
        match places.last_mut() {
            Some(place) if same_place => place.push(*slice),
            _ => places.push(vec![*slice]),
        }
    }
    let count = places.first()?.len();
    if count < 2 || places.iter().any(|place| place.len() != count) {
        return None;
    }
    for place in &mut places {
        place.sort_by(by_what_it_says);
    }
    let volume = |k: usize| places.iter().filter_map(move |place| place.get(k).copied());
    Some((0..count).map(|k| volume(k).collect()).collect())
}

/// The order of two images at one position: by what tells volumes apart, most telling first,
/// and then by the order the scanner numbered and stored them in.
fn by_what_it_says(a: &Slice<'_>, b: &Slice<'_>) -> Ordering {
    let measured = |a: Option<f64>, b: Option<f64>| match (a, b) {
        (Some(a), Some(b)) => a.total_cmp(&b),
        (a, b) => a.is_some().cmp(&b.is_some()),
    };
    let (told, other) = (&a.told, &b.told);
    told.temporal
        .cmp(&other.temporal)
        .then(measured(told.b_value, other.b_value))
        .then(measured(told.echo_ms, other.echo_ms))
        .then(measured(told.trigger_ms, other.trigger_ms))
        .then(told.acquisition.cmp(&other.acquisition))
        .then((a.instance, a.entry, a.frame).cmp(&(b.instance, b.entry, b.frame)))
}

/// A way of reading one thing an image says, and of putting it into words.
type Way = (fn(&Told) -> Option<f64>, fn(f64) -> String);

/// A number to two decimal places, without the zeros that say nothing.
fn plain(value: f64) -> String {
    format!("{}", (value * 100.0).round() / 100.0)
}

/// A name for each volume, from the first thing that every image of a volume agrees on and
/// that differs between volumes; empty names when there is no such thing.
fn what(volumes: &[Vec<Slice<'_>>]) -> Vec<String> {
    let ways: [Way; 5] = [
        (
            |told| told.temporal.map(f64::from),
            |time| format!("time {}", plain(time)),
        ),
        (|told| told.b_value, |b| format!("b {}", plain(b))),
        (|told| told.echo_ms, |ms| format!("echo {} ms", plain(ms))),
        (
            |told| told.trigger_ms,
            |ms| format!("trigger {} ms", plain(ms)),
        ),
        (
            |told| told.acquisition.map(f64::from),
            |number| format!("acquisition {}", plain(number)),
        ),
    ];
    for (reads, says) in ways {
        let values: Option<Vec<f64>> = volumes.iter().map(|volume| agreed(volume, reads)).collect();
        let differ = |values: &[f64]| {
            values
                .windows(2)
                .any(|pair| matches!(pair, [a, b] if a.total_cmp(b).is_ne()))
        };
        if let Some(values) = values.filter(|values| differ(values)) {
            return values.into_iter().map(says).collect();
        }
    }
    vec![String::new(); volumes.len()]
}

/// The value every image of a volume gives, when they all give one and the same.
fn agreed(volume: &[Slice<'_>], reads: fn(&Told) -> Option<f64>) -> Option<f64> {
    let mut values = volume.iter().map(|slice| reads(&slice.told));
    let first = values.next()??;
    let same = |value: Option<f64>| value.is_some_and(|value| value.total_cmp(&first).is_eq());
    values.all(same).then_some(first)
}
