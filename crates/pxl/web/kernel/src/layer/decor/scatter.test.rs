// pxl-kernel/layer/decor/scatter.test.rs

use super::*;
use crate::layer::decor::kind::{car, train, tree};
use pxl_spec::bake::pack::with_channel;

// A whole page's lanes, because that is what a page is: PAGE_CELLS²
// cells. A fixture of fewer rows lets an alignment vote near the last row
// read past the lane.
const SPAN: usize = PAGE_CELLS as usize;
const PI: f64 = core::f64::consts::PI;

fn transport_page(cells: &[(usize, usize)], t: u8) -> Vec<GroundCell> {
    let mut lane = vec![0 as GroundCell; SPAN * SPAN];
    for &(x, y) in cells {
        lane[y * SPAN + x] = with_channel(0, GroundChannelKey::TRANSPORT, t);
    }
    lane
}

/// A cell holding `land`, `water`, `structure` and `transport`.
fn cell(land: u8, water: u8, structure: u8, transport: u8) -> GroundCell {
    [
        (GroundChannelKey::LAND, land),
        (GroundChannelKey::WATER, water),
        (GroundChannelKey::STRUCTURE, structure),
        (GroundChannelKey::TRANSPORT, transport),
    ]
    .into_iter()
    .fold(0, |c, (key, t)| with_channel(c, key, t))
}

const V: u8 = cube_types::common::void;

#[test]
fn a_tree_stands_on_green_and_bare_land_and_on_nothing_else() {
    for land in [cube_types::land::green_light, cube_types::land::green_deep, cube_types::land::bare] {
        assert!(tree::DEF.rules.allows(cell(land, V, V, V)), "land {land}");
    }
    assert!(!tree::DEF.rules.allows(cell(cube_types::land::residential, V, V, V)));
}

#[test]
fn any_painted_water_structure_or_transport_overrides_otherwise_green_land() {
    let green = cube_types::land::green_deep;
    assert!(tree::DEF.rules.allows(cell(green, V, V, V)));
    assert!(!tree::DEF.rules.allows(cell(green, cube_types::water::lake, V, V)));
    assert!(!tree::DEF.rules.allows(cell(green, V, cube_types::structure::building, V)));
    assert!(!tree::DEF.rules.allows(cell(green, V, V, cube_types::transport::road_motorway)));
}

#[test]
fn a_car_stands_on_a_motorway_and_not_on_a_plain_road_or_a_rail() {
    assert!(car::DEF.rules.allows(cell(V, V, V, cube_types::transport::road_motorway)));
    assert!(!car::DEF.rules.allows(cell(V, V, V, cube_types::transport::road_default)));
    assert!(!car::DEF.rules.allows(cell(V, V, V, cube_types::transport::rail_main)));
}

#[test]
fn a_train_stands_on_a_rail_and_not_on_a_motorway() {
    assert!(train::DEF.rules.allows(cell(V, V, V, cube_types::transport::rail_main)));
    assert!(!train::DEF.rules.allows(cell(V, V, V, cube_types::transport::road_motorway)));
}

/// align_transport also randomly flips the heading +PI, so these check
/// membership in {axis, axis + PI} rather than one exact value.
fn assert_on_axis(rotation: f64, axis: f64) {
    let tau = PI * 2.0;
    let normalized = ((rotation % tau) + tau) % tau;
    let a = ((axis % tau) + tau) % tau;
    let b = (a + PI) % tau;
    assert!((normalized - a).abs() < 1e-9 || (normalized - b).abs() < 1e-9, "expected {normalized} to be {a} or {b}");
}

#[test]
fn transport_aligned_rotation_faces_along_x_when_the_road_runs_east_west() {
    let lane = transport_page(&[(1, 2), (2, 2), (3, 2)], cube_types::transport::road_motorway);
    assert_on_axis(transport_aligned_rotation(&lane, 2, 2, false), PI / 2.0);
}

#[test]
fn transport_aligned_rotation_faces_along_y_when_the_road_runs_north_south() {
    let lane = transport_page(&[(2, 1), (2, 2), (2, 3)], cube_types::transport::road_motorway);
    assert_on_axis(transport_aligned_rotation(&lane, 2, 2, false), 0.0);
}

#[test]
fn transport_aligned_rotation_isn_t_thrown_off_by_a_single_cell_diagonal_stairstep_on_an_otherwise_vertical_rail() {
    let mut cells: Vec<(usize, usize)> = (0..=6).map(|y| (2, y)).collect();
    cells.push((3, 3));
    let lane = transport_page(&cells, cube_types::transport::rail_main);
    assert_on_axis(transport_aligned_rotation(&lane, 2, 3, false), 0.0);
}

#[test]
fn transport_aligned_rotation_flips_a_heading_by_a_half_turn_so_one_road_isn_t_all_one_way() {
    let cells: Vec<(usize, usize)> = (0..8).map(|x| (x, 2)).collect();
    let lane = transport_page(&cells, cube_types::transport::road_motorway);
    let (a, b) = (transport_aligned_rotation(&lane, 3, 2, false), transport_aligned_rotation(&lane, 3, 2, true));
    assert_ne!(a.to_bits(), b.to_bits());
    assert_on_axis(b, PI / 2.0);
}

#[test]
fn transport_aligned_rotation_an_isolated_cell_has_no_axis_and_falls_back_to_zero() {
    let lane = transport_page(&[(4, 4)], cube_types::transport::road_motorway);
    assert_eq!(transport_aligned_rotation(&lane, 4, 4, false), 0.0);
}
