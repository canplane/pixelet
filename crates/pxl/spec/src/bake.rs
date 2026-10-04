// pxl-spec/bake.rs

//! What a baked page is, as both ends read it: the formats a terrain or POI
//! page is written in and decoded from, the ways a page carries above its
//! ground, the cell catalog a ground channel is
//! numbered by, which of a cell's four channels a viewer sees, how those four
//! are packed into one word, the surface a page's elevation draws and the
//! bound its index lane keeps on it, and the version the client asks for.
//! One implementation, so a page the bake wrote and a page the kernel reads
//! cannot drift.

use xpute_core::status::errno::Errno;
use xpute_core::status::error::MarshalError;

pub mod constant;
pub mod deck;
pub mod fold;
pub mod format;
pub mod ground;
#[path = "bake/object.spec.rs"]
pub mod object;
pub mod pack;
pub mod place_page;
#[path = "bake/poi_badge.spec.rs"]
pub mod poi_badge;
pub mod poi_page;
#[path = "bake/published.spec.rs"]
pub mod published;
pub mod quadkey;
pub mod surface;
pub mod way;

/// A packet no bake writes, as every page reader refuses it.
#[track_caller]
pub(crate) fn malformed() -> MarshalError {
    MarshalError::new(Errno::EBADMSG)
}
