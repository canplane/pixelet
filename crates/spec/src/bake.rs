// pxl-spec/bake.rs

//! Baked tile formats shared by the bake and the kernel, so the two cannot drift.

use xpute_core::status::errno::Errno;
use xpute_core::status::error::MarshalError;

pub mod constant;
pub mod deck;
pub mod division_tile;
pub mod earth_tile;
pub mod fold;
pub mod format;
pub mod named;
#[path = "bake/object.spec.rs"]
pub mod object;
#[path = "bake/place_badge.spec.rs"]
pub mod place_badge;
pub mod place_tile;
#[path = "bake/published.spec.rs"]
pub mod published;
pub mod surface;
pub mod way;

/// A packet no bake writes, as every tile reader refuses it.
#[track_caller]
pub(crate) fn malformed() -> MarshalError {
    MarshalError::new(Errno::EBADMSG)
}
