// pxl-kernel/layer/terrain/resolve.rs

//! What the pages already resident can answer, before anything is asked of
//! the network.
//!
//! A page the camera wants is one of four things, and only the last of them
//! is a request: it is resident; it can be folded from the pages under it,
//! which costs no network at all; it is not there but a coarser page over it
//! is, so the ground is drawn — coarsely — while the page is fetched; or
//! there is nothing, and the screen has a hole until the fetch lands.
//!
//! Which of the four it is decides what the fetch is *for*, and that is what
//! orders the queue (tile_fetcher.rs): a page with nothing over it is
//! coverage and a page with something over it is refinement. Latency then
//! shows up as ground getting sharper rather than as ground appearing.

use crate::layer::terrain::level::LevelPage;
use crate::layer::terrain::store::tile_store;

/// What the resident pages can do for a page the camera wants.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Resolved {
    /// The page itself is resident.
    Exact,
    /// Not resident, but every page it folds from is: the client can stand
    /// it up itself (tile_fetcher.rs's `fold_held`), and no request is made.
    Foldable,
    /// Not resident and not foldable, but a page over it is, so its ground
    /// is drawn coarse. The fetch refines what is already on the screen.
    Covered,
    /// Nothing draws this ground. The fetch is what puts it on the screen.
    Missing,
}

/// A page's nearest resident ancestor inside `window`. What draws its ground
/// while the page itself is not there.
///
/// The window is passed in rather than read here: what covers a page is what
/// the view draws, and a page resident above the window's top is one nothing
/// draws — the range keeps it against a zoom back, not as cover.
pub fn ancestor(p: LevelPage, window: &std::ops::Range<u32>) -> Option<LevelPage> {
    let top = window.end;
    let mut up = p;
    while up.level + 1 < top {
        up = up.parent();
        if tile_store().get_page(up).is_some() {
            return Some(up);
        }
    }
    None
}

/// Whether every page `p` folds from is resident.
pub fn children_resident(p: LevelPage) -> bool {
    p.level > 0 && p.children().iter().all(|&c| tile_store().get_page(c).is_some())
}

/// What the resident pages can do for `p`, the view drawing `window`.
pub fn resolve(p: LevelPage, window: &std::ops::Range<u32>) -> Resolved {
    if tile_store().get_page(p).is_some() {
        return Resolved::Exact;
    }
    if children_resident(p) {
        return Resolved::Foldable;
    }
    if ancestor(p, window).is_some() {
        return Resolved::Covered;
    }
    Resolved::Missing
}

/// Whether letting `p` go leaves its ground drawn: something coarser over it
/// is resident and drawn, or the pages it folds from are.
pub fn covered(p: LevelPage, window: &std::ops::Range<u32>) -> bool {
    ancestor(p, window).is_some() || children_resident(p)
}

#[cfg(test)]
#[path = "resolve.test.rs"]
mod test;
