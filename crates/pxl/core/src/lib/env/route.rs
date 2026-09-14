// pxl_core/lib/env/route.rs

//! Every same-origin route the app requests.

pub use pxl_shared::bake::published::{BakeKind as TileKind, PUBLISHED_VERSION};

/// Everything the server answers. The app routes on paths of its own, so a
/// relay tells a deep link from a server request by this root alone.
const API_ROOT: &str = "/api";

/// Bucket-served bakes, a branch under the same root.
const TILE_ROOT: &str = "/api/tiles";

/// `<root>/<kind>[/<version>]`, joined with a page index by the caller.
pub fn tile_base(kind: TileKind, published: bool) -> String {
    let name = match kind {
        TileKind::terrain => "terrain",
        TileKind::poi => "poi",
        TileKind::geocode => "geocode",
    };
    if published {
        format!("{TILE_ROOT}/{name}/{}", PUBLISHED_VERSION.of(kind))
    } else {
        format!("{TILE_ROOT}/{name}")
    }
}

/// A content-addressed asset — a model, a texture — by its bucket key
/// (`asset/<sha256>`, the shipped palette's `model_key`/`skin_key`). No
/// version: the key is the content.
pub fn asset_url(key: &str) -> String {
    format!("{TILE_ROOT}/{key}")
}

pub const SEARCH_URL: &str = "/api/search";

/// Mirrored as literals in worker/index.ts, which is bundled for the Workers
/// runtime rather than from src/.
pub const EDGE_TILE_PREFIX: &str = "/api/tiles/";
pub const EDGE_API_PREFIX: &str = "/api/";

const _: () = assert!(API_ROOT.len() == 4, "the API root");
