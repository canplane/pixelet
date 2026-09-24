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

use pxl_spec::bake::constant::{BAND_CELLS, CELL_STEPS, MIN_LEVEL, POS_BITS, POS_MASK};
use pxl_spec::bake::fold::majority;

use super::adapter::{Entries, Painter};

/// One column's entries and where the column starts, in cells.
pub struct Column<'a> {
    pub cx0: i64,
    pub cy0: i64,
    pub entries: Entries<'a>,
}

/// Every non-empty cell of `level` the columns' voxel entries fold to, by its
/// coordinate at that level, into `out`. Instances and untaught states are
/// not voxels and do not fold; `hide` is left out, as the paint leaves it.
pub fn fold_columns(painter: &Painter, columns: &[Column], level: u32, hide: Option<(i64, i64, i32, u32)>, out: &mut HashMap<[i64; 3], u32>) {
    let top = level as i32;
    let mut native: HashMap<i32, Vec<([i64; 3], u32)>> = HashMap::new();
    for col in columns {
        let e = &col.entries;
        for k in 0..e.band.len() {
            let (band, pos, state) = (e.band[k], e.pos[k], e.state[k]);
            if !painter.is_voxel(state) || hide == Some((col.cx0, col.cy0, band, pos)) {
                continue;
            }
            // In finest steps, absolute: the column's corner, then the band's.
            let steps = CELL_STEPS as i64;
            let x = col.cx0 * steps + (pos & POS_MASK) as i64;
            let y = col.cy0 * steps + ((pos >> POS_BITS) & POS_MASK) as i64;
            let z = band as i64 * BAND_CELLS as i64 * steps + ((pos >> (2 * POS_BITS)) & POS_MASK) as i64;
            let own = (pos >> (3 * POS_BITS)) as i32 + MIN_LEVEL;
            let at = |v: i64, l: i32| v >> (l - MIN_LEVEL);
            if own <= top {
                native.entry(own).or_default().push(([at(x, own), at(y, own), at(z, own)], state));
            } else {
                // Coarser than the level: every level cell it covers.
                let n = 1i64 << (own - top);
                let (bx, by, bz) = (at(x, top), at(y, top), at(z, top));
                for dz in 0..n {
                    for dy in 0..n {
                        for dx in 0..n {
                            native.entry(top).or_default().push(([bx + dx, by + dy, bz + dz], state));
                        }
                    }
                }
            }
        }
    }

    let mut cur: HashMap<[i64; 3], u32> = HashMap::new();
    for lv in MIN_LEVEL..=top {
        // An entry at its own level stands over whatever finer ones folded
        // into its cell, as the paint lays it over them.
        for (c, state) in native.remove(&lv).unwrap_or_default() {
            cur.insert(c, state);
        }
        if lv == top {
            break;
        }
        let mut parents: HashMap<[i64; 3], [u32; 8]> = HashMap::with_capacity(cur.len());
        for (c, state) in cur.drain() {
            let p = c.map(|v| v.div_euclid(2));
            let child = (c[0].rem_euclid(2) + 2 * c[1].rem_euclid(2) + 4 * c[2].rem_euclid(2)) as usize;
            parents.entry(p).or_insert([0; 8])[child] = state;
        }
        cur.extend(parents.into_iter().filter_map(|(p, v)| {
            let m = majority(v, lv + 1);
            (m != 0).then_some((p, m))
        }));
    }
    out.clear();
    out.extend(cur);
}

#[cfg(test)]
#[path = "fold.test.rs"]
mod test;
