// pxl-spec/bake.rs

//! Baked tile formats shared by the bake and the kernel, so the two cannot drift.

use xpute_core::status::errno::Errno;
use xpute_core::status::error::MarshalError;

pub mod constant;
pub mod deck;
pub mod earth_tile;
pub mod fold;
pub mod format;
pub mod ground;
pub mod named;
#[path = "bake/object.spec.rs"]
pub mod object;
pub mod place_tile;
#[path = "bake/poi_badge.spec.rs"]
pub mod poi_badge;
pub mod poi_tile;
#[path = "bake/published.spec.rs"]
pub mod published;
pub mod quadkey;
pub mod surface;
pub mod way;

/// A packet no bake writes, as every tile reader refuses it.
#[track_caller]
pub(crate) fn malformed() -> MarshalError {
    MarshalError::new(Errno::EBADMSG)
}
