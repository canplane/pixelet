// pxl-spec/route.spec.rs
//
// GENERATED from spec/spec/route.json
// The address space, as four reserved roots and the branches under them,
// and the two origins they are served from.
//
// The app routes on paths of its own, so a relay tells a deep link from a
// request meant for a server by a root alone — which is why the reservation is
// on the server's side, where it never reaches an address bar.
//
// Every relay reads a root or a branch and never a route name: adding a route
// is a change to the one server that answers it and to nothing else. This file
// is where all of them read it from: the client's driver, the Worker and the
// servers are written from it, and wrangler's and Caddy's configurations take
// their part of it. The kernel names what it opens and never an address.

/// The world's data: what is baked from Overture and the elevation raster,
/// and the one query over it that cannot be baked. pxl-geo answers what the
/// bucket does not.
pub const GEO_ROOT: &str = "/geo";

/// The application: sessions, chat, what a player places, and the realtime
/// half. pxl-api answers here, and answers nothing yet — the root, the
/// origin and the process exist so that the first route arrives into a
/// shape rather than assembling one on the day it is written.
pub const API_ROOT: &str = "/api";

/// Baked tiles by kind, version and name: under the root a kind's own, and
/// under that its object as a published set lays it out (bake/published.json
/// `OBJECT`, and `SPHERE` for the Earth's picture whole). Its own root for
/// the reason the asset root is: the bake
/// puts them in the bucket and the edge serves them, so neither server
/// answers them, and the edge tells them from the geo server's routes by
/// the root alone.
pub const TILE_ROOT: &str = "/tile";

/// Models and textures, by the sha256 of their bytes. Its own root because
/// it is neither server's: the ingest puts them in the bucket and the edge
/// serves them, and a content-addressed blob is as immutable as a URL gets.
/// Under the geo root it would be claiming to be geography, and under the
/// application's it would be claiming to be state.
pub const ASSET_ROOT: &str = "/asset";

/// Forward search, the one geo query with no finite tile to bake it against.
pub const SEARCH_URL: &str = "/geo/search";

/// Ops routes, behind the shared admin token. One per root rather than one
/// shared branch: each server gates its own prefix, and a token that opened
/// both would make either server's leak the other's.
pub const GEO_INTERNAL: &str = "/geo/internal";

/// pxl-api's own, gated the same way.
pub const API_INTERNAL: &str = "/api/internal";

pub const GEO_PREFIX: &str = "/geo/";

pub const API_PREFIX: &str = "/api/";

pub const TILE_PREFIX: &str = "/tile/";

pub const ASSET_PREFIX: &str = "/asset/";

/// Every root, as the edge claims them.
pub const WORKER_FIRST: [&str; 4] = ["/geo/*", "/api/*", "/tile/*", "/asset/*"];
