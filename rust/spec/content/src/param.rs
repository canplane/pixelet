// pxl-content/param.rs

//! gpu-portable: what pixelet's content is, as it sets it: each value one
//! literal, a subject a module, its reason beside it, as the engine's
//! (pxl-engine param.rs) are.

pub mod cloud {
    /// A lattice unit is 2^12 cells, about 8 km at mid latitudes; a power of two
    /// so the clouds wrap with the map.
    pub const LATTICE_SHAMT: u32 = 12;

    /// In cells a second, about 50 m/s at mid latitudes, a jet stream's: at a
    /// breeze's 8 a cloud a kilometer wide takes minutes to cross its width and
    /// reads as still. The field moves against it, so a point on the ground sees
    /// the field at itself plus the drift.
    pub const WIND: [f64; 2] = [24.0, 10.0];

    /// A fair-weather cumulus base.
    pub const CUMULUS_KM: f64 = 1.5;
}

pub(crate) mod cave {
    /// The tunnels every root cube (256 units) holds, by class: one 256
    /// across, four 128, fourteen 64 and forty-eight 32; within what a root
    /// can start (cave.rs `room`), so every coarser node is the same share.
    pub const ROOT_TUNNELS: [u32; 4] = [1, 4, 14, 48];
    /// The 2-unit blocks a tunnel of each class hollows: thirty a pipe cell
    /// of the node's side, under the 32 a side's pipe cells can take.
    pub const BLOCKS_A_TUNNEL: [u32; 4] = [960, 480, 240, 120];
    /// How far a block's own hash moves it from its line, in squared
    /// sixteenths of a unit: walls that do not run smooth.
    pub const NUDGE: u32 = 1200;
}

pub(crate) mod strata {
    /// A layer of ground cells under a face, in leaf cells, before the stone.
    pub const FILLER_DEPTH: i32 = 3;

    /// Sandstone under a sand or beach face's filler, in leaf cells.
    pub const SANDSTONE_DEPTH: i32 = 2;

    /// The bedrock's top under a face, this far under it or under sea level,
    /// whichever is lower, in leaf cells (strata.rs `bedrock`).
    pub const BEDROCK_DEPTH: i32 = 64;

    /// Leaf cells over the ground or sea level, whichever is higher, a cube
    /// may reach: Minecraft's 320 over its −64, held from sea level so a shore
    /// is not buried.
    pub const HEADROOM: i32 = 128;
}

pub(crate) mod scatter {
    /// How far about a cell its turn is voted: a fixed radius (O(1) reads),
    /// enough to outvote a one-cell stairstep jog in a straight run.
    pub const ALIGN_NEIGHBOR_RADIUS: i32 = 3;
}
