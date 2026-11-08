// pxl-core/domain/scene/decor/param.rs

//! The tiers decor is drawn in. By how much of a camera grid cell it spans, a
//! decor is drawn as a model, as an impostor, or not at all.

/// A decor is drawn as a model while it spans at least this many camera grid
/// cells, and as an impostor below that. At two cells a model is drawn down
/// to about 22 px.
pub const DECOR_MODEL_CELLS: f64 = 2.0;

/// A decor's impostor is not drawn when it spans less than this share of a
/// camera grid cell, because an instance that small reads as pixel noise. The
/// value was chosen by eye.
pub const DECOR_IMPOSTOR_CELLS: f64 = 0.25;
