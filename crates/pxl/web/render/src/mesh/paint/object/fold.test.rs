// pxl-render/mesh/paint/object/fold.test.rs

use super::*;
use crate::mesh::paint::object::adapter::Kind;

const RED: u32 = 20;
const AIR: u32 = pxl_spec::cell::cube_types::common::air as u32;

fn painter() -> Painter {
    let mut p = Painter::new();
    p.teach(RED, Kind::Solid, 0x01ff0000, 0);
    p.teach(AIR, Kind::Air, 0, 0);
    p
}

/// A cell-sized entry's packed position at cell (x, y, z) of its band.
fn cell_pos(x: u32, y: u32, z: u32) -> u32 {
    let s = CELL_STEPS;
    (x * s) | ((y * s) << POS_BITS) | ((z * s) << (2 * POS_BITS)) | (((0 - MIN_LEVEL) as u32) << (3 * POS_BITS))
}

fn fold(cells: &[(u32, u32, u32, u32)], level: u32) -> HashMap<[i64; 3], u32> {
    let band: Vec<i32> = cells.iter().map(|_| 0).collect();
    let pos: Vec<u32> = cells.iter().map(|&(x, y, z, _)| cell_pos(x, y, z)).collect();
    let state: Vec<u32> = cells.iter().map(|c| c.3).collect();
    let columns = [Column {
        cx0: 64,
        cy0: -64,
        entries: Entries {
            band: &band,
            pos: &pos,
            state: &state,
        },
    }];
    let mut out = HashMap::new();
    fold_columns(&painter(), &columns, level, None, &mut out);
    out
}

#[test]
fn a_cell_folds_to_itself_at_level_0() {
    let out = fold(&[(3, 4, 5, RED)], 0);
    assert_eq!(out.get(&[67, -60, 5]), Some(&RED));
    assert_eq!(out.len(), 1);
}

#[test]
fn a_lone_cube_goes_at_the_first_fold_and_a_slab_of_half_stays() {
    assert!(fold(&[(0, 0, 0, RED)], 1).is_empty());
    let slab = [(0, 0, 0, RED), (1, 0, 0, RED), (0, 1, 0, RED), (1, 1, 0, RED)];
    assert_eq!(fold(&slab, 1).get(&[32, -32, 0]), Some(&RED));
}

#[test]
fn a_solid_block_keeps_folding_while_it_fills_its_parent() {
    let mut cube = Vec::new();
    for z in 0..4 {
        for y in 0..4 {
            for x in 0..4 {
                cube.push((x, y, z, RED));
            }
        }
    }
    let out = fold(&cube, 2);
    assert_eq!(out.get(&[16, -16, 0]), Some(&RED));
    assert_eq!(out.len(), 1);
}

#[test]
fn air_is_a_state_and_folds_like_one() {
    let slab = [(0, 0, 0, AIR), (1, 0, 0, AIR), (0, 1, 0, AIR), (1, 1, 0, AIR), (0, 0, 1, AIR)];
    assert_eq!(fold(&slab, 1).get(&[32, -32, 0]), Some(&AIR));
}
