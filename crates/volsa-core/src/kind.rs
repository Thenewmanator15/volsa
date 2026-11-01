//! What kind of thing a series is, in the words a person would use for it.

use crate::model::{SeriesInfo, SeriesProblem};

/// The most images a set of locating images holds. More than this in several planes is
/// something else, and is said to be a stack with a reason.
pub const MOST_LOCATING: usize = 12;

/// What a series is. Every export holds series that are not volumes, and most of those are
/// as they should be: only the last two kinds mean that something stops Volsa showing a
/// series in full.
#[derive(Debug, Clone, PartialEq)]
pub enum SeriesKind {
    /// A regular stack of slices: it can be shown in 3D.
    Volume,
    /// A few images in several planes, taken to plan the examination.
    Locating,
    /// One image.
    Single,
    /// Pictures stored in colour: maps, screenshots, pages of a report.
    Colour,
    /// A stack that cannot be treated as a volume, and the first reason why.
    Stack(SeriesProblem),
    /// Images compressed in a way Volsa cannot read; the compression is named.
    Unreadable(String),
}

impl SeriesKind {
    /// The kind in a few plain words, for the kinds that are normal in an export and are not
    /// volumes. `None` for a volume, which needs no words, and for the two kinds that are
    /// faults, which are described by their reason.
    pub fn says(&self) -> Option<&'static str> {
        match self {
            Self::Locating => Some("locating images"),
            Self::Single => Some("a single image"),
            Self::Colour => Some("colour pictures"),
            Self::Volume | Self::Stack(_) | Self::Unreadable(_) => None,
        }
    }

    /// Whether something stops Volsa showing the series as it was meant to be seen.
    pub fn is_fault(&self) -> bool {
        matches!(self, Self::Stack(_) | Self::Unreadable(_))
    }
}

impl SeriesInfo {
    /// What kind of series this is.
    pub fn kind(&self) -> SeriesKind {
        let unreadable = self.problems.iter().find_map(|problem| match problem {
            SeriesProblem::UnsupportedTransferSyntax(name) => Some(name.clone()),
            _ => None,
        });
        if let Some(name) = unreadable {
            return SeriesKind::Unreadable(name);
        }
        if self.colour {
            return SeriesKind::Colour;
        }
        if self.slices == 1 {
            return SeriesKind::Single;
        }
        let planes = self.problems.contains(&SeriesProblem::MixedOrientations);
        let only_that = self.problems.iter().all(|problem| {
            matches!(
                problem,
                SeriesProblem::MixedOrientations | SeriesProblem::MixedSizes
            )
        });
        if planes && only_that && self.slices <= MOST_LOCATING {
            return SeriesKind::Locating;
        }
        match self.problems.first() {
            Some(problem) => SeriesKind::Stack(problem.clone()),
            None => SeriesKind::Volume,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn of(slices: usize, problems: &[SeriesProblem]) -> SeriesKind {
        let info = SeriesInfo {
            slices,
            problems: problems.to_vec(),
            ..SeriesInfo::default()
        };
        info.kind()
    }

    #[test]
    fn a_series_with_nothing_against_it_is_a_volume_and_needs_no_words() {
        assert_eq!(of(30, &[]), SeriesKind::Volume);
        assert_eq!(SeriesKind::Volume.says(), None);
        assert!(!SeriesKind::Volume.is_fault());
    }

    #[test]
    fn a_few_images_in_several_planes_are_locating_images_not_a_fault() {
        let planes = [SeriesProblem::MixedOrientations];
        assert_eq!(of(3, &planes), SeriesKind::Locating);
        assert_eq!(SeriesKind::Locating.says(), Some("locating images"));
        assert!(!SeriesKind::Locating.is_fault());
        let and_sizes = [SeriesProblem::MixedOrientations, SeriesProblem::MixedSizes];
        assert_eq!(of(MOST_LOCATING, &and_sizes), SeriesKind::Locating);
        assert_eq!(
            of(MOST_LOCATING + 1, &planes),
            SeriesKind::Stack(SeriesProblem::MixedOrientations),
            "a long series in several planes is not a set of locating images"
        );
        assert_eq!(
            of(3, &[SeriesProblem::MixedSizes]),
            SeriesKind::Stack(SeriesProblem::MixedSizes),
            "sizes alone do not make locating images"
        );
    }

    #[test]
    fn one_image_is_a_single_image() {
        assert_eq!(of(1, &[]), SeriesKind::Single);
        assert_eq!(SeriesKind::Single.says(), Some("a single image"));
        assert!(!SeriesKind::Single.is_fault());
    }

    #[test]
    fn colour_pictures_are_colour_pictures_whatever_else() {
        let info = SeriesInfo {
            slices: 4,
            colour: true,
            problems: vec![SeriesProblem::MixedSizes],
            ..SeriesInfo::default()
        };
        assert_eq!(info.kind(), SeriesKind::Colour);
        assert_eq!(SeriesKind::Colour.says(), Some("colour pictures"));
        assert!(!SeriesKind::Colour.is_fault());
    }

    #[test]
    fn a_stack_with_a_reason_is_a_fault_and_says_nothing_soothing() {
        let uneven = of(20, &[SeriesProblem::UnevenSpacing]);
        assert_eq!(uneven, SeriesKind::Stack(SeriesProblem::UnevenSpacing));
        assert!(uneven.is_fault());
        assert_eq!(uneven.says(), None);
    }

    #[test]
    fn images_that_cannot_be_read_come_before_everything() {
        let info = SeriesInfo {
            slices: 1,
            colour: true,
            problems: vec![
                SeriesProblem::MixedOrientations,
                SeriesProblem::UnsupportedTransferSyntax("x".to_owned()),
            ],
            ..SeriesInfo::default()
        };
        assert_eq!(info.kind(), SeriesKind::Unreadable("x".to_owned()));
        assert!(info.kind().is_fault());
    }

    #[test]
    fn each_problem_has_a_few_words_for_a_narrow_place() {
        assert_eq!(SeriesProblem::UnevenSpacing.brief(), "uneven spacing");
        assert_eq!(SeriesProblem::MissingGeometry.brief(), "no position data");
        assert_eq!(
            SeriesProblem::UnsupportedTransferSyntax("x".to_owned()).brief(),
            "cannot be read yet"
        );
    }
}
