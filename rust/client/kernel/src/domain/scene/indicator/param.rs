// pxl-kernel/domain/scene/indicator/param.rs

//! The location puck's tuned numbers: its zone's footprint and its beacon.

/// The zone's footprint in world units, whatever the ground's cell.
pub const ZONE_FOOTPRINT_U: f64 = 4.0;

pub const BEACON_Z_BIAS: f64 = 0.5;
pub const BEACON_SMOOTH_RATE: f64 = 6.0;
/// How far the beacon bobs either way, in units.
pub const BEACON_BOB: f64 = 0.1;
/// How fast the beacon bobs, in radians of its swing a second.
pub const BEACON_BOB_RATE: f64 = 2.0;
/// How fast the beacon spins, in radians a second.
pub const BEACON_SPIN_RATE: f64 = 1.2;
