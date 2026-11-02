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
    use pxl_engine::Zoom;

    /// The tunnels' lattice, as powers of two of units across and up: 32
    /// units across and 16 up, so a tunnel winds more across than up.
    pub const TUNNEL_SHIFT: [u32; 2] = [5, 4];
    /// The caverns' lattice: 64 across, 32 up.
    pub const CAVERN_SHIFT: [u32; 2] = [6, 5];
    /// The detail's lattice, the finest: 8 units every way.
    pub const DETAIL_SHIFT: [u32; 2] = [3, 3];
    /// The detail's span against a noise's, in halvings: a quarter.
    pub const DETAIL_SHRINK: u32 = 2;
    /// The band about zero both tunnel noises fall in where a tunnel is, of
    /// a noise's about `±2^14`.
    pub const TUNNEL_BAND: i32 = 1200;
    /// What the caverns' noise passes where a cavern is, of its about `±2^14`.
    pub const CAVERN_OVER: i32 = 7000;

    /// The coarsest cube a cave takes, in zoom steps over the leaf cell: 32
    /// units a side; nothing coarser is hollow, so every coarser fold of the
    /// caves is solid.
    pub const COARSEST_STEPS: Zoom = 5;
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
