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
