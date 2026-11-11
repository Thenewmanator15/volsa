//! What kind of scan a series is, as its own files state it.

/// One of the three planes a body is usually cut in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Facing {
    /// Across the body: the slices are stacked from feet to head.
    Axial,
    /// From the front: the slices are stacked from front to back.
    Coronal,
    /// From the side: the slices are stacked from one side to the other.
    Sagittal,
}

/// The plane the slices of a series lie in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Plane {
    /// The usual plane the slices lie nearest to.
    pub nearest: Facing,
    /// True when they are tilted well away from it.
    pub oblique: bool,
}

/// A family of ways of making the picture. A series can belong to more than one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Family {
    /// An inversion pulse comes first.
    InversionRecovery,
    /// The whole picture is read after one excitation.
    EchoPlanar,
    /// The echo is made by a refocusing pulse.
    SpinEcho,
    /// The echo is made by the gradients alone.
    GradientEcho,
}

/// What the files of a series say about how it was acquired. Every part may be absent:
/// scanners differ in what they write.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ScanFacts {
    /// The plane of the slices.
    pub plane: Option<Plane>,
    /// Whether the series was acquired as one three-dimensional block (`true`) or slice by
    /// slice (`false`).
    pub in_3d: Option<bool>,
    /// The families of sequence it belongs to.
    pub family: Vec<Family>,
    /// Repetition time, in milliseconds.
    pub repetition_ms: Option<f64>,
    /// Echo time, in milliseconds.
    pub echo_ms: Option<f64>,
    /// Inversion time, in milliseconds.
    pub inversion_ms: Option<f64>,
    /// Flip angle, in degrees.
    pub flip_deg: Option<f64>,
    /// Field strength, in tesla.
    pub field_t: Option<f64>,
    /// The part of the body examined, as the scanner names it.
    pub body_part: String,
}

/// Slices tipped further than this from the nearest usual plane are called oblique.
const OBLIQUE_BEYOND_DEG: f64 = 15.0;

impl Facing {
    fn word(self) -> &'static str {
        match self {
            Self::Axial => "axial",
            Self::Coronal => "coronal",
            Self::Sagittal => "sagittal",
        }
    }
}

impl Plane {
    /// The plane of slices whose columns run along the first three numbers of `orientation`
    /// and whose rows run along the last three, in the patient. `None` when the two do not
    /// span a plane.
    pub fn of(orientation: [f64; 6]) -> Option<Self> {
        let [ax, ay, az, bx, by, bz] = orientation;
        let across = [ay * bz - az * by, az * bx - ax * bz, ax * by - ay * bx];
        let length = across.iter().map(|part| part * part).sum::<f64>().sqrt();
        if !length.is_finite() || length < 1e-6 {
            return None;
        }
        let [x, y, z] = across.map(|part| (part / length).abs());
        let (nearest, along) = if z >= x && z >= y {
            (Facing::Axial, z)
        } else if y >= x {
            (Facing::Coronal, y)
        } else {
            (Facing::Sagittal, x)
        };
        Some(Self {
            nearest,
            oblique: along < OBLIQUE_BEYOND_DEG.to_radians().cos(),
        })
    }

    /// The plane in a word or two, with a capital: "Axial", "Oblique coronal".
    pub fn words(self) -> String {
        let word = self.nearest.word();
        if self.oblique {
            return format!("Oblique {word}");
        }
        let mut letters = word.chars();
        let first = letters.next().map(|letter| letter.to_ascii_uppercase());
        first.into_iter().chain(letters).collect()
    }
}

/// A number to two decimal places, without the zeros that say nothing.
fn plain(value: f64) -> String {
    format!("{}", (value * 100.0).round() / 100.0)
}

impl ScanFacts {
    /// The families of sequence as one phrase: "inversion-recovery spin echo".
    fn family_words(&self) -> Option<String> {
        let has = |family: Family| self.family.contains(&family);
        let echo = match (has(Family::SpinEcho), has(Family::GradientEcho)) {
            (true, true) => Some("spin and gradient echo"),
            (true, false) => Some("spin echo"),
            (false, true) => Some("gradient echo"),
            (false, false) => None,
        };
        let mut words: Vec<&str> = Vec::new();
        if has(Family::InversionRecovery) {
            words.push(if echo.is_some() || has(Family::EchoPlanar) {
                "inversion-recovery"
            } else {
                "inversion recovery"
            });
        }
        if has(Family::EchoPlanar) {
            words.push("echo-planar");
        }
        words.extend(echo);
        (!words.is_empty()).then(|| words.join(" "))
    }

    /// A few words for a narrow place: the plane, whether in two or three dimensions, and the
    /// family of sequence, as many of them as are known. `None` when none is.
    pub fn line(&self) -> Option<String> {
        let dimensions = self.in_3d.map(|block| if block { "3D" } else { "2D" });
        let parts: Vec<String> = [
            self.plane.map(Plane::words),
            dimensions.map(str::to_owned),
            self.family_words(),
        ]
        .into_iter()
        .flatten()
        .collect();
        (!parts.is_empty()).then(|| parts.join(" · "))
    }

    /// Everything known, one fact a line, each with its unit.
    pub fn lines(&self) -> Vec<String> {
        let plane = self.plane.map(|plane| {
            let word = plane.nearest.word();
            if plane.oblique {
                format!("Plane: oblique, nearest {word}")
            } else {
                format!("Plane: {word}")
            }
        });
        let acquired = self.in_3d.map(|block| {
            if block {
                "Acquired as one block (3D)".to_owned()
            } else {
                "Acquired slice by slice (2D)".to_owned()
            }
        });
        let measured = |name: &str, value: Option<f64>, unit: &str| {
            value.map(|value| format!("{name} {}{unit}", plain(value)))
        };
        let body = (!self.body_part.is_empty()).then(|| format!("Body part: {}", self.body_part));
        [
            plane,
            acquired,
            self.family_words()
                .map(|family| format!("Sequence: {family}")),
            measured("Repetition time", self.repetition_ms, " ms"),
            measured("Echo time", self.echo_ms, " ms"),
            measured("Inversion time", self.inversion_ms, " ms"),
            measured("Flip angle", self.flip_deg, "°"),
            measured("Field strength", self.field_t, " T"),
            body,
        ]
        .into_iter()
        .flatten()
        .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const AXIAL: [f64; 6] = [1.0, 0.0, 0.0, 0.0, 1.0, 0.0];

    /// The axial plane tipped about the left-to-right axis by this many degrees.
    fn tipped(degrees: f64) -> [f64; 6] {
        let (sin, cos) = degrees.to_radians().sin_cos();
        [1.0, 0.0, 0.0, 0.0, cos, sin]
    }

    #[test]
    fn the_plane_is_the_usual_one_the_slices_lie_nearest() {
        let plane = |nearest| Plane {
            nearest,
            oblique: false,
        };
        assert_eq!(Plane::of(AXIAL), Some(plane(Facing::Axial)));
        let coronal = [1.0, 0.0, 0.0, 0.0, 0.0, -1.0];
        assert_eq!(Plane::of(coronal), Some(plane(Facing::Coronal)));
        let sagittal = [0.0, 1.0, 0.0, 0.0, 0.0, -1.0];
        assert_eq!(Plane::of(sagittal), Some(plane(Facing::Sagittal)));
        assert_eq!(Plane::of(tipped(10.0)), Some(plane(Facing::Axial)));
    }

    #[test]
    fn slices_tipped_well_away_from_a_plane_are_oblique() {
        let oblique = Plane {
            nearest: Facing::Axial,
            oblique: true,
        };
        assert_eq!(Plane::of(tipped(20.0)), Some(oblique));
        assert_eq!(oblique.words(), "Oblique axial");
        assert_eq!(
            Plane::of(tipped(70.0)).map(Plane::words),
            Some("Oblique coronal".to_owned())
        );
        assert_eq!(Plane::of([0.0; 6]), None, "no plane at all");
        assert_eq!(
            Plane::of([1.0, 0.0, 0.0, 2.0, 0.0, 0.0]),
            None,
            "one line, not a plane"
        );
    }

    fn facts(family: &[Family]) -> ScanFacts {
        ScanFacts {
            family: family.to_vec(),
            ..ScanFacts::default()
        }
    }

    #[test]
    fn the_short_line_gives_plane_dimensions_and_family() {
        let whole = ScanFacts {
            plane: Plane::of(AXIAL),
            in_3d: Some(true),
            ..facts(&[Family::GradientEcho])
        };
        assert_eq!(whole.line().as_deref(), Some("Axial · 3D · gradient echo"));
        let slices = ScanFacts {
            in_3d: Some(false),
            ..facts(&[Family::InversionRecovery, Family::SpinEcho])
        };
        assert_eq!(
            slices.line().as_deref(),
            Some("2D · inversion-recovery spin echo")
        );
        assert_eq!(
            ScanFacts::default().line(),
            None,
            "nothing known, nothing said"
        );
    }

    #[test]
    fn families_are_said_together() {
        let said = |family: &[Family]| facts(family).line();
        assert_eq!(said(&[Family::SpinEcho]).as_deref(), Some("spin echo"));
        assert_eq!(
            said(&[Family::EchoPlanar, Family::SpinEcho]).as_deref(),
            Some("echo-planar spin echo")
        );
        assert_eq!(said(&[Family::EchoPlanar]).as_deref(), Some("echo-planar"));
        assert_eq!(
            said(&[Family::InversionRecovery]).as_deref(),
            Some("inversion recovery")
        );
        assert_eq!(
            said(&[Family::SpinEcho, Family::GradientEcho]).as_deref(),
            Some("spin and gradient echo")
        );
    }

    #[test]
    fn the_long_list_gives_each_fact_with_its_unit_and_leaves_out_what_is_absent() {
        let known = ScanFacts {
            plane: Plane::of(tipped(20.0)),
            in_3d: Some(false),
            repetition_ms: Some(2345.0),
            echo_ms: Some(67.0),
            inversion_ms: Some(890.0),
            flip_deg: Some(123.0),
            field_t: Some(7.0),
            body_part: "KNEE".to_owned(),
            ..facts(&[Family::InversionRecovery, Family::SpinEcho])
        };
        assert_eq!(
            known.lines(),
            [
                "Plane: oblique, nearest axial",
                "Acquired slice by slice (2D)",
                "Sequence: inversion-recovery spin echo",
                "Repetition time 2345 ms",
                "Echo time 67 ms",
                "Inversion time 890 ms",
                "Flip angle 123°",
                "Field strength 7 T",
                "Body part: KNEE",
            ]
        );
        let little = ScanFacts {
            in_3d: Some(true),
            echo_ms: Some(2.125),
            ..ScanFacts::default()
        };
        assert_eq!(
            little.lines(),
            ["Acquired as one block (3D)", "Echo time 2.13 ms"]
        );
        assert_eq!(ScanFacts::default().lines(), Vec::<String>::new());
    }
}
