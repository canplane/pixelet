// pxl-core/domain/param.rs

//! The tuning the scene and control share. Each head keeps its own knobs,
//! and a value both must agree on lives here, the lowest module both read.

/// The rate at which an eased position approaches its target, per second
/// (smooth.rs `smoothing_factor`). The camera and the characters ease at the
/// same rate.
pub const POS_DAMP: f64 = 10.0;
