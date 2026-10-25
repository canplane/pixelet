// pxl-kernel/domain/param.rs

//! What both heads tune alike: the scene's and control's knobs are each
//! their own, and the one they share is here, the lowest place both read.

/// The rate a position eases at (approach.rs), the camera's and the
/// companion's alike.
pub const POS_DAMP: f64 = 10.0;
