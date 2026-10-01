// pxl-render/mesh/paint/level.rs

//! A level-k chunk's runs: its level page's ground and the object layer
//! folded to the level, a level-k cell a column across and the height in the
//! ground's z16 cells
//! (issue/20260921-level-step-and-zoom-are-one-number.md).
//!
//! The ground is a height field, and only its footprint coarsens with the
//! level: the elevation stays in z16 cells at every level, so the column
//! is a run from the floor to it, whole, and never a level cell part filled.
//! A part cell would have to be drawn as something inside the cell, and
//! every shape that is not the cell leaves a gap beside the next column — in
//! a flat place, where the ground is thin against a coarse cell, the whole
//! level turns to cubes standing apart. In z16 cells a column is exactly
//! as tall as the ground it stands for, and next to a column of the same
//! height it merges.
//!
//! What a placed cube folds to stands over the ground where it lies, a level
//! cell tall, and air folded to a cell takes the ground out of it, as the
//! paint lays each at the finest level. A deck over the column (pxl-spec
//! bake/deck.rs) stands over it as its span says, in z16 cells like the
//! ground.

use std::collections::HashMap;

use crate::mesh::paint::object::adapter::Painter;
use crate::mesh::paint::terrain::adapter::ChunkSample;
use crate::mesh::voxel_scan::{ScanContext, APRON, COL_SPAN};

/// What stands in a column, from its bottom to its top exclusive, in z16
/// cells: an object folded to a level cell, air where its material is none,
/// or a deck's span.
type Stands = (i32, i32, Option<(u32, u8)>);

/// Writes a level chunk's runs into `direct`, an open direct writer: every
/// column of the padded grid, the chunk's own and the apron's. `sample` is the
/// chunk read from its level page; `cells` what the object layer folds to at
/// the level, by level coordinate; `(cx0, cy0)` the chunk's own first column
/// at that coordinate; `step` how many z16 cells a level cell spans.
/// Heights are z16 cells.
pub fn write_level_runs(direct: &mut ScanContext, sample: &ChunkSample, painter: &Painter, cells: &HashMap<[i64; 3], u32>, (cx0, cy0): (i64, i64), step: u32) {
    let span = COL_SPAN as i64;
    let apron = APRON as i64;
    let s = step as i32;
    let mut by_column: Vec<Vec<Stands>> = vec![Vec::new(); (COL_SPAN * COL_SPAN) as usize];
    for (&[x, y, z], &state) in cells {
        let (px, py) = (x - cx0 + apron, y - cy0 + apron);
        if (0..span).contains(&px) && (0..span).contains(&py) {
            by_column[(py * span + px) as usize].push((z as i32 * s, z as i32 * s + s, painter.material_of(state)));
        }
    }
    for (i, column) in by_column.iter_mut().enumerate() {
        for d in &sample.spans[sample.span_at[i] as usize..sample.span_at[i + 1] as usize] {
            column.push((d.lo, d.hi, Some((d.material, d.layer))));
        }
    }
    for (i, column) in by_column.iter_mut().enumerate() {
        column.sort_unstable_by_key(|&(z, _, _)| z);
        direct.direct_column(i as u32);
        let (material, layer) = (sample.material[i], sample.layer_idx[i]);
        let height = sample.ele[i] as i32;
        // The ground from `from` up to `to`, both exclusive at the top.
        let ground = |direct: &mut ScanContext, from: i32, to: i32| {
            let top = height.min(to) - 1;
            if from <= top {
                direct.direct_run(from, top, material, layer);
            }
        };
        let mut z = sample.floor;
        for &(lo, hi, stands) in column.iter() {
            if lo < z {
                continue;
            }
            ground(direct, z, lo);
            if let Some((m, l)) = stands {
                direct.direct_run(lo, hi - 1, m, l);
            }
            z = hi;
        }
        ground(direct, z, i32::MAX);
    }
}
