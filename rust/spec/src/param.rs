// pxl-spec/param.rs

//! How the world shows its places, as this layer sets it: each value one
//! literal, a subject a module, its reason beside it. What follows from
//! them is computed by name where it is used.

pub mod place {
    use crate::space::coord::zoom::Zoom;

    /// Carto's last zoom, openstreetmap.org's: the leaf places through it.
    pub const LAST: Zoom = 19;

    /// How far apart places stand, in zooms: an icon's cell is this many
    /// coarser than the camera grid's, and the cell a place keeps to itself
    /// this many coarser again. At 2 an icon's cell spans four grid cells round
    /// the drawn icon, and a place keeps four icons a side, sixteen cells; at 1
    /// places stood too close for their names. A product choice.
    pub const SPREAD_STEPS: Zoom = 2;
}
