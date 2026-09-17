// pxl_core/lib/env/route.rs

//! Every same-origin route the app requests.

use core::fmt;

pub use pxl_shared::bake::published::{BakeKind as TileKind, PUBLISHED_VERSION};

/// Everything the server answers. The app routes on paths of its own, so a
/// relay tells a deep link from a server request by this root alone.
const API_ROOT: &str = "/api";

/// Bucket-served bakes, a branch under the same root.
const TILE_ROOT: &str = "/api/tiles";

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

pub const SEARCH_URL: &str = "/api/search";

/// Mirrored as literals in worker/index.ts, which is bundled for the Workers
/// runtime rather than from src/.
pub const EDGE_TILE_PREFIX: &str = "/api/tiles/";
pub const EDGE_API_PREFIX: &str = "/api/";

const _: () = assert!(API_ROOT.len() == 4, "the API root");
