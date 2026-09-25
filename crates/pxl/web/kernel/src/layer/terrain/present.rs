// pxl-kernel/layer/terrain/present.rs

//! What the terrain draws of its cover (world/pyramid.rs `Layer::present`).
//! The cover is every layer's; the limit here is the terrain's mesh, and so
//! it is applied to the terrain's cover alone: a count has no seams.
//!
//! What the mesh cannot draw as it stands — a step too steep for a skirt — is
//! refined or coarsened within what the resident pages can draw, and drawn
//! as it is where they cannot. The cover's gaps are the ground nothing over
//! is held for; until it comes, the stale chunks that drew it last are drawn
//! (chunk/table.rs `retire`).

use std::collections::{HashMap, HashSet};

use crate::world::pyramid::chunk::Chunk;
use crate::world::pyramid::{Cover, Domain};
use xpute_core::status::bug::OrBug;
use xpute_core::status::errno::Errno;

/// The terrain's cover as its mesh can draw it: no two chunks the view sees
/// that meet more than a level apart. A chunk's mesh hangs a skirt one of its
/// own cells deep, which hides the step where it meets a chunk a level from
/// it (pxl-render mesh/chunk_mesher.rs) and not one two levels from it. A
/// step only shows where the edge two chunks share is on screen, and an edge
/// on screen is inside both chunks' bounds, so both are seen: a pair the view
/// does not see both of has no step to hide. Held to it anyway, ground the
/// frustum leaves at the pyramid's top — which is never refined — coarsened
/// the ground under the eye, a level a step, from the screen's edge inward.
///
/// Every chunk too coarse for a neighbor is refined where the view wants it
/// finer and something the cover can draw it from is held — its four
/// children in its place, never finer than the view wants, so no more
/// chunks than the view has. Then what is still too fine for a coarse
/// neighbor is coarsened: its sibling group and everything under it
/// becomes the nearest chunk over it the cover can draw. Where none is it
/// stays as it is, a seam until a page over it comes, where opening it would
/// show nothing there at all. The first pass only makes chunks finer and
/// stops at what the view wants; the second only makes them coarser, and
/// ends on a round that has none to coarsen. Neither undoes the other, so
/// both end.
pub fn mesh(cover: Cover, d: &Domain, drawable: impl Fn(Chunk) -> bool) -> Cover {
    let mut drawn: HashSet<Chunk> = cover.drawn.into_iter().collect();
    let mut gaps: HashSet<Chunk> = cover.gaps.into_iter().collect();

    loop {
        let refine: HashSet<Chunk> = unbalanced(&drawn, d)
            .into_iter()
            .map(|(_, coarse)| coarse)
            .filter(|&c| d.splits(c) && d.children(c).first().is_some_and(|&k| drawable(k)))
            .collect();
        if refine.is_empty() {
            break;
        }
        for c in refine {
            drawn.remove(&c);
            drawn.extend(d.children(c));
        }
    }
    loop {
        let ups: HashSet<Chunk> = unbalanced(&drawn, d).into_iter().map(|(fine, _)| fine.parent().or_bug(Errno::ENOTRECOVERABLE)).collect();
        if ups.is_empty() {
            break;
        }
        // One sibling group at a time from the finest: a parent coarsened
        // in the same round as one over it would be drawn under it.
        let ups: HashSet<Chunk> = ups.iter().copied().filter(|u| !u.ancestors().any(|a| ups.contains(&a))).collect();
        let up = standing(ups, &drawable);
        if up.is_empty() {
            break;
        }
        replace(&mut drawn, &mut gaps, &up);
    }
    Cover {
        drawn: drawn.into_iter().collect(),
        gaps: gaps.into_iter().collect(),
    }
}

/// What a coarsened chunk becomes: each one's nearest self or ancestor the
/// cover can draw, and none for one that has none, which stays as it is. An
/// ancestor drawn draws everything under it, so none is kept inside another.
fn standing(chunks: HashSet<Chunk>, drawable: &impl Fn(Chunk) -> bool) -> HashSet<Chunk> {
    let mut up: HashSet<Chunk> = chunks.into_iter().filter_map(|c| std::iter::once(c).chain(c.ancestors()).find(|&a| drawable(a))).collect();
    let nested: Vec<Chunk> = up.iter().copied().filter(|a| a.ancestors().any(|b| up.contains(&b))).collect();
    for a in nested {
        up.remove(&a);
    }
    up
}

/// Everything drawn or open under `up` gone and `up` drawn: one pass however
/// many are replaced.
fn replace(drawn: &mut HashSet<Chunk>, gaps: &mut HashSet<Chunk>, up: &HashSet<Chunk>) {
    let under = |k: &Chunk| std::iter::once(*k).chain(k.ancestors()).any(|a| up.contains(&a));
    drawn.retain(|k| !under(k));
    gaps.retain(|k| !under(k));
    drawn.extend(up.iter().copied());
}

/// Every pair of drawn chunks the view sees that meet more than a level
/// apart, the finer first: across each of its sides, the chunks of the
/// column beside it at its level or any over it whose heights reach its own.
/// The drawn chunks are grouped by column once, so a side is a walk up that
/// column's levels and not one up every chunk above and below it.
fn unbalanced(drawn: &HashSet<Chunk>, d: &Domain) -> Vec<(Chunk, Chunk)> {
    let mut columns: HashMap<(u32, [i64; 2]), Vec<Chunk>> = HashMap::new();
    for &c in drawn {
        columns.entry((c.level(), c.column().1)).or_default().push(c);
    }
    let mut out = Vec::new();
    for &fine in drawn {
        let (f0, f1) = fine.z_range();
        let reach = f1 - f0;
        for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            let next = fine.beside(dx, dy, 0);
            for a in std::iter::once(next).chain(next.ancestors()).filter(|a| a.level() >= fine.level() + 2) {
                let Some(cubes) = columns.get(&(a.level(), a.column().1)) else { continue };
                for &coarse in cubes {
                    let (c0, c1) = coarse.z_range();
                    if c0 < f1 + reach && c1 > f0 - reach && d.sees(fine) && d.sees(coarse) {
                        out.push((fine, coarse));
                    }
                }
            }
        }
    }
    out
}
