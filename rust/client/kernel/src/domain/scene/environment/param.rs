// pxl-kernel/domain/scene/environment/param.rs

//! The sky's and the clouds' tuned numbers.

// ---- the sky ----

/// The sun and moon follow a jump of the anchor, and the stars fade in, over
/// 1/SKY_EASE_RATE s, and never step.
pub const SKY_EASE_RATE: f64 = 1.5;

// ---- the clouds ----

/// A cloud's width in units, at the least and the most: about 2 to 3.5 km,
/// whatever the spacing.
pub const CLOUD_WIDTH: [f64; 2] = [1024.0, 1840.0];
/// The share of cells holding a cloud, the same everywhere: tied to the
/// field, the cumulus gathered in clumps with bare country between.
pub const CLOUD_SHARE: f64 = 0.5;
pub const CLOUD_SALT: u32 = 0x0c0c;
/// The fewest of the view's camera cells a cell of the field spans as it is
/// drawn: past where it spans fewer, each octave farther shows a quarter of
/// the clouds, the same quarter at every distance, so a cloud seen nearer
/// stays. Whole, the field ran to the horizon a cloud a cell, a band of dots.
/// Set by eye.
pub const CLOUD_SPACING_CELLS: f64 = 16.0;
/// The share of a camera grid cell the clouds drift a step, at the nearest
/// drawn, set by eye: a twelfth of a cell, 1/1152 rad, is not seen to move
/// by itself, and a whole cell's step at once was seen as the clouds
/// stopping and jumping.
pub const CLOUD_DRIFT_CELLS: f64 = 1.0 / 12.0;
/// Where the clouds are gone, faded from half of it: a product number.
pub const CLOUD_GONE_KM: f64 = 64.0;
