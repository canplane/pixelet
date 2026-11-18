// pxl-content/param.rs

//! wgsl-port: the values pixelet's content sets, each one literal with its reason beside
//! it, in a module for its subject, as the engine's are (pxl-engine param.rs).

pub mod cloud {
    /// A lattice unit is 2^12 cells, about 8 km at mid latitudes; it is a power of two, so the
    /// clouds wrap with the map.
    pub const LATTICE_SHAMT: u32 = 12;

    /// The wind in cells a second, about 50 m/s at mid latitudes, a jet stream's speed. At a
    /// breeze's 8, a cloud a kilometer wide takes minutes to cross its own width and reads as
    /// still. The field moves against the wind, so a point on the ground sees the field at itself
    /// plus the drift.
    pub const WIND: [f64; 2] = [24.0, 10.0];

    /// A fair-weather cumulus base.
    pub const CUMULUS_KM: f64 = 1.5;
}

pub mod night {
    /// The sine of the sun's height across which night falls, wherever night is shown: the stars
    /// over the sky (pxl-render look/environment.rs), the cities' lights over the land
    /// (atmos_air.wgsl `city_shown`), and the night the core tells (pxl-core
    /// domain/scene/environment.rs `night`). Night is whole at the first value and gone at the
    /// second, the horizon.
    pub const FALL: [f32; 2] = [-0.1, 0.0];
}

pub mod cave {
    /// The tunnels every root cube (256 units) holds, by class: one 256 across, four 128, sixteen
    /// 64, sixty-four 32 and a hundred and sixty 16. The last is under two thirds of the 256 the
    /// root could start, so those tunnels lie scattered. All are within what a root can start
    /// (cave.rs `cap`), so every coarser node holds the same share.
    pub const ROOT_TUNNELS: [u32; 5] = [1, 4, 16, 64, 160];
    /// The 2-unit blocks a tunnel of each class hollows: forty-eight for each pipe cell of its
    /// node's side, fewer than the 64, at the least, that a pipe cell holds of the cells it
    /// crosses.
    pub const BLOCKS_A_TUNNEL: [u32; 5] = [1536, 768, 384, 192, 96];
    /// How far a block's own hash moves it from its line, in squared
    /// eighths of a unit: walls that do not run smooth.
    pub const NUDGE: u32 = 1200;
}

pub mod strata {
    /// A layer of ground cells under the ground's top cell, in leaf cells, before the stone.
    pub const FILLER_DEPTH: i32 = 3;

    /// Sandstone under a sand or beach face's filler, in leaf cells.
    pub const SANDSTONE_DEPTH: i32 = 2;

    /// The bedrock's top under the ground, this far under its elevation or under sea level,
    /// whichever is lower, in leaf cells (strata.rs `bedrock`).
    pub const BEDROCK_DEPTH: i32 = 64;

    /// How many leaf cells above the ground or sea level, whichever is higher, a cube may reach:
    /// Minecraft's 320 over its −64, counted from sea level too, so that a shore is not buried.
    pub const HEADROOM: i32 = 128;
}

pub mod scatter {
    /// How far around a cell its turn is voted: a fixed radius, so a vote takes O(1) reads, wide
    /// enough to outvote a one-cell stairstep jog in a straight run.
    pub const ALIGN_NEIGHBOR_RADIUS: i32 = 3;
}
