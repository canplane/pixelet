// pxl-kernel/layer/terrain/present.rs

//! What the terrain draws of its cover (world/pyramid.rs `Layer::present`).
//! The cover is every layer's; the two limits here are the terrain's mesh
//! and chunk table's, and so they are applied to the terrain's cover alone:
//! a count has no seams and no table to fill.
//!
//! What cannot be drawn becomes a gap rather than being drawn wrong, and a
//! gap is what the wishlist fetches, so the ground left open is the ground
//! asked for first. Until it comes, the stale chunks that drew it last are
//! drawn (chunk/table.rs `retire`).

use std::collections::HashSet;

use crate::world::pyramid::chunk::Chunk;
use crate::world::pyramid::{Cover, Domain};

/// The terrain's cover as its mesh can draw it.
///
/// First, a finer page stands in for the ground the view wants a level
/// finer and no more: a stand-in draws its ground with four times the chunks
/// a level, and the chunk table holds the view's chunks four times over
/// (domain.rs `VIEW_CHUNKS`). A stand-in deeper than that gives way to the
/// chunk a level under the wanted one it lies in, drawn by its nearest
/// resident self or ancestor (`standing`), and a gap only where nothing over
/// it is held. Every ancestor of the ground drawn is wanted held, so a
/// stand-in from under is what a view falls back to only before those come.
///
/// Then the balance: no two chunks the view sees that meet more than a level
/// apart. A chunk's mesh hangs a skirt one of its own cells deep, which hides
/// the step where it meets a chunk a level from it (pxl-render
/// mesh/chunk_mesher.rs) and not one two levels from it. A step only shows
/// where the edge two chunks share is on screen, and an edge on screen is
/// inside both chunks' bounds, so both are seen: a pair the view does not
/// see both of has no step to hide. Held to it anyway, ground the frustum
/// leaves at the pyramid's top — which is never refined — coarsened the
/// ground under the eye, a level a step, from the screen's edge inwards. Every chunk too
/// coarse for a neighbour is refined where the view wants it finer and the
/// page under it is resident — its four children in its place, never finer
/// than the view wants, so no more chunks than the view has. Then what is
/// still too fine for a coarse neighbour is coarsened: its sibling group and
/// everything under it becomes its parent, or the nearest resident ancestor
/// over it where the parent's page is not. The first pass only makes chunks
/// finer and stops at what the view wants; the second only makes them
/// coarser or removes them and stops at the pyramid's top. Neither undoes
/// the other, so both end.
pub fn mesh(cover: Cover, d: &Domain, resident: impl Fn(Chunk) -> bool) -> Cover {
    let mut drawn: HashSet<Chunk> = HashSet::with_capacity(cover.drawn.len());
    let mut open: HashSet<Chunk> = HashSet::new();
    for c in cover.drawn {
        let want = d.wanted_over(c);
        if c.level() + 1 >= want.level() {
            drawn.insert(c);
        } else {
            // Past the one level a stand-in may reach: that branch opens, a
            // level under the wanted one, and the stand-ins beside it stay.
            open.insert(c.ancestors().find(|a| a.level() + 1 == want.level()).expect("a chunk under the wanted one has one a level under it"));
        }
    }
    let mut gaps: HashSet<Chunk> = cover.gaps.into_iter().collect();
    let (up, open) = standing(open, &resident);
    replace(&mut drawn, &mut gaps, &up, open);

    loop {
        let refine: HashSet<Chunk> = unbalanced(&drawn, d)
            .into_iter()
            .map(|(_, coarse)| coarse)
            .filter(|&c| d.splits(c) && resident(c.children()[0]))
            .collect();
        if refine.is_empty() {
            break;
        }
        for c in refine {
            drawn.remove(&c);
            drawn.extend(c.children());
        }
    }
    loop {
        let ups: HashSet<Chunk> = unbalanced(&drawn, d)
            .into_iter()
            .map(|(fine, _)| fine.parent().expect("a chunk two levels under another has a parent"))
            .collect();
        if ups.is_empty() {
            break;
        }
        // One sibling group at a time from the finest: a parent coarsened
        // in the same round as one over it would be drawn under it.
        let ups: HashSet<Chunk> = ups.iter().copied().filter(|u| !u.ancestors().any(|a| ups.contains(&a))).collect();
        let (up, open) = standing(ups, &resident);
        replace(&mut drawn, &mut gaps, &up, open);
    }
    Cover {
        drawn: drawn.into_iter().collect(),
        gaps: gaps.into_iter().collect(),
    }
}

/// What stands in for ground the mesh cannot draw as it is: each chunk's
/// nearest resident self or ancestor, up to the pyramid's top (domain.rs
/// `pages`), so the ground is drawn coarser rather than opened; a gap only where nothing over it is held. An ancestor
/// drawn draws everything under it, so none is kept inside another.
fn standing(chunks: HashSet<Chunk>, resident: &impl Fn(Chunk) -> bool) -> (HashSet<Chunk>, HashSet<Chunk>) {
    let mut up = HashSet::new();
    let mut open = HashSet::new();
    for c in chunks {
        match std::iter::once(c).chain(c.ancestors()).find(|&a| resident(a)) {
            Some(a) => up.insert(a),
            None => open.insert(c),
        };
    }
    let nested: Vec<Chunk> = up.iter().copied().filter(|a| a.ancestors().any(|b| up.contains(&b))).collect();
    for a in nested {
        up.remove(&a);
    }
    open.retain(|c| !c.ancestors().any(|b| up.contains(&b)));
    (up, open)
}

/// Everything drawn or open under `up` or `open` gone, `up` drawn and
/// `open` left as gaps: one pass however many are replaced.
fn replace(drawn: &mut HashSet<Chunk>, gaps: &mut HashSet<Chunk>, up: &HashSet<Chunk>, open: HashSet<Chunk>) {
    if up.is_empty() && open.is_empty() {
        return;
    }
    let under = |k: &Chunk| std::iter::once(*k).chain(k.ancestors()).any(|a| up.contains(&a) || open.contains(&a));
    drawn.retain(|k| !under(k));
    gaps.retain(|k| !under(k));
    drawn.extend(up.iter().copied());
    gaps.extend(open);
}

/// Every pair of drawn chunks the view sees that meet more than a level
/// apart, the finer first.
fn unbalanced(drawn: &HashSet<Chunk>, d: &Domain) -> Vec<(Chunk, Chunk)> {
    let mut out = Vec::new();
    for &fine in drawn {
        for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            let next = fine.beside(dx, dy);
            if let Some(coarse) = std::iter::once(next).chain(next.ancestors()).find(|a| drawn.contains(a)) {
                if coarse.level() >= fine.level() + 2 && d.sees(fine) && d.sees(coarse) {
                    out.push((fine, coarse));
                }
            }
        }
    }
    out
}
