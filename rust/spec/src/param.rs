// pxl-spec/param.rs

//! What the world shows of its places and divisions, as this layer sets
//! it: each value one literal, a subject a module, its reason beside it.
//! What follows from them is computed by name where it is used.

pub mod marker {
    use crate::space::coord::zoom::Zoom;

    /// A mark's cell: 16 pixels of the web map's 256-pixel tile, four zooms
    /// finer than the tile, the cell Carto keeps its 14-pixel icons apart in.
    pub const STEPS: Zoom = 4;
}

pub mod place {
    use crate::space::coord::zoom::Zoom;

    /// How far apart places stand, in zooms: an icon's cell is this many
    /// coarser than the camera grid's, and the cell a place keeps to itself
    /// this many coarser again. At 2 an icon's cell spans four grid cells round
    /// the drawn icon, and a place keeps four icons a side, sixteen cells; at 1
    /// places stood too close for their names. A product choice.
    pub const SPREAD_STEPS: Zoom = 2;
}
