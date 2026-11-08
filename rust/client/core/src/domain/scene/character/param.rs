// pxl-core/domain/scene/character/param.rs

//! The tuned numbers of the person and the pet: where the pet keeps beside
//! the person, how the two catch up and settle, and where a line hangs over
//! them.

/// The pet's place beside the person, in units. The value is arbitrary; it
/// only keeps the pet off the person's footprint.
pub const PET_OFF: [f64; 2] = [-0.6, -0.6];

/// A body's half width with its arms out, as a share of its height. A body is
/// drawn at least this far from a wall.
pub const BODY_SHARE: f64 = 0.4;

/// How long a catch-up walk takes, in seconds. A catch-up takes a fixed time,
/// not a fixed speed (domain/scene/character/companion.rs `WalkPlan`).
pub const CATCHUP_DURATION_S: f64 = 0.6;

pub const SETTLE_DELAY_S: f64 = 0.15;

/// How far, in units, a line hangs over the model's bounding box. The
/// clearance is the same for every model.
pub const HEAD_CLEARANCE_U: f64 = 0.22;

/// The gap, in units, within which a character counts as at its target and
/// does not walk. It is under the distance one GPS fix covers at a walking
/// pace (a fix a second at 1.4 m/s, about 0.74 u at Seoul's latitude), and
/// over the near-zero gap a still map leaves. A rotate or a pinch leaves such
/// a gap, and a catch-up across it would play the walk clip in place.
pub const WALK_EPS: f64 = 0.03;

pub const FOLLOW_WALK_LINGER_S: f64 = 1.5;

/// How long, in seconds, a clip cross-fades into the next. A character's
/// first clip starts without a fade.
pub const CLIP_FADE_S: f32 = 0.2;

/// How fast a character surface turns its character, in radians a second, so
/// that every side of it shows.
pub const SURFACE_SPIN_RAD_S: f64 = 0.6;
