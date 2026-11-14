// pxl-spec/bake.rs

//! Baked tile formats shared by the bake and the core.

use xpute_kit::status::errno::Errno;
use xpute_kit::status::error::XputeError;

pub mod constant;
pub mod fold;
#[path = "bake/object.spec.rs"]
pub mod object;
#[path = "bake/place_theme.spec.rs"]
pub mod place_theme;
#[path = "bake/publication.spec.rs"]
pub mod publication;
pub mod separation;
pub mod source;
pub mod tile;
pub mod way;

/// A packet no bake writes, as every tile reader refuses it.
#[track_caller]
pub(crate) fn malformed() -> XputeError {
    XputeError::new(Errno::EBADMSG)
}
