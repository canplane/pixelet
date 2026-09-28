// pxl-kernel/layer/terrain/extent.rs

//! How high the ground under a page can stand, as far as what is resident
//! says: the index lane (pxl-spec bake/format.rs `Index`) of the page itself
//! or of the nearest resident page over it. A lane's cell bounds the ground
//! drawn there at its own level and every level under it, the
//! interpolation's overshoot included, so the answer is never tighter than
//! the ground the page's own record, or any under it, will draw.

use std::collections::HashMap;

use pxl_spec::bake::format::Terrain;
use pxl_spec::space::pyramid::LevelPage;

use crate::global::Global;
use crate::world::access::Access;
use crate::world::pyramid::Layer as _;

/// Every page's answer, and the resident set it was read at: an observation
/// is estimated again every frame the eye moves and asks of each page it
/// names, while the answer moves only with what is resident, and a page's own
/// lane is a thousand cells.
type Read = (Option<u32>, HashMap<LevelPage, Option<(f64, f64)>>);
static READ: Global<Read> = Global::new();

/// The heights, in units, the ground under `p` can stand at: the lane's
/// cells over `p`'s square, read from the page it is drawn from. None where
/// nothing over it is resident.
pub fn extent_under(p: LevelPage) -> Option<(f64, f64)> {
    let revision = Terrain::pyramid().revision();
    let read = READ.get(Default::default);
    if read.0 != Some(revision) {
        *read = (Some(revision), HashMap::new());
    }
    *read.1.entry(p).or_insert_with(|| lane_under(p))
}

fn lane_under(p: LevelPage) -> Option<(f64, f64)> {
    Access::now().extent(p, p.node()).value()
}
