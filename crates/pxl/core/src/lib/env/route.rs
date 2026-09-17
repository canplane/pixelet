// pxl_core/lib/env/route.rs

//! Every same-origin route the app requests.

use core::fmt::{self, Write};

use pxl_shared::asset::ingested::{IngestedAsset, INGESTED};
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

/// A content-addressed asset — a model, a texture — by its bucket key
/// (`asset/<sha256>`, the shipped palette's `model_key`/`skin_key`). No
/// version: the key is the content.
pub fn asset_url(key: &str) -> Url {
    let mut url = Url::new();
    let _ = write!(url, "{TILE_ROOT}/{key}");
    url
}

/// A shipped asset's row, by its path under asset/: the bucket key the asset
/// ingest gave its bytes, their size and a model's extent. A path the ingest
/// has not seen is a build that shipped a catalog ahead of its assets.
pub fn shipped_asset(path: &str) -> IngestedAsset {
    match INGESTED.get(path) {
        Some(row) => *row,
        None => panic!("{path} is not in pxl_shared::asset::ingested — run the asset ingest"),
    }
}

/// Where a shipped asset is fetched from, by its path under asset/.
pub fn shipped_asset_url(path: &str) -> Url {
    asset_url(shipped_asset(path).key)
}

pub const SEARCH_URL: &str = "/api/search";

/// Mirrored as literals in worker/index.ts, which is bundled for the Workers
/// runtime rather than from src/.
pub const EDGE_TILE_PREFIX: &str = "/api/tiles/";
pub const EDGE_API_PREFIX: &str = "/api/";

const _: () = assert!(API_ROOT.len() == 4, "the API root");
