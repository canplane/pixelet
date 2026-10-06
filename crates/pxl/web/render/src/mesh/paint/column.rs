// pxl-render/mesh/paint/column.rs

//! The runs of a chunk coarser than a leaf's, a cell a column across with heights
//! in leaf cells. A column is a whole run from the floor to the ground: a
//! part-filled coarse cell would have to be drawn as something inside the cell,
//! and in flat places the whole zoom turned to cubes standing apart.

use crate::mesh::{grow, Refused};
use std::collections::HashMap;

use crate::mesh::paint::object::adapter::Painter;
use crate::mesh::paint::terrain::adapter::ChunkSample;
use crate::mesh::run::{RunWriter, APRON, COL_SPAN};

/// Bottom to top exclusive, in leaf cells: a folded object, air, or a deck's span.
pub type Stands = (i32, i32, Option<(u32, u8)>);

/// Every column of the padded grid, apron included. `step` is leaf cells a
/// cell spans; heights are leaf cells. A cell in `mixed` shows that material
/// over what its value would (`Painter::mean_of`).
#[allow(clippy::too_many_arguments)]
pub fn write_column_runs(
    direct: &mut RunWriter,
    sample: &ChunkSample,
    painter: &Painter,
    cells: &HashMap<[i64; 3], u32>,
    mixed: &HashMap<[i64; 3], (u32, u8)>,
    (cx0, cy0): (i64, i64),
    step: u32,
    stands: &mut Vec<(u32, Stands)>,
) -> Result<(), Refused> {
    let span = COL_SPAN as i64;
    let apron = APRON as i64;
    let s = step as i32;
    let columns = (COL_SPAN * COL_SPAN) as usize;
    stands.clear();
    grow(stands, cells.len() + sample.span_at[columns] as usize)?;
    for (&[x, y, z], &state) in cells {
        let (px, py) = (x - cx0 + apron, y - cy0 + apron);
        if (0..span).contains(&px) && (0..span).contains(&py) {
            let material = painter.material_of(state).map(|m| mixed.get(&[x, y, z]).copied().unwrap_or(m));
            stands.push(((py * span + px) as u32, (z as i32 * s, z as i32 * s + s, material)));
        }
    }
    for i in 0..columns {
        for d in &sample.spans[sample.span_at[i] as usize..sample.span_at[i + 1] as usize] {
            stands.push((i as u32, (d.lo, d.hi, Some((d.material, d.layer)))));
        }
    }
    stands.sort_unstable_by_key(|&(col, (z, _, _))| (col, z));
    let mut next = 0;
    for i in 0..columns {
        direct.column(i as u32);
        let ground = |direct: &mut RunWriter, from: i32, to: i32| sample.ground_runs(i, from, to, |lo, hi, m, l| direct.run(lo, hi, m, l));
        // Under the floor a stand is inside the ground; where there is none, nothing hides it.
        let mut z = if sample.has_ground(i) { sample.floor } else { i32::MIN };
        while next < stands.len() && stands[next].0 == i as u32 {
            let (lo, hi, stand) = stands[next].1;
            next += 1;
            if lo < z {
                continue;
            }
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
