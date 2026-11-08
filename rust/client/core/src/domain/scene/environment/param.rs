// pxl-core/domain/scene/environment/param.rs

//! The sky's and the clouds' tuned numbers.

// ---- the sky ----

/// The rate, per second, at which the sun and the moon ease across a jump of
/// the anchor and the stars fade in: each transition takes 1/SKY_EASE_RATE s.
/// These transitions are eased every frame, never stepped.
pub const SKY_EASE_RATE: f64 = 1.5;

// ---- the clouds ----

/// The least and the most width of a cloud, in units: about 2 to 3.5 km. It
/// does not depend on `CLOUD_SPACING_CELLS`.
pub const CLOUD_WIDTH: [f64; 2] = [1024.0, 1840.0];
/// The share of the cloud field's cells that hold a cloud, the same
/// everywhere. The field itself gathers the cumulus in clumps, with bare
/// country between them.
pub const CLOUD_SHARE: f64 = 0.5;
pub const CLOUD_SALT: u32 = 0x0c0c;
/// The fewest camera grid cells a cell of the cloud field may span as drawn.
/// Farther out, where a field cell would span fewer, each octave of distance
/// shows a quarter of the clouds the octave before it shows. The kept clouds
/// are the same at every distance, so a cloud seen far off is still there
/// when the eye comes nearer. The value was chosen by eye.
pub const CLOUD_SPACING_CELLS: f64 = 16.0;
/// The eye's height, in km, at which the clouds are gone. They begin to fade
/// at half this height. It is a product choice, not a physical value.
pub const CLOUD_GONE_KM: f64 = 64.0;
