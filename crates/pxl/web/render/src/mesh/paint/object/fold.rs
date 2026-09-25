// pxl-render/mesh/paint/object/fold.rs

//! The object layer a level up: a coarse cell from the 2×2×2 under it, by
//! the rule the terrain folds by one dimension down (pxl-spec bake/fold.rs's
//! `majority`) — the state that fills at least half of eight, empty counted
//! as a state, so a lone cube goes at the first fold and a wall keeps whole
//! cells. A store's entries are cubes from an eighth of a cell up, each at
//! its own level, and they are folded from the finest up one level at a
//! time, an entry joining at its own level.
//!
//! Folded here rather than held in the store's trees: the pyramid a band tree
//! already is has no slot for an internal node's majority, and the fold is
//! the same function whether it is kept or made again, so it is made again
//! when a chunk that draws it is built (issue/20260921-level-step-and-zoom-are-one-number.md).

use std::collections::HashMap;

use pxl_spec::bake::constant::{unpack_pos, EntryAt, BAND, CELL, OBJECT_FINEST};
use pxl_spec::bake::fold::majority;
use pxl_spec::space::hierarchy::{parent_of, slot_of, Scale};
use pxl_spec::space::indexer::FROM_L2_IDX;
use pxl_spec::space::pvf3::ivec;

use super::adapter::{Entries, Painter};

/// Every non-empty cell of scale `target` the columns' voxel entries fold to,
/// by its index at that scale, into `out`. Instances and untaught states are
/// not voxels and do not fold; `hide` is left out, as the paint leaves it.
pub fn fold_columns(painter: &Painter, columns: &[Entries], target: Scale, hide: Option<EntryAt>, out: &mut HashMap<[i64; 3], u32>) {
    let mut native: HashMap<Scale, Vec<([i64; 3], u32)>> = HashMap::new();
    for e in columns {
        let corner = FROM_L2_IDX(e.col_idx);
        let (x0, y0) = (OBJECT_FINEST.index(ivec::X(corner)), OBJECT_FINEST.index(ivec::Y(corner)));
        for k in 0..e.band.len() {
            let (band, pos, state) = (e.band[k], e.pos[k], e.state[k]);
            if !painter.is_voxel(state) || hide == Some(e.at(k)) {
                continue;
            }
            // In finest steps, absolute: the column's corner, then the band's.
            let (x, y, z, own) = unpack_pos(pos);
            let x = x0 + x as i64;
            let y = y0 + y as i64;
            let z = band as i64 * OBJECT_FINEST.count_in(BAND) as i64 + z as i64;
            let at = |v: i64, s: Scale| v.div_euclid(OBJECT_FINEST.count_in(s) as i64);
            if own <= target {
                native.entry(own).or_default().push(([at(x, own), at(y, own), at(z, own)], state));
            } else {
                // Coarser than the target: every target cell it covers.
                let n = target.count_in(own) as i64;
                let (bx, by, bz) = (at(x, target), at(y, target), at(z, target));
                for dz in 0..n {
                    for dy in 0..n {
                        for dx in 0..n {
                            native.entry(target).or_default().push(([bx + dx, by + dy, bz + dz], state));
                        }
                    }
                }
            }
        }
    }

    let mut cur: HashMap<[i64; 3], u32> = HashMap::new();
    let mut scale = OBJECT_FINEST;
    loop {
        // An entry at its own scale stands over whatever finer ones folded
        // into its cell, as the paint lays it over them.
        for (c, state) in native.remove(&scale).unwrap_or_default() {
            cur.insert(c, state);
        }
        if scale >= target {
            break;
        }
        let mut parents: HashMap<[i64; 3], [u32; 8]> = HashMap::with_capacity(cur.len());
        for (c, state) in cur.drain() {
            parents.entry(parent_of(c)).or_insert([0; 8])[slot_of(c)] = state;
        }
        // The tie alternates by the parent's level as the ground's is
        // counted, from its cell, so an object and the ground under it fold
        // the same way at the same level.
        let parent = scale.up(1);
        cur.extend(parents.into_iter().filter_map(|(p, v)| {
            let m = majority(v, parent.above(CELL));
            (m != 0).then_some((p, m))
        }));
        scale = parent;
    }
    out.clear();
    out.extend(cur);
}

#[cfg(test)]
#[path = "fold.test.rs"]
mod test;
