// pxl-spec/bake.rs

//! Baked tile formats shared by the bake and the kernel, so the two cannot drift.

use xpute_kit::status::errno::Errno;
use xpute_kit::status::error::MarshalError;

pub mod constant;
pub mod fold;
pub mod named;
#[path = "bake/object.spec.rs"]
pub mod object;
#[path = "bake/place_theme.spec.rs"]
pub mod place_theme;
#[path = "bake/published.spec.rs"]
pub mod published;
pub mod separation;
pub mod source;
pub mod tile;
pub mod way;

/// A packet no bake writes, as every tile reader refuses it.
#[track_caller]
pub(crate) fn malformed() -> MarshalError {
    MarshalError::new(Errno::EBADMSG)
}
