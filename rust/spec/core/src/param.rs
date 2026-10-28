// pxl-spec-core/param.rs

//! gpu-portable: what the world is and how it is observed and drawn, as
//! this layer sets it: each value one literal, a subject a module, its
//! reason beside it. What follows from them is computed by name where it is
//! used.

pub mod beam {
    /// Samples a camera grid cell a side: the lattice whose stops a pixel takes
    /// the nearest of (beam.rs `SAMPLE_NODE`).
    pub const SAMPLE: u32 = 4;

    /// Samples past the screen on each side, so a cell seen at the screen's edge
    /// is one a sample stopped at.
    pub const NEAR: u32 = 2;

    /// The margin a beam ray's reach takes over a cube's, `√6 · s`.
    pub const REACH_SLACK: f32 = 1.01;
}

pub mod cave {
    use crate::Zoom;

    /// The chance a drawing cube turns one of its eight children over, as the
    /// hash's share under it: a quarter. With each child drawing again at even
    /// odds (one hash bit), some 6% of the ground is hollow.
    pub const TURN: u32 = 1 << 30;

    /// The coarsest cube a cave takes, in zoom steps over the leaf cell: 32
    /// units a side; nothing coarser is hollow, so every coarser fold of the
    /// caves is solid.
    pub const COARSEST_STEPS: Zoom = 5;
}

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

pub mod ground {
    /// A layer of ground cells under a face, in leaf cells, before the stone.
    pub const FILLER_DEPTH: i32 = 3;

    /// Sandstone under a sand or beach face's filler, in leaf cells.
    pub const SANDSTONE_DEPTH: i32 = 2;

    /// The bedrock's top under a face, this far under it or under sea level,
    /// whichever is lower, in leaf cells (ground.rs `bedrock`).
    pub const BEDROCK_DEPTH: i32 = 64;

    /// Leaf cells over the ground or sea level, whichever is higher, a cube
    /// may reach: Minecraft's 320 over its −64, held from sea level so a shore
    /// is not buried.
    pub const HEADROOM: i32 = 128;
}

pub mod raster {
    /// Drawn pixels a face is grown by in its own plane, at each corner's
    /// distance: so a pixel's middle on its edge is not lost to the
    /// rasterizer's rounding. The face stays in its plane, so a pixel's depth
    /// is still where its ray meets the face, and what the growth alone covers
    /// fails the fragment's exact test (raster.wgsl `fs`): it costs fragments
    /// and changes no pick.
    pub const GROWN: f32 = 0.25;
}

pub mod scatter {
    /// How far about a cell its turn is voted: a fixed radius (O(1) reads, no
    /// flood fill), enough to outvote a one-cell stairstep jog in a straight run;
    /// it does not resolve curves.
    pub const ALIGN_NEIGHBOR_RADIUS: i32 = 3;
}

pub mod sun {
    /// The slope a caster's depth in the shadow map is pushed back by, as a
    /// rasterizer's polygon offset: its greatest slope, in depth a texel
    /// (sun.rs `slope`), times this. The raster casters' depth pipelines take it
    /// (pxl-render record.rs `depth_only`), and the world's cast adds it
    /// (pxl-shader `sun_cs`), so the two kinds of caster shadow alike. A
    /// receiver compares each tap at its own plane's depth there (pxl-render
    /// shadow.wgsl), so the push covers only the texels a tap filters, a texel
    /// each way, along the steeper axis; the receiver's texel of normal offset
    /// covers the other. At 2, with each tap compared at the receiver's own
    /// depth, a step a unit high shadowed the ground below it in stripes a
    /// texel apart: the taps up the slope saw past the step.
    pub const SLOPE_BIAS: f32 = 1.0;

    /// The shadow map's slots between samples, across and down (sun.rs
    /// `samples_at`).
    pub const SAMPLE: u32 = 4;

    /// The receiver's taps apart in the shadow map, in texels, a 3×3 grid
    /// (pxl-render shadow.wgsl `sun_lit`): the filtered edge spans about
    /// four texels, half a unit where a texel is 1/8 u, where taps a texel
    /// apart cut each cell's shadow out as a hard square.
    pub const SPREAD: f32 = 1.5;
}

pub mod way {
    // From louis-e/arnis `c96872e`, Copyright (c) 2022-2026 Louis Erbkamm, under
    // the Apache License, Version 2.0 (THIRD_PARTY_NOTICES.md); changed to stand
    // on a deck's line rather than place Minecraft blocks.
    // A bridge the source names no structure for is Arnis's beam bridge
    // (`pier_interval`, `place_pier_bent`), of one-cell columns.

    /// The farthest past half its width a line is drawn, in its tile's cells.
    pub const LINE_PAST: f32 = 1.0;

    /// A deck's slab under its surface, in ground metres: Arnis's
    /// `DECK_STRUCTURE_DEPTH`.
    pub const DECK_THICKNESS_M: f32 = 2.0;

    /// Clear height a deck keeps over a deck under it, in ground metres: Arnis's
    /// `STACKED_DECK_HEADROOM`.
    pub const STACKED_DECK_HEADROOM_M: f32 = 6.0;

    /// A beam bridge's bents apart, in ground metres: Arnis's `pier_interval`.
    pub const BEAM_PILLAR_INTERVAL_M: f32 = 8.0;

    /// A beam bridge's bents apart once its half width passes two cells, in
    /// ground metres: Arnis's `pier_interval`.
    pub const WIDE_BEAM_PIER_INTERVAL_M: f32 = 12.0;

    /// Arnis's `place_pillar`: a deck this near the ground rests on it.
    pub const PIER_MIN_RISE_M: f32 = 2.0;
}
