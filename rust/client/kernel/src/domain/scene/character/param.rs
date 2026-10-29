// pxl-kernel/domain/scene/character/param.rs

//! The companion's tuned numbers: where the pet keeps, how the two catch up
//! and settle, and where a line hangs over them.

/// Arbitrary, so the pet does not share the person's footprint.
pub const PET_OFF: [f64; 2] = [-0.6, -0.6];

/// A fixed duration, not a fixed speed (domain/scene/character/path.rs
/// `WalkPlan`).
pub const CATCHUP_DURATION_S: f64 = 0.6;

pub const SETTLE_DELAY_S: f64 = 0.15;

/// A line hangs this far over the model's bounding box: a fixed clearance,
/// not a multiplier of its height.
pub const HEAD_CLEARANCE_U: f64 = 0.22;

/// Under one GPS fix's travel at a walking pace, a fix a second at 1.4 m/s,
/// about 0.74u at Seoul's latitude, and over the near-zero gap a still map
/// leaves. A rotate or pinch leaves such a gap, and a catch-up across it
/// would play the walk clip in place.
pub const WALK_EPS: f64 = 0.03;

pub const FOLLOW_WALK_LINGER_S: f64 = 1.5;

/// How long a clip crosses into the next, in seconds, once one plays.
pub const CLIP_FADE_S: f32 = 0.2;
