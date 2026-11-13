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
//! The layers' reads (the terrain's and the store's own methods, `clip_read`,
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

use pxl_content::generation::CLAIM_EXTENT;
use pxl_engine::answer::In;
use pxl_spec::space::coord::zoom::Zoom;
use pxl_spec::space::hierarchy::tree::Node;
use pxl_spec::space::layer::lattice::GROUND;

use crate::domain::layer::instance::Instance;
use crate::domain::layer::tile_cache::Path;
use crate::domain::layer::Answer;
use node::At;

impl World {
    /// What is in the cell `node` at its own zoom, which is `coarsest` or
    /// finer, for an asker with nothing handed down (`asset_at`), the decor
    /// standing in it included. X is on the asker's lap.
    pub fn asset(&self, node: Node<3>) -> Answer<u32> {
        report_claims(node);
        self.asset_at(node, |at, zooms, path| {
            self.node(at, zooms, path, &mut |_| true)
        })
    }

    /// What is in `at`'s node, asked over `zooms`, the zooms from far to
    /// near that a reader asks within it: this is the world's one query. What
    /// the layers compose (`layer_node`), with the decor standing in it
    /// (`with_decor`), which only the CPU answers. So a tree the ground grew
    /// and a tree someone planted are one thing to whatever asks, of the
    /// decor it `sees`.
    fn node(
        &self,
        at: &mut At,
        zooms: [Zoom; 2],
        path: &mut Path,
        sees: &mut dyn FnMut(&Instance) -> bool,
    ) -> Answer<In<[u32; 2]>> {
        let (answer, relief) = self.layers(at, zooms, path);
        answer.map(|i| self.with_decor(at, zooms[1], relief, i, sees))
    }
}

/// Reports as read the columns a decor claiming the node's leaf cell may
/// stand on, for an asker that keeps what the cell answers.
fn report_claims((zoom, c): Node<3>) {
    if let Some(s) = zoom.checked_sub(GROUND.leaf_cell_zoom()) {
        let [x, y] = [c[0] >> s, c[1] >> s].map(f64::from);
        change::report_read(
            [
                x - f64::from(CLAIM_EXTENT[0]),
                y - f64::from(CLAIM_EXTENT[1]),
            ],
            [x, y],
            GROUND.leaf_cell_zoom(),
        );
    }
}

#[cfg(test)]
#[path = "query.test.rs"]
pub(crate) mod test;
