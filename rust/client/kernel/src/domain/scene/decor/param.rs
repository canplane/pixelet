// pxl-kernel/domain/scene/decor/param.rs

//! Decor's tiers: by how much a kind covers, a model, a sprite or nothing.

/// A decor kind is drawn as a model while it covers at least this many pixels
/// at the reference view: by eye, where Google's maps stop drawing buildings
/// in 3D.
pub const DECOR_MODEL_PX: f64 = 12.0;

/// Under this share of a camera grid cell a decor kind's sprite is not drawn:
/// set by eye, below it an instance is pixel noise; about three CSS pixels
/// at the reference view.
pub const DECOR_SPRITE_CELLS: f64 = 0.25;
