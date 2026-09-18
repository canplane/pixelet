// pxl-kernel/shell/config/route.rs

//! Every same-origin route the app requests. The roots and branches are
//! spec/route.json's, written into route_spec.rs; what is here is how a URL is
//! built out of them.

use core::fmt;

pub use pxl_spec::bake::published::{BakeKind as TileKind, PUBLISHED_VERSION};

pub use crate::shell::config::route_spec::{ASSET_PREFIX, GEO_PREFIX, SEARCH_URL, TILE_PREFIX, TILE_ROOT};

/// `<root>/<kind>[/<version>]`, joined with a page index by the caller —
/// written where the URL is written.
#[derive(Clone, Copy)]
pub struct TileBase {
    kind: TileKind,
    published: bool,
}

pub fn tile_base(kind: TileKind, published: bool) -> TileBase {
    TileBase { kind, published }
}

impl fmt::Display for TileBase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self.kind {
            TileKind::terrain => "terrain",
            TileKind::poi => "poi",
            TileKind::geocode => "geocode",
        };
        write!(f, "{TILE_ROOT}/{name}")?;
        if self.published {
            write!(f, "/{}", PUBLISHED_VERSION.of(self.kind))?;
        }
        Ok(())
    }
}

/// `<base>/<key>`.
#[derive(Clone, Copy)]
pub struct Joined<B, K>(pub B, pub K);

impl<B: fmt::Display, K: fmt::Display> fmt::Display for Joined<B, K> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.0, self.1)
    }
}

/// A URL kept inline: every route the app has is well inside it.
pub type Url = String;

/// A content-addressed asset by its bucket key (`asset/<sha256>`). The root and
/// the bucket's own prefix are the same word deliberately, so the URL is the key
/// with a leading slash.
pub struct AssetUrl<K>(pub K);

impl<K: fmt::Display> fmt::Display for AssetUrl<K> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "/{}", self.0)
    }
}
