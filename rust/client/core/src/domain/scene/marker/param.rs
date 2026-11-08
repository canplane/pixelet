// pxl-core/domain/scene/marker/param.rs

//! The markers' tuned numbers: the width a place keeps on the screen, how
//! generously a marker is kept in view, and how long a chat line shows.

/// The width a place keeps to itself on the screen, in CSS pixels. Its name
/// wraps to this width, and its marker's region is this wide. A marker is
/// laid out in CSS pixels, as the UI's text is, so the width looks the same
/// on every screen. The value was chosen by eye to hold a name of a few
/// words.
pub const SPACING_PX: f64 = 193.0;

/// The radius of the sphere tested against the frustum, as a fraction of the
/// distance, since a marker keeps a constant size on the screen. It is
/// generous, because one marker too many costs only a sprite.
pub const ANCHOR_RADIUS: f64 = 0.08;

/// How long a line shows, in ms. The chat has no expiry of its own.
pub const SHOWN_MS: f64 = 8000.0;
