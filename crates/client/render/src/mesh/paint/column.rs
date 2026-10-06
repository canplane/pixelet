// pxl-render/mesh/paint/column.rs

//! A chunk's runs, a cell a column across with heights in the sample's
//! cells. A column is a whole run from the floor to the ground: a part-filled
//! cell would have to be drawn as something inside the cell, and in flat
//! places the whole zoom turned to cubes standing apart.

use crate::mesh::{grow, Refused};
use std::collections::HashMap;

use crate::mesh::paint::object::Painter;
use crate::mesh::paint::terrain::adapter::ChunkSample;
use crate::mesh::run::{RunWriter, APRON};

/// Bottom to top exclusive: a folded object, air, or a deck's span.
pub type Stands = (i32, i32, Option<(u32, u8)>);

/// What stands in each column of the sample's grid, apron included, lowest
/// first and none overlapping. `step` is the sample's cells a cell spans
/// upright. What is painted later wins a cell, as the layers are laid: the
/// ground, then decks, then what is placed, air included.
pub fn stands_of(sample: &ChunkSample, painter: &Painter, cells: &HashMap<[i64; 3], u32>, (cx0, cy0): (i64, i64), step: u32, stands: &mut Vec<(u32, Stands)>) -> Result<(), Refused> {
    let span = i64::from(sample.span);
    let apron = APRON as i64;
    let s = step as i32;
    let columns = (sample.span * sample.span) as usize;
    stands.clear();
    // A deck span cut by n placed stands is n + 1 pieces.
    grow(stands, 2 * cells.len() + sample.span_at[columns] as usize)?;
    for (&[x, y, z], &state) in cells {
        let (px, py) = (x - cx0 + apron, y - cy0 + apron);
        if (0..span).contains(&px) && (0..span).contains(&py) {
            stands.push(((py * span + px) as u32, (z as i32 * s, z as i32 * s + s, painter.material_of(state))));
        }
    }
    stands.sort_unstable_by_key(|&(col, (z, _, _))| (col, z));
    let placed = stands.len();
    // A deck's span keeps only what nothing placed stands in.
    let mut next = 0;
    for i in 0..columns {
        let first = next;
        while next < placed && stands[next].0 == i as u32 {
            next += 1;
        }
        for d in &sample.spans[sample.span_at[i] as usize..sample.span_at[i + 1] as usize] {
            let mut lo = d.lo;
            for k in first..next {
                let (plo, phi, _) = stands[k].1;
                if phi <= lo || plo >= d.hi {
                    continue;
                }
                if plo > lo {
                    stands.push((i as u32, (lo, plo, Some((d.material, d.layer)))));
                }
                lo = lo.max(phi);
            }
            if lo < d.hi {
                stands.push((i as u32, (lo, d.hi, Some((d.material, d.layer)))));
            }
        }
    }
    stands.sort_unstable_by_key(|&(col, (z, _, _))| (col, z));
    Ok(())
}

/// The runs of the grid's `rows`, from `stands` (`stands_of`); `next` is the
/// first stand of the first row and is left at the next row's.
pub fn write_rows(direct: &mut RunWriter, sample: &ChunkSample, stands: &[(u32, Stands)], rows: core::ops::Range<u32>, next: &mut usize) -> Result<(), Refused> {
    for i in rows.start * sample.span..rows.end * sample.span {
        let i = i as usize;
        direct.column(i as u32);
        // Ground is drawn from the floor up, between what stands; a stand under the floor is drawn as it is.
        let ground = |direct: &mut RunWriter, from: i32, to: i32| sample.ground_runs(i, from.max(sample.floor), to, |lo, hi, m, l| direct.run(lo, hi, m, l));
        let mut z = i32::MIN;
        while *next < stands.len() && stands[*next].0 == i as u32 {
            let (lo, hi, stand) = stands[*next].1;
            *next += 1;
            ground(direct, z, lo)?;
            if let Some((m, l)) = stand {
                direct.run(lo, hi - 1, m, l)?;
            }
            z = hi;
        }
        ground(direct, z, i32::MAX)?;
    }
    Ok(())
}
