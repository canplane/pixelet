// pxl-core/domain/scene/gizmo/param.rs

//! The frame gizmo's tuned numbers: the size of a point's cross and of the
//! rings about the pivot.

/// Half the length of each arm of the cross a point is drawn as.
pub(crate) const POINT_HALF: f64 = 0.15;

/// The radius of the rings drawn about the pivot, in units.
pub(crate) const RING_RADIUS: f64 = 4.0;
