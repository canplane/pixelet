// pxl_render/mesh/paint/terrain/elevation.test.rs

use pxl_shared::bake::constant::ELE_PADDED_SPAN;
use pxl_shared::spec::cell::cube_types;
use xpute_core::kit::array::U8Array;

use super::*;

const FLAT_ELE: i16 = 100;

/// A flat source raster: bicubic-interpolating a constant field returns that
/// same constant everywhere, with no edge/padding artifacts to account for —
/// isolates the recess logic from the interpolation math it sits on top of.
fn flat_page_ele() -> Vec<i16> {
    vec![FLAT_ELE; (ELE_PADDED_SPAN * ELE_PADDED_SPAN) as usize]
}

/// Channels of `span`² cells, each lane's cell 0 set as given.
fn ground(span: usize, land: u8, water: u8, structure: u8, transport: u8) -> Ground {
    let lane = |first: u8| {
        let mut v = vec![0u8; span * span];
        v[0] = first;
        U8Array::from(&v)
    };
    Ground {
        land: lane(land),
        water: lane(water),
        structure: lane(structure),
        transport: lane(transport),
    }
}

#[test]
fn dense_ele_region_a_water_channel_cell_is_recessed_by_water_recess_cells() {
    let g = ground(2, 0, cube_types::water::river, 0, 0);
    let mut out = [0i16; 4];
    dense_ele_region(&flat_page_ele(), 0, 0, 2, Some(&g), &mut out);
    assert_eq!(out[0], FLAT_ELE - WATER_RECESS_CELLS);
    assert_eq!(out[1], FLAT_ELE);
}

#[test]
fn dense_ele_region_land_sea_is_recessed_too_even_though_it_s_not_the_water_channel() {
    let g = ground(2, cube_types::land::sea, 0, 0, 0);
    let mut out = [0i16; 4];
    dense_ele_region(&flat_page_ele(), 0, 0, 2, Some(&g), &mut out);
    assert_eq!(out[0], FLAT_ELE - WATER_RECESS_CELLS);
}

#[test]
fn dense_ele_region_a_structure_over_water_e_g_a_bridge_is_not_recessed() {
    let g = ground(2, 0, cube_types::water::river, cube_types::structure::building, 0);
    let mut out = [0i16; 4];
    dense_ele_region(&flat_page_ele(), 0, 0, 2, Some(&g), &mut out);
    assert_eq!(out[0], FLAT_ELE);
}

#[test]
fn dense_ele_region_transport_over_water_e_g_a_road_is_not_recessed() {
    let g = ground(2, 0, cube_types::water::lake, 0, cube_types::transport::road_default);
    let mut out = [0i16; 4];
    dense_ele_region(&flat_page_ele(), 0, 0, 2, Some(&g), &mut out);
    assert_eq!(out[0], FLAT_ELE);
}

#[test]
fn dense_ele_region_omitting_ground_recesses_nothing() {
    let mut out = [0i16; 4];
    dense_ele_region(&flat_page_ele(), 0, 0, 2, None, &mut out);
    assert_eq!(out[0], FLAT_ELE);
    assert_eq!(out[1], FLAT_ELE);
}

#[test]
fn is_water_family_at_plain_land_is_not_water_family() {
    let g = ground(1, cube_types::land::bare, 0, 0, 0);
    assert!(!is_water_family_at(&g, 0));
}
