//! What a cast forgets of the observed pyramid when a clip level's rows move (pxl-render
//! gpu/ray/cast.rs, `clipmap::Entry::moved_from`, `pyramid::forgets`): over the real world of
//! tests/real_world, every node a ray down over a brick visits is forgotten where that brick
//! turns present or its slot changes, and none where only a fill's ticket changed, whose
//! answers stand; and where a clip level slides, its entries let go never filled, every node
//! still kept answers as the world does after it.

mod keeping;
mod real_world;

use std::collections::BTreeMap;

use keeping::*;
use pxl_engine::cast::{self, Detail, Ray, Spacing, Until};
use pxl_engine::clipmap::{self, Entry, ACROSS, ENTRIES, FIRST_CELL, ORIGIN_AT};
use pxl_engine::overwrite::Cut;
use pxl_engine::pyramid::{self, MOVED_WORDS};
use pxl_engine::{wrap_idx, Zoom, LEAF_CELL};
use real_world::{Built, World, FINEST, UNLAID};

/// East of X = 0, by the seam, as tests/light_miss.rs's eye.
const EYE: [i32; 2] = [(1 << 23) - 300, (40 << 16) + 30];

/// The moved words a frame lays from the rows it held, `was`, to those it holds, `now`: the
/// host's rule (gpu/ray/cast.rs).
fn moved_of(was: &[[i32; 4]], now: &[[i32; 4]]) -> [u32; MOVED_WORDS as usize] {
    let mut moved = [0u32; MOVED_WORDS as usize];
    let rows = (clipmap::EYE_CLIP_LEVELS * ENTRIES) as usize;
    for k in 0..rows {
        if Entry::of(now[k]).moved_from(Entry::of(was[k])) {
            moved[k / 32] |= 1 << (k % 32);
        }
    }
    moved
}

/// Every node rays down over brick `q` of the clip level at `cell` visit, at the zooms they ask,
/// each from over the ground `b` holds where it starts.
fn over_brick(b: &Built, w: World, cell: Zoom, q: [i32; 2]) -> Met {
    let mut table = vec![0u32; pyramid::WORDS as usize];
    let mut met = BTreeMap::new();
    let mut k = Keeping {
        w,
        cuts: [Cut::NONE; 2],
        table: &mut table,
        tag: pyramid::tag(3, pyramid::MISSED_PASS),
        asks: Asks::default(),
        met: Some(&mut met),
        writes: true,
    };
    let detail = Detail {
        range: [8, FINEST],
        spacing: Spacing {
            angle: 0.002,
            across: 0.0,
        },
    };
    let s = clipmap::BRICK_SHIFT + LEAF_CELL - cell;
    let (g, side) = ([q[0] << s, q[1] << s], 1 << s);
    for (dx, dy) in [(1, 1), (side / 2, side / 3), (side - 2, side - 3)] {
        let at = [real_world::wrap(g[0] + dx), g[1] + dy];
        let o = q8([at[0], at[1], b.top_at(at) + 30]);
        let r = Ray::new([0.01, 0.02, -1.0], 400.0);
        cast::first(&mut k, o, &r, detail, Until::Opaque);
    }
    met
}

#[test]
fn a_moved_brick_forgets_what_reads_it_and_a_ticket_none() {
    let built = real_world::build(EYE);
    let rows = (clipmap::EYE_CLIP_LEVELS * ENTRIES) as usize;
    // The leaf clip level's row of the brick the eye stands on, present.
    let (q, _) = clipmap::brick_of(EYE);
    let k = ((LEAF_CELL - FIRST_CELL) * ENTRIES + clipmap::entry(q)) as usize;
    let row = built.clipmap[k];
    assert!(Entry::of(row).holds(q), "the eye's brick held: {row:?}");
    let met = over_brick(&built, built.world(), LEAF_CELL, q);
    let nodes: Vec<(Zoom, [i32; 3], [Zoom; 2])> = met.keys().copied().collect();
    assert!(
        nodes.len() > 40,
        "too few nodes over the brick: {}",
        nodes.len()
    );
    let now = &built.clipmap[..rows];

    // Turning present: every node over it forgotten.
    let mut was = now.to_vec();
    was[k][2] = 0;
    let moved = moved_of(&was, now);
    let kept: Vec<_> = nodes
        .iter()
        .filter(|n| !pyramid::forgets(&moved, n.0, [n.1[0], n.1[1]]))
        .collect();
    assert!(kept.is_empty(), "turning present kept {kept:?}");

    // Its slot changed: every node over it forgotten.
    let mut was = now.to_vec();
    was[k][2] = row[2] + 1;
    let moved = moved_of(&was, now);
    let kept: Vec<_> = nodes
        .iter()
        .filter(|n| !pyramid::forgets(&moved, n.0, [n.1[0], n.1[1]]))
        .collect();
    assert!(kept.is_empty(), "a slot moved kept {kept:?}");

    // A fill's ticket alone: none forgotten, and every answer over it as it was.
    let mut was = now.to_vec();
    was[k][3] = row[3] ^ 0x5a5a;
    let moved = moved_of(&was, now);
    assert!(moved.iter().all(|&m| m == 0), "a ticket moved a row");
    let mut ticketed = built.clipmap.clone();
    ticketed[k][3] = row[3] ^ 0x5a5a;
    let other = World {
        clipmap: &ticketed,
        ..built.world()
    };
    let cuts = [Cut::NONE; 2];
    let differ: Vec<_> = nodes
        .iter()
        .filter(|n| {
            let (a, h) = truth(built.world(), cuts, n.0, n.1, n.2);
            let (b, i) = truth(other, cuts, n.0, n.1, n.2);
            (a.kind as u32, a.asset, h) != (b.kind as u32, b.asset, i)
        })
        .collect();
    assert!(differ.is_empty(), "a ticket changed answers: {differ:?}");
}

/// The rows of `b` before and after the clip level at `cell` slid `by` bricks, the entries it
/// lets go wanted and not yet filled: before, those absent and the rest as `b` holds them;
/// after, each wanting the brick it takes, absent, as the kernel's table lays it, and its first
/// brick moved; every clip level's entries folded again each time.
fn slide(b: &Built, cell: Zoom, by: [i32; 2]) -> [Vec<[i32; 4]>; 2] {
    let (mut was, mut now) = (b.clipmap.clone(), b.clipmap.clone());
    let z = clipmap::bricks_zoom(cell);
    let o = (ORIGIN_AT + cell - FIRST_CELL) as usize;
    let first = [wrap_idx(z, now[o][0] + by[0]), now[o][1] + by[1]];
    now[o][..2].copy_from_slice(&first);
    for k in 0..ENTRIES {
        let q = [
            wrap_idx(z, first[0] + (k % ACROSS) as i32),
            first[1] + (k / ACROSS) as i32,
        ];
        let r = ((cell - FIRST_CELL) * ENTRIES + clipmap::entry(q)) as usize;
        if now[r][..2] != q {
            was[r][2..].fill(0);
            now[r] = [q[0], q[1], 0, 0];
        }
    }
    for rows in [&mut was, &mut now] {
        real_world::fold(rows, &b.ground, &b.palette, &b.shards, &b.pool);
    }
    [was, now]
}

#[test]
fn a_slide_forgets_every_answer_it_moves() {
    let built = real_world::build(EYE);
    let rows = (clipmap::EYE_CLIP_LEVELS * ENTRIES) as usize;
    let cuts = [Cut::NONE; 2];
    for cell in (FIRST_CELL..=LEAF_CELL).filter(|c| !UNLAID.contains(c)) {
        for by in [[1, 0], [0, 1], [-1, -1]] {
            let [was, now] = slide(&built, cell, by);
            let (before, after) = (
                World {
                    clipmap: &was,
                    ..built.world()
                },
                World {
                    clipmap: &now,
                    ..built.world()
                },
            );
            let moved = moved_of(&was[..rows], &now[..rows]);
            // Every node over the clip level, before the slide and after.
            let first = was[(ORIGIN_AT + cell - FIRST_CELL) as usize];
            let mut nodes = BTreeMap::new();
            for j in -1..=ACROSS as i32 {
                for i in -1..=ACROSS as i32 {
                    let q = [first[0] + i, first[1] + j];
                    nodes.extend(over_brick(&built, before, cell, q));
                    nodes.extend(over_brick(&built, after, cell, q));
                }
            }
            let stale: Vec<_> = nodes
                .keys()
                .filter(|n| {
                    pyramid::keeps(n.0, n.1) && !pyramid::forgets(&moved, n.0, [n.1[0], n.1[1]])
                })
                .filter(|n| {
                    let (a, h) = truth(before, cuts, n.0, n.1, n.2);
                    let (b, i) = truth(after, cuts, n.0, n.1, n.2);
                    (a.kind as u32, a.asset, h) != (b.kind as u32, b.asset, i)
                })
                .collect();
            assert!(
                stale.is_empty(),
                "the clip level at {cell} slid {by:?} kept {} of {} nodes stale: {:?}",
                stale.len(),
                nodes.len(),
                &stale[..stale.len().min(8)]
            );
        }
    }
}
