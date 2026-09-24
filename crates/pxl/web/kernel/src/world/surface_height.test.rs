// pxl-kernel/world/surface_height.test.rs

use pxl_spec::cell::cube_types::Block;
use pxl_spec::cell::object::PlaceableCube;
use pxl_spec::space::pvf3::pvf3::{self, Pvf3};

use super::*;
use crate::global::serial;
use crate::layer::object::store::place;

// place()/get_objects_near() are kernel-global state (object/store.rs), not
// scoped per-test — every test below uses its own well-separated local X so
// its own placed objects never land in another test's ±1-cell query
// neighborhood (1.0 is 1, so 10 units apart is comfortably clear).
const MAP_ORIGIN: IVec = 0;

fn put_block(x: f64, y: f64, z: f64) {
    let mut corner = Pvf3::default();
    pvf3::gather(&mut corner, pvf3::from_real(x), pvf3::from_real(y), pvf3::from_real(z));
    place(&corner, 1.0, PlaceableCube::Block(Block::red), false);
}

fn close(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-7, "{a} is not {b}");
}

#[test]
fn surface_z_from_terrain_bare_terrain_with_no_objects_returns_terrain_ele_unchanged() {
    let _g = serial();
    assert_eq!(surface_z_from_terrain(MAP_ORIGIN, 1000.0, 1000.0, 3.5, 3.5), 3.5);
}

#[test]
fn surface_z_from_terrain_single_object_standing_on_the_ground_lands_on_its_top_not_inside_it() {
    let _g = serial();
    let (x, y) = (1010.0, 1000.0);
    put_block(x, y, 0.0);
    close(surface_z_from_terrain(MAP_ORIGIN, x, y, 0.0, 0.0), 1.0);
}

#[test]
fn surface_z_from_terrain_tall_contiguous_stack_lands_on_its_true_top() {
    let _g = serial();
    let (x, y) = (1020.0, 1000.0);
    for level in 0..5 {
        put_block(x, y, level as f64);
    }
    close(surface_z_from_terrain(MAP_ORIGIN, x, y, 0.0, 0.0), 5.0);
}

/// x,O,x,O,x,x (solid, gap, solid, gap, solid, solid) — a 2-tall contiguous
/// segment (levels 4-5) above lone blocks, with a real gap either side.
fn put_gapped_structure(x: f64, y: f64) {
    put_block(x, y, 0.0);
    put_block(x, y, 2.0);
    put_block(x, y, 4.0);
    put_block(x, y, 5.0);
}

#[test]
fn surface_z_from_terrain_gapped_structure_hint_at_the_ground_stops_at_the_first_block_doesn_t_skip_the_gap() {
    let _g = serial();
    let (x, y) = (1030.0, 1000.0);
    put_gapped_structure(x, y);
    close(surface_z_from_terrain(MAP_ORIGIN, x, y, 0.0, 0.0), 1.0);
}

#[test]
fn surface_z_from_terrain_gapped_structure_hint_inside_the_lower_gap_snaps_up_to_the_next_platform_s_top() {
    let _g = serial();
    let (x, y) = (1040.0, 1000.0);
    put_gapped_structure(x, y);
    close(surface_z_from_terrain(MAP_ORIGIN, x, y, 0.0, 1.4), 3.0);
}

#[test]
fn surface_z_from_terrain_gapped_structure_hint_inside_the_upper_segment_climbs_to_its_true_contiguous_top() {
    let _g = serial();
    let (x, y) = (1050.0, 1000.0);
    put_gapped_structure(x, y);
    close(surface_z_from_terrain(MAP_ORIGIN, x, y, 0.0, 4.5), 6.0);
}
