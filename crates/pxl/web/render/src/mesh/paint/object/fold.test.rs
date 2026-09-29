// pxl-render/mesh/paint/object/fold.test.rs

use super::*;
use pxl_spec::bake::constant::CELL_STEPS;
use pxl_spec::cell::cube_types::Block;
use pxl_spec::cell::object::{placeable_cube_num_id, PlaceableCube};
use pxl_spec::space::pyramid::GROUND;

const AIR: u32 = pxl_spec::cell::cube_types::common::air as u32;

/// A solid cube's state, as the catalog numbers it.
fn red() -> u32 {
    placeable_cube_num_id(PlaceableCube::Block(Block::red))
}

fn painter() -> Painter {
    Painter::new()
}

/// A cell-sized entry's packed position at cell (x, y, z) of its band.
fn cell_pos(x: u32, y: u32, z: u32) -> u32 {
    let s = CELL_STEPS;
    pxl_spec::bake::constant::pack_pos(x * s, y * s, z * s, CELL)
}

/// Where the test column's corner is at `level`, in that level's cells.
fn origin(level: u32) -> [i64; 2] {
    let s = GROUND.scale(level);
    [s.index(64), s.index(-64)]
}

fn fold(cells: &[(u32, u32, u32, u32)], level: u32) -> HashMap<[i64; 3], u32> {
    let band: Vec<i32> = cells.iter().map(|_| 0).collect();
    let pos: Vec<u32> = cells.iter().map(|&(x, y, z, _)| cell_pos(x, y, z)).collect();
    let state: Vec<u32> = cells.iter().map(|c| c.3).collect();
    let columns = [Entries {
        col_idx: pxl_spec::space::indexer::L2_IDX(pxl_spec::space::pvf3::ivec::gather(64, -64, 0)),
        band: &band,
        pos: &pos,
        state: &state,
    }];
    let mut fold = Fold::default();
    fold.fold(&painter(), columns, GROUND.scale(level), None);
    fold.cells().clone()
}

#[test]
fn a_cell_folds_to_itself_at_level_0() {
    let out = fold(&[(3, 4, 5, red())], 0);
    let [x, y] = origin(0);
    assert_eq!(out.get(&[x + 3, y + 4, 5]), Some(&red()));
    assert_eq!(out.len(), 1);
}

#[test]
fn a_lone_cube_goes_at_the_first_fold_and_a_slab_of_half_stays() {
    assert!(fold(&[(0, 0, 0, red())], 1).is_empty());
    let slab = [(0, 0, 0, red()), (1, 0, 0, red()), (0, 1, 0, red()), (1, 1, 0, red())];
    let [x, y] = origin(1);
    assert_eq!(fold(&slab, 1).get(&[x, y, 0]), Some(&red()));
}

#[test]
fn a_solid_block_keeps_folding_while_it_fills_its_parent() {
    let mut cube = Vec::new();
    for z in 0..4 {
        for y in 0..4 {
            for x in 0..4 {
                cube.push((x, y, z, red()));
            }
        }
    }
    let out = fold(&cube, 2);
    let [x, y] = origin(2);
    assert_eq!(out.get(&[x, y, 0]), Some(&red()));
    assert_eq!(out.len(), 1);
}

#[test]
fn air_is_a_state_and_folds_like_one() {
    let slab = [(0, 0, 0, AIR), (1, 0, 0, AIR), (0, 1, 0, AIR), (1, 1, 0, AIR), (0, 0, 1, AIR)];
    let [x, y] = origin(1);
    assert_eq!(fold(&slab, 1).get(&[x, y, 0]), Some(&AIR));
}
