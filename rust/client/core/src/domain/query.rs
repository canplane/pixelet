// pxl-core/domain/query.rs

//! Queries over what is resident. Every query goes through the world's one
//! query of what is in a node (`World::node`). On it stand the asset in a
//! cell at the zoom it is asked (`World::asset`) and the first cell a ray
//! meets (`first_hit`). An eye casts such rays, and a walker casts them
//! straight up and down its cell (`support.rs`). A query never fetches or
//! holds anything. It returns an exact answer, a coarser answer with the
//! tile it lacked, or nothing with that tile.
//!
//! A cell is what the cast draws, by one rule, so picks and column surfaces
//! stand on what is on the screen. A node coarser than the zoom asked is not
//! what is drawn there; it is asked only whether anything under it fills, at
//! any zoom.
//!
//! The query is the only reader of the terrain and the objects
//! (ARCHITECTURE.md, "The query, the only seam"). It is a child of `layer` in
//! the module tree, its files beside the layers' (domain/layer.rs), and
//! callers name it `crate::domain::query`.
//! The layers' reads (the terrain's and the store's own methods, `CpuClipmap`,
//! the ways' pool) are visible within `layer` alone, so the compiler refuses
//! them anywhere else, and none is re-exported. A public `World` function
//! answers from the composed world; an answer from one layer alone
//! (`surface_of`) stays inside the query.

// layer.rs names this file by `#[path]`, so it resolves its children as a
// mod-rs file does: each is named from here.
#[path = "query/change.rs"]
pub mod change;
#[path = "query/decor.rs"]
pub mod decor;
#[path = "query/decor_box.rs"]
pub mod decor_box;
#[path = "query/entry.rs"]
mod entry;
#[path = "query/limit.rs"]
mod limit;
#[path = "query/node.rs"]
mod node;
#[path = "query/place.rs"]
mod place;
#[path = "query/ray.rs"]
mod ray;
#[path = "query/scatter.rs"]
mod scatter;
#[path = "query/support.rs"]
mod support;
#[path = "query/surface.rs"]
mod surface;
#[path = "query/world.rs"]
mod world;

pub use ray::{at_zoom, by_dist, cell_zoom_at, finest, zoom_range, Hit, HitKind, Ray};
pub use support::{floor_of, is_unknown, on_floor, over, unknown};
pub use world::{ColumnWalk, World};

#[cfg(test)]
#[path = "query.test.rs"]
pub(crate) mod test;
