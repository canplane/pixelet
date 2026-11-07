// pxl-core/domain/scene/decor/param.rs

//! Decor's tiers: by how much a kind covers, a model, an impostor or culled.

/// A decor kind is drawn as a model while it spans at least this share of a
/// camera grid cell, an impostor below it: set by eye at one cell, moved to
/// two by the user's decision of 2026-11-06, so a model is drawn down to
/// about 22 px.
pub const DECOR_MODEL_CELLS: f64 = 2.0;

/// Under this share of a camera grid cell a decor kind's impostor is not drawn:
/// set by eye, below it an instance is pixel noise.
pub const DECOR_IMPOSTOR_CELLS: f64 = 0.25;
