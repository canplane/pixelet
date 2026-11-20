// pxl-core/domain/control/camera/motion.rs

//! The camera's move. The camera (camera.rs `Camera`) is one, and a hand's
//! pan writes it as it moves: a drag and a held key put the pivot where they
//! ask at once. Whatever takes time is a move, one at a time: where the
//! camera ends. The distance and the facing are eased there. Where the pivot
//! is to go, the orbit flies, the whole camera carried (orbit.rs `step`),
//! level to a place beside it and higher with how far it goes, and first
//! person is eased. A warp starts no move: the pivot is there at once, and a
//! move under way ends there (controller.rs `warp`). With no move the camera
//! rests where it is drawn.

use pxl_global::Global;
use pxl_spec::space::coord::v3::V3;
use xpute_kit::math::scalar::clamp;

use crate::domain::control::camera::{Camera, facing_domain};
use crate::domain::control::param::{MAX_DIST, MIN_DIST};

/// A move of the camera: the camera it ends at, its pivot in the world's
/// reals as the camera's is, and how far along the orbit's flight is.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Move {
    pub to: V3,
    pub dist: f64,
    /// Radians from straight down, in [0, PI].
    pub polar: f64,
    /// Radians about Z from north toward east, in [-PI, PI].
    pub azimuth: f64,
    /// How fast the orbit goes along its flight's way (domain/smooth.rs
    /// `fly`).
    pub speed: f64,
    /// How high the flight carries the orbit over where it stands, in units:
    /// the eye and the pivot together, with nothing between them changed.
    pub height: f64,
}

impl Move {
    /// The move with its distance and facing within the camera's ranges.
    pub(crate) fn in_range(self) -> Move {
        let (polar, azimuth) = facing_domain(self.polar, self.azimuth);
        Move {
            dist: clamp(self.dist, MIN_DIST, MAX_DIST),
            polar,
            azimuth,
            ..self
        }
    }
}

static MOVE: Global<Option<Move>> = Global::new();

/// The camera's move, none while it rests.
pub(crate) fn motion() -> &'static mut Option<Move> {
    MOVE.get(|| None)
}

/// Where `cam` ends now: the end of the move it is on, or where it stands
/// with none.
pub(crate) fn end(cam: &Camera) -> Move {
    motion().unwrap_or(Move {
        to: cam.pivot,
        dist: cam.dist,
        polar: cam.polar,
        azimuth: cam.azimuth,
        speed: 0.0,
        height: 0.0,
    })
}
