// pxl-render/mesh/paint/level.rs

//! A level-k chunk's runs, a level cell a column across with heights in z16
//! cells. A column is a whole run from the floor to the ground: a part-filled
//! level cell would have to be drawn as something inside the cell, and in flat
//! places the whole level turned to cubes standing apart.

use std::collections::HashMap;

use crate::mesh::paint::object::adapter::Painter;
use crate::mesh::paint::terrain::adapter::ChunkSample;
use crate::mesh::voxel_scan::{ScanContext, APRON, COL_SPAN};

/// Bottom to top exclusive, in z16 cells: a folded object, air, or a deck's span.
pub type Stands = (i32, i32, Option<(u32, u8)>);

/// Every column of the padded grid, apron included. `step` is z16 cells a
/// level cell spans; heights are z16 cells.
pub fn write_level_runs(
    direct: &mut ScanContext,
    sample: &ChunkSample,
    painter: &Painter,
    cells: &HashMap<[i64; 3], u32>,
    (cx0, cy0): (i64, i64),
    step: u32,
    stands: &mut Vec<(u32, Stands)>,
) -> Result<(), Refused> {
    let span = COL_SPAN as i64;
    let apron = APRON as i64;
    let s = step as i32;
    let mut by_column: Vec<Vec<(i32, u32)>> = vec![Vec::new(); (COL_SPAN * COL_SPAN) as usize];
    for (&[x, y, z], &state) in cells {
        let (px, py) = (x - cx0 + apron, y - cy0 + apron);
        if (0..span).contains(&px) && (0..span).contains(&py) {
            by_column[(py * span + px) as usize].push((z as i32 * s, state));
        }
    }
    for (i, objects) in by_column.iter_mut().enumerate() {
        objects.sort_unstable_by_key(|&(z, _)| z);
        direct.direct_column(i as u32);
        let ground = |direct: &mut ScanContext, from: i32, to: i32| sample.ground_runs(i, from, to, |lo, hi, m, l| direct.direct_run(lo, hi, m, l));
        let mut z = sample.floor;
        for &(oz, state) in objects.iter() {
            if oz < z {
                continue;
            }
            ground(direct, z, oz);
            if let Some((m, l)) = painter.material_of(state) {
                direct.direct_run(oz, oz + s - 1, m, l);
            }
            z = oz + s;
        }
        ground(direct, z, i32::MAX);
    }
}
