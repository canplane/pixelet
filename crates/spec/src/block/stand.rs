// pxl-spec/block/stand.rs

//! What is in each block of a chunk's columns, heights in the sample's
//! cells: the ground, its bridges, tunnels and piers, and what is placed. The
//! one composition the chunks draw and the world's mechanics answer from.

use std::collections::HashMap;

use crate::bake::tile::terrain::GroundId;
use crate::block::object::{cube_of, object_value_is_air};
use crate::block::sample::{grow, ChunkSample, Refused, APRON};
use crate::space::coord::zoom::{units, Zoom};
use crate::space::layer::lattice::GROUND;

/// What is in a cell: the ground's, a way's or a pier's look, or what is
/// placed, air included. Where there is none, it is void.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Block {
    Ground(GroundId),
    Placed(u32),
}

impl Block {
    /// Whether it fills its cell: ground does, and of what is placed a cube
    /// that is not air; decor stands in a cell it leaves empty.
    pub fn fills(self) -> bool {
        match self {
            Block::Ground(_) => true,
            Block::Placed(state) => !object_value_is_air(state) && cube_of(state).is_some(),
        }
    }
}

/// Bottom to top exclusive, in the sample's cells: what is placed, or a
/// deck's span.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Stand {
    pub lo: i32,
    pub hi: i32,
    pub block: Block,
}

/// The world's units a run's cell is tall: a leaf cell's, or past the
/// leaves the run's cell.
pub fn z_unit(runs: Zoom) -> f64 {
    units(runs.max(GROUND.cell()))
}

/// What stands in each column of the sample's grid, apron included, lowest
/// first and none overlapping, from `cells` folded to `runs`. What is painted later wins a cell, as the layers are laid: the
/// ground, then bridges and tunnels, then what is placed, air included.
pub fn stands_of(sample: &ChunkSample, cells: &HashMap<[i64; 3], u32>, (cx0, cy0): (i64, i64), runs: Zoom, stands: &mut Vec<(u32, Stand)>) -> Result<(), Refused> {
    let span = i64::from(sample.span);
    let apron = APRON as i64;
    // The sample's cells a cell spans upright.
    let s = (units(runs) / z_unit(runs)) as i32;
    let columns = (sample.span * sample.span) as usize;
    stands.clear();
    // A deck span cut by n placed stands is n + 1 pieces.
    grow(stands, 2 * cells.len() + sample.over_at[columns] as usize)?;
    for (&[x, y, z], &state) in cells {
        let (px, py) = (x - cx0 + apron, y - cy0 + apron);
        if (0..span).contains(&px) && (0..span).contains(&py) {
            let (lo, hi) = (z as i32 * s, z as i32 * s + s);
            stands.push(((py * span + px) as u32, Stand { lo, hi, block: Block::Placed(state) }));
        }
    }
    stands.sort_unstable_by_key(|&(col, s)| (col, s.lo));
    // A stack of one material is one stand, however many leaves it was folded from.
    stands.dedup_by(|above, below| {
        let joins = above.0 == below.0 && above.1.lo == below.1.hi && above.1.block == below.1.block;
        if joins {
            below.1.hi = above.1.hi;
        }
        joins
    });
    let placed = stands.len();
    // A deck's span keeps only what nothing placed stands in.
    let mut next = 0;
    for i in 0..columns {
        let first = next;
        while next < placed && stands[next].0 == i as u32 {
            next += 1;
        }
        for d in &sample.over[sample.over_at[i] as usize..sample.over_at[i + 1] as usize] {
            let block = Block::Ground(d.block);
            let mut lo = d.lo;
            for k in first..next {
                let p = stands[k].1;
                if p.hi <= lo || p.lo >= d.hi {
                    continue;
                }
                if p.lo > lo {
                    stands.push((i as u32, Stand { lo, hi: p.lo, block }));
                }
                lo = lo.max(p.hi);
            }
            if lo < d.hi {
                stands.push((i as u32, Stand { lo, hi: d.hi, block }));
            }
        }
    }
    stands.sort_unstable_by_key(|&(col, s)| (col, s.lo));
    Ok(())
}

/// Column `i`'s blocks in the heights `[lo, hi)`, lowest first, each run
/// inclusive: the ground between what stands there, and each stand, air
/// included. Only what is asked is composed, so a chunk composes its own
/// heights and nothing under them. `stands` is the column's.
pub fn compose<E>(sample: &ChunkSample, stands: &[(u32, Stand)], i: usize, [lo, hi]: [i32; 2], mut run: impl FnMut(i32, i32, Block) -> Result<(), E>) -> Result<(), E> {
    let mut z = lo;
    for &(_, s) in stands.iter().filter(|(_, s)| s.hi > lo && s.lo < hi) {
        sample.ground_runs(i, z, s.lo, |lo, hi, g| run(lo, hi, Block::Ground(g)))?;
        run(s.lo.max(lo), s.hi.min(hi) - 1, s.block)?;
        z = s.hi;
    }
    sample.ground_runs(i, z, hi, |lo, hi, g| run(lo, hi, Block::Ground(g)))
}

#[cfg(test)]
#[path = "stand.test.rs"]
mod test;
