// pxl-spec/param.rs

//! The values this crate sets for how the world shows its places and how its ways stand, each
//! one literal with its reason beside it, in a module for its subject. What follows from them
//! is computed by name where it is used.

pub(crate) mod place {
    use crate::space::coord::zoom::Zoom;

    /// How far apart places stand, in zooms: an icon's cell is this many zooms coarser than the
    /// camera grid's, and the cell a place keeps to itself is this many coarser again. At 2, an
    /// icon's cell spans four grid cells around the drawn icon, and a place keeps four icons a
    /// side, sixteen cells; at 1, places stand too close for their names. It is a product choice.
    pub(crate) const SPREAD_STEP_CNT: Zoom = 2;
}

pub mod way {
    // From louis-e/arnis `c96872e`, Copyright (c) 2022-2026 Louis Erbkamm, under
    // the Apache License, Version 2.0 (THIRD_PARTY_NOTICES.md); changed to stand
    // on a deck's line rather than place Minecraft blocks.
    // A bridge whose source names no structure is drawn as Arnis's beam bridge
    // (`pier_interval`, `place_pier_bent`), standing on one-cell columns.

    /// How far past half its width a line is drawn, in its tile's cells.
    pub(crate) const LINE_PAST: f32 = 1.0;

    /// The thickness of a deck's slab under its surface, in ground meters (Arnis's
    /// `DECK_STRUCTURE_DEPTH`).
    pub const DECK_THICKNESS_M: f32 = 2.0;

    /// The clear height a deck keeps over a deck under it, in ground meters (Arnis's
    /// `STACKED_DECK_HEADROOM`).
    pub const STACKED_DECK_HEADROOM_M: f32 = 6.0;

    /// The spacing of a beam bridge's bents, in ground meters (Arnis's `pier_interval`).
    pub(crate) const BEAM_PILLAR_INTERVAL_M: f32 = 8.0;

    /// The spacing of a beam bridge's bents once its half width passes two cells, in ground
    /// meters (Arnis's `pier_interval`).
    pub(crate) const WIDE_BEAM_PIER_INTERVAL_M: f32 = 12.0;

    /// A deck this near the ground, in ground meters, rests on it (Arnis's `place_pillar`).
    pub const PIER_MIN_RISE_M: f32 = 2.0;
}
