// pxl-core/domain/scene/indicator/param.rs

//! The indicators' tuned numbers: the location's zone and beacon, and the
//! ghost's opacity.

/// How far the zone reaches from its center, in world units. It does not
/// change with the size of the ground's cell.
pub(crate) const ZONE_FOOTPRINT_U: f64 = 4.0;

pub(crate) const BEACON_Z_BIAS: f64 = 0.5;
pub(crate) const BEACON_SMOOTH_RATE: f64 = 6.0;
/// How far the beacon bobs either way, in units.
pub(crate) const BEACON_BOB: f64 = 0.1;
/// How fast the beacon bobs, in radians of its swing a second.
pub(crate) const BEACON_BOB_RATE: f64 = 2.0;
/// How fast the beacon spins, in radians a second.
pub(crate) const BEACON_SPIN_RATE: f64 = 1.2;

/// The opacity the ghost is drawn at.
pub(crate) const GHOST_OPACITY: f32 = 0.5;
