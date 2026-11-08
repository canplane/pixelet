// pxl-spec/param.rs

//! The values this crate sets for how the world shows its places, each one literal with its
//! reason beside it, in a module for its subject. What follows from them is computed by name
//! where it is used.

pub(crate) mod place {
    use crate::space::coord::zoom::Zoom;

    /// How far apart places stand, in zooms: an icon's cell is this many zooms coarser than the
    /// camera grid's, and the cell a place keeps to itself is this many coarser again. At 2, an
    /// icon's cell spans four grid cells around the drawn icon, and a place keeps four icons a
    /// side, sixteen cells; at 1, places stand too close for their names. It is a product choice.
    pub const SPREAD_STEPS: Zoom = 2;
}
