// pxl-engine/param.rs

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

    /// The margin a beam ray's radius takes over a cube's, `√6 · s`.
    pub(crate) const RADIUS_SLACK: f32 = 1.01;
}

pub mod raster {
    /// Drawn pixels a face is grown by in its own plane, at each corner's
    /// distance: so a pixel's middle on its edge is not lost to the
    /// rasterizer's rounding. The face stays in its plane, so a pixel's depth
    /// is still where its ray meets the face, and what the growth alone covers
    /// fails the fragment's exact test (raster.wgsl `fs`): it costs fragments
    /// and changes no visible sample.
    pub const GROWTH: f32 = 0.25;
}

pub(crate) mod scatter {
    /// How far about a cell its turn is voted: a fixed radius (O(1) reads),
    /// enough to outvote a one-cell stairstep jog in a straight run.
    pub const ALIGN_NEIGHBOR_RADIUS: i32 = 3;
}

pub mod light {
    /// The slope a caster's depth in the shadow map is pushed back by, as a
    /// rasterizer's polygon offset: its greatest slope, in depth a texel
    /// (light.rs `slope`), times this. The raster casters' depth pipelines take it
    /// (pxl-render record.rs `depth_only`), and the world's cast adds it
    /// (pxl-shader `light_cs`), so the two kinds of caster shadow alike. A
    /// receiver compares each tap at its own plane's depth there (pxl-render
    /// shadow.wgsl), so the push covers only the texels a tap filters, a texel
    /// each way, along the steeper axis; the receiver's texel of normal offset
    /// covers the other. At 2, with each tap compared at the receiver's own
    /// depth, a step a unit high shadowed the ground below it in stripes a
    /// texel apart: the taps up the slope saw past the step.
    pub const SLOPE_BIAS: f32 = 1.0;

    /// The shadow map's slots between samples, across and down (light.rs
    /// `samples_at`).
    pub const SAMPLE: u32 = 4;

    /// The receiver's taps apart in the shadow map, in texels, a 3×3 grid
    /// (pxl-render shadow.wgsl `shadow_visibility`): the filtered edge spans
    /// about four texels, half a unit where a texel is 1/8 u, where taps a
    /// texel apart cut each cell's shadow out as a hard square.
    pub const SPREAD: f32 = 1.5;
}

pub mod way {
    // From louis-e/arnis `c96872e`, Copyright (c) 2022-2026 Louis Erbkamm, under
    // the Apache License, Version 2.0 (THIRD_PARTY_NOTICES.md); changed to stand
    // on a deck's line rather than place Minecraft blocks.
    // A bridge the source names no structure for is Arnis's beam bridge
    // (`pier_interval`, `place_pier_bent`), of one-cell columns.

    /// The farthest past half its width a line is drawn, in its tile's cells.
    pub(crate) const LINE_PAST: f32 = 1.0;

    /// A deck's slab under its surface, in ground meters: Arnis's
    /// `DECK_STRUCTURE_DEPTH`.
    pub(crate) const DECK_THICKNESS_M: f32 = 2.0;

    /// Clear height a deck keeps over a deck under it, in ground meters: Arnis's
    /// `STACKED_DECK_HEADROOM`.
    pub const STACKED_DECK_HEADROOM_M: f32 = 6.0;

    /// A beam bridge's bents apart, in ground meters: Arnis's `pier_interval`.
    pub(crate) const BEAM_PILLAR_INTERVAL_M: f32 = 8.0;

    /// A beam bridge's bents apart once its half width passes two cells, in
    /// ground meters: Arnis's `pier_interval`.
    pub(crate) const WIDE_BEAM_PIER_INTERVAL_M: f32 = 12.0;

    /// Arnis's `place_pillar`: a deck this near the ground rests on it.
    pub(crate) const PIER_MIN_RISE_M: f32 = 2.0;
}
