//! Typed coordinate spaces: points and transforms that remember which space they belong to.
//!
//! A position can be given in the patient (millimetres, DICOM left-posterior-superior), in the
//! image grid (column, row, slice) or on the screen (pixels). Mixing them up is the classic way
//! to show an image mirrored, so each space is its own type and a transform only accepts points
//! from the space it maps from.
//!
//! ```compile_fail
//! use voxa_core::{Affine, Patient, Point, Screen, Voxel};
//! let voxel_to_patient: Affine<Voxel, Patient> = Affine::identity();
//! let on_screen: Point<Screen> = Point::new(10.0, 20.0, 0.0);
//! let _ = voxel_to_patient.apply(on_screen); // a screen point is not a voxel position
//! ```

use std::marker::PhantomData;

/// Patient space: millimetres, DICOM left-posterior-superior. +x is the patient's left, +y the
/// back, +z the head.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Patient;

/// Image-grid space: (column, row, slice), counted from zero at voxel centres.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Voxel;

/// Screen space: pixels in a viewport, x to the right and y down. `z` is 0.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Screen;

/// A position in the space `S`.
pub struct Point<S> {
    /// First coordinate.
    pub x: f64,
    /// Second coordinate.
    pub y: f64,
    /// Third coordinate; 0.0 for [`Screen`] points.
    pub z: f64,
    space: PhantomData<S>,
}

// Written out instead of derived: a derive would demand the same trait of the marker type `S`,
// and a point is plain numbers whatever space it is in.
impl<S> Clone for Point<S> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<S> Copy for Point<S> {}

impl<S> PartialEq for Point<S> {
    fn eq(&self, other: &Self) -> bool {
        self.x == other.x && self.y == other.y && self.z == other.z
    }
}

impl<S> std::fmt::Debug for Point<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Point({}, {}, {})", self.x, self.y, self.z)
    }
}

impl<S> Point<S> {
    /// A point at the given coordinates.
    pub fn new(x: f64, y: f64, z: f64) -> Self {
        Self {
            x,
            y,
            z,
            space: PhantomData,
        }
    }
}

/// A transform taking points in `From` to points in `To`: a 3×3 linear part and a translation.
pub struct Affine<From, To> {
    /// Images of the three unit axes of `From`, each expressed in `To`.
    axes: [[f64; 3]; 3],
    /// Image of the origin of `From`.
    origin: [f64; 3],
    spaces: PhantomData<(From, To)>,
}

impl<A, B> Clone for Affine<A, B> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<A, B> Copy for Affine<A, B> {}

impl<A, B> PartialEq for Affine<A, B> {
    fn eq(&self, other: &Self) -> bool {
        self.axes == other.axes && self.origin == other.origin
    }
}

impl<A, B> std::fmt::Debug for Affine<A, B> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Affine")
            .field("axes", &self.axes)
            .field("origin", &self.origin)
            .finish()
    }
}

impl<A, B> Affine<A, B> {
    /// The transform that sends the unit x, y and z axes of `A` to the given vectors of `B`
    /// and the origin of `A` to `origin`.
    pub fn from_columns(x: [f64; 3], y: [f64; 3], z: [f64; 3], origin: [f64; 3]) -> Self {
        Self {
            axes: [x, y, z],
            origin,
            spaces: PhantomData,
        }
    }

    /// The transform that changes nothing.
    pub fn identity() -> Self {
        Self::from_columns([1., 0., 0.], [0., 1., 0.], [0., 0., 1.], [0., 0., 0.])
    }

    /// The images of the unit x, y and z axes, as given to [`Affine::from_columns`].
    pub fn columns(&self) -> [[f64; 3]; 3] {
        self.axes
    }

    /// The image of the origin.
    pub fn origin(&self) -> [f64; 3] {
        self.origin
    }

    /// Move a point from `A` to `B`.
    pub fn apply(&self, p: Point<A>) -> Point<B> {
        let [x, y, z] = self.axes;
        let [ox, oy, oz] = self.origin;
        Point::new(
            x[0] * p.x + y[0] * p.y + z[0] * p.z + ox,
            x[1] * p.x + y[1] * p.y + z[1] * p.z + oy,
            x[2] * p.x + y[2] * p.y + z[2] * p.z + oz,
        )
    }

    /// The transform back from `B` to `A`, or `None` when this one flattens space (a zero or
    /// repeated axis) and so cannot be undone.
    pub fn inverse(&self) -> Option<Affine<B, A>> {
        let [a, b, c] = self.axes;
        // Rows of the inverse are cross products of the columns, divided by the determinant.
        let r0 = cross(b, c);
        let r1 = cross(c, a);
        let r2 = cross(a, b);
        let det = dot(a, r0);
        let scale = norm(a) * norm(b) * norm(c);
        if !det.is_finite() || det.abs() <= scale * 1e-12 {
            return None;
        }
        let rows = [r0, r1, r2].map(|r| r.map(|v| v / det));
        let [r0, r1, r2] = rows;
        let origin = [
            -dot(r0, self.origin),
            -dot(r1, self.origin),
            -dot(r2, self.origin),
        ];
        Some(Affine::from_columns(
            [r0[0], r1[0], r2[0]],
            [r0[1], r1[1], r2[1]],
            [r0[2], r1[2], r2[2]],
            origin,
        ))
    }

    /// This transform followed by `next`: from `A` straight to `C`.
    pub fn then<C>(&self, next: &Affine<B, C>) -> Affine<A, C> {
        let through = |v: [f64; 3]| {
            let [x, y, z] = next.axes;
            [
                x[0] * v[0] + y[0] * v[1] + z[0] * v[2],
                x[1] * v[0] + y[1] * v[1] + z[1] * v[2],
                x[2] * v[0] + y[2] * v[1] + z[2] * v[2],
            ]
        };
        let [ox, oy, oz] = self.origin;
        let moved = next.apply(Point::<B>::new(ox, oy, oz));
        let [x, y, z] = self.axes;
        Affine::from_columns(
            through(x),
            through(y),
            through(z),
            [moved.x, moved.y, moved.z],
        )
    }
}

fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn norm(a: [f64; 3]) -> f64 {
    dot(a, a).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn close(a: Point<Voxel>, b: Point<Voxel>) -> bool {
        (a.x - b.x).abs() < 1e-9 && (a.y - b.y).abs() < 1e-9 && (a.z - b.z).abs() < 1e-9
    }

    #[test]
    fn affine_maps_voxel_to_patient() {
        let a: Affine<Voxel, Patient> = Affine::from_columns(
            [0.9, 0., 0.],
            [0., 0.9, 0.],
            [0., 0., 5.],
            [-100., -120., 0.],
        );
        let p = a.apply(Point::new(10., 20., 3.));
        assert_eq!((p.x, p.y, p.z), (-91.0, -102.0, 15.0));
    }

    #[test]
    fn inverse_round_trips() {
        let a: Affine<Voxel, Patient> = Affine::from_columns(
            [0.7, 0.02, 0.0],
            [-0.02, 0.69, 0.1],
            [0.0, -0.8, 7.4],
            [-90.5, -110.0, -31.5],
        );
        let back = a.inverse().expect("an oblique scan grid is invertible");
        let start = Point::<Voxel>::new(120.0, 300.5, 11.0);
        assert!(close(back.apply(a.apply(start)), start));
    }

    #[test]
    fn singular_affine_has_no_inverse() {
        let flat: Affine<Voxel, Patient> =
            Affine::from_columns([1., 0., 0.], [0., 1., 0.], [0., 0., 0.], [5., 5., 5.]);
        assert!(flat.inverse().is_none());
    }

    #[test]
    fn columns_and_origin_are_read_back() {
        let a: Affine<Voxel, Patient> =
            Affine::from_columns([1., 2., 3.], [4., 5., 6.], [7., 8., 9.], [10., 11., 12.]);
        assert_eq!(a.columns(), [[1., 2., 3.], [4., 5., 6.], [7., 8., 9.]]);
        assert_eq!(a.origin(), [10., 11., 12.]);
    }

    #[test]
    fn identity_leaves_a_point_alone() {
        let p = Affine::<Voxel, Voxel>::identity().apply(Point::new(3.5, -2.0, 7.0));
        assert_eq!((p.x, p.y, p.z), (3.5, -2.0, 7.0));
    }

    fn column() -> impl Strategy<Value = [f64; 3]> {
        [-10.0..10.0f64, -10.0..10.0f64, -10.0..10.0f64]
    }

    proptest! {
        #[test]
        fn then_is_the_same_as_applying_one_after_the_other(
            ax in column(), ay in column(), az in column(), ao in column(),
            bx in column(), by in column(), bz in column(), bo in column(),
            p in column(),
        ) {
            let a: Affine<Voxel, Patient> = Affine::from_columns(ax, ay, az, ao);
            let b: Affine<Patient, Screen> = Affine::from_columns(bx, by, bz, bo);
            let [x, y, z] = p;
            let point = Point::<Voxel>::new(x, y, z);
            let joined = a.then(&b).apply(point);
            let stepwise = b.apply(a.apply(point));
            prop_assert!((joined.x - stepwise.x).abs() < 1e-6);
            prop_assert!((joined.y - stepwise.y).abs() < 1e-6);
            prop_assert!((joined.z - stepwise.z).abs() < 1e-6);
        }
    }
}
