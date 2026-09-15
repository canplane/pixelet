// pxl_core/module/world/decor/scatter.test.rs

use super::*;
use crate::module::world::decor::car_kind::CAR_RULES;
use crate::module::world::decor::train_kind::TRAIN_RULES;
use crate::module::world::decor::tree_kind::TREE_RULES;
use xpute_core::kit::array::U8Array;

// A whole page's lanes. scatter.ts's tests used eight rows and let an
// alignment vote near the last row read past the array, which JavaScript
// answers with undefined; a page is always PAGE_CELL_SPAN² cells, so the
// fixture is one.
const SPAN: usize = PAGE_CELL_SPAN as usize;
const PI: f64 = core::f64::consts::PI;

struct Lanes {
    land: Vec<u8>,
    water: Vec<u8>,
    structure: Vec<u8>,
    transport: Vec<u8>,
}

impl Lanes {
    fn new(land_fill: u8) -> Lanes {
        let size = SPAN * SPAN;
        let void = cube_types::common::VOID;
        Lanes {
            land: vec![land_fill; size],
            water: vec![void; size],
            structure: vec![void; size],
            transport: vec![void; size],
        }
    }

    fn ground(&self) -> Ground {
        Ground {
            land: U8Array::from(&self.land),
            water: U8Array::from(&self.water),
            structure: U8Array::from(&self.structure),
            transport: U8Array::from(&self.transport),
        }
    }
}

fn seed(tag: u64, density: f64, orientation: DecorOrientationMode) -> DecorKindSeed {
    DecorKindSeed { tag, density, variant_count: 5, orientation }
}

const RANDOM: DecorOrientationMode = DecorOrientationMode::Random;
const ALIGN: DecorOrientationMode = DecorOrientationMode::AlignTransport;

#[test]
fn scatter_decor_candidates_density_1_places_on_every_eligible_cell_in_range() {
    let g = Lanes::new(cube_types::land::GREEN_LIGHT).ground();
    assert_eq!(scatter_decor_candidates(&g, 0, 0, 4, 42, &seed(1, 1.0, RANDOM), &TREE_RULES(), 1.0).len(), 16);
}

#[test]
fn scatter_decor_candidates_density_0_never_places_regardless_of_eligibility() {
    let g = Lanes::new(cube_types::land::GREEN_LIGHT).ground();
    assert_eq!(scatter_decor_candidates(&g, 0, 0, 4, 42, &seed(1, 0.0, RANDOM), &TREE_RULES(), 1.0).len(), 0);
}

#[test]
fn scatter_decor_candidates_ineligible_land_type_not_green_is_never_a_candidate_even_at_density_1() {
    let g = Lanes::new(cube_types::land::RESIDENTIAL).ground();
    assert_eq!(scatter_decor_candidates(&g, 0, 0, 4, 42, &seed(1, 1.0, RANDOM), &TREE_RULES(), 1.0).len(), 0);
}

#[test]
fn scatter_decor_candidates_bare_land_counts_as_eligible() {
    let g = Lanes::new(cube_types::land::BARE).ground();
    assert_eq!(scatter_decor_candidates(&g, 0, 0, 4, 42, &seed(1, 1.0, RANDOM), &TREE_RULES(), 1.0).len(), 16);
}

#[test]
fn scatter_decor_candidates_a_non_void_water_structure_transport_cell_overrides_an_otherwise_green_cell() {
    let mut lanes = Lanes::new(cube_types::land::GREEN_DEEP);
    lanes.water[2 * SPAN + 1] = cube_types::water::LAKE; // cell (1, 2)
    let c = scatter_decor_candidates(&lanes.ground(), 0, 0, 4, 42, &seed(1, 1.0, RANDOM), &TREE_RULES(), 1.0);
    assert_eq!(c.len(), 15);
    assert!(!c.iter().any(|c| c.cell_x == 1 && c.cell_y == 2));
}

#[test]
fn scatter_decor_candidates_deterministic_identical_inputs_always_produce_identical_output() {
    let g = Lanes::new(cube_types::land::GREEN_LIGHT).ground();
    let kind = seed(7, 0.5, RANDOM);
    let a = scatter_decor_candidates(&g, 0, 0, 8, 1234, &kind, &TREE_RULES(), 1.0);
    let b = scatter_decor_candidates(&g, 0, 0, 8, 1234, &kind, &TREE_RULES(), 1.0);
    assert_eq!(a, b);
    assert!(!a.is_empty() && a.len() < 64);
}

#[test]
fn scatter_decor_candidates_a_different_page_seed_reshuffles_which_cells_get_placed() {
    let g = Lanes::new(cube_types::land::GREEN_LIGHT).ground();
    let kind = seed(7, 0.5, RANDOM);
    assert_ne!(
        scatter_decor_candidates(&g, 0, 0, 8, 1234, &kind, &TREE_RULES(), 1.0),
        scatter_decor_candidates(&g, 0, 0, 8, 5678, &kind, &TREE_RULES(), 1.0)
    );
}

#[test]
fn scatter_decor_candidates_every_candidate_s_variant_is_within_0_variant_count() {
    let g = Lanes::new(cube_types::land::GREEN_LIGHT).ground();
    for c in scatter_decor_candidates(&g, 0, 0, 8, 99, &seed(3, 1.0, RANDOM), &TREE_RULES(), 1.0) {
        assert!(c.variant < 5);
    }
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
fn scatter_decor_candidates_align_transport_faces_along_x_when_the_road_runs_east_west() {
    let mut lanes = Lanes::new(cube_types::common::VOID);
    for x in [1, 2, 3] {
        lanes.transport[2 * SPAN + x] = cube_types::transport::ROAD_MOTORWAY;
    }
    let c = scatter_decor_candidates(&lanes.ground(), 0, 0, 8, 42, &seed(1, 1.0, ALIGN), &CAR_RULES(), 1.0);
    let at = c.iter().find(|c| c.cell_x == 2 && c.cell_y == 2).expect("a car at (2, 2)");
    assert_on_axis(at.rotation, PI / 2.0);
}

#[test]
fn scatter_decor_candidates_align_transport_faces_along_y_when_the_road_runs_north_south() {
    let mut lanes = Lanes::new(cube_types::common::VOID);
    for y in [1, 2, 3] {
        lanes.transport[y * SPAN + 2] = cube_types::transport::ROAD_MOTORWAY;
    }
    let c = scatter_decor_candidates(&lanes.ground(), 0, 0, 8, 42, &seed(1, 1.0, ALIGN), &CAR_RULES(), 1.0);
    let at = c.iter().find(|c| c.cell_x == 2 && c.cell_y == 2).expect("a car at (2, 2)");
    assert_on_axis(at.rotation, 0.0);
}

#[test]
fn scatter_decor_candidates_align_transport_isn_t_thrown_off_by_a_single_cell_diagonal_stairstep_on_an_otherwise_vertical_rail() {
    let mut lanes = Lanes::new(cube_types::common::VOID);
    for y in 0..=6 {
        lanes.transport[y * SPAN + 2] = cube_types::transport::RAIL_MAIN;
    }
    lanes.transport[3 * SPAN + 3] = cube_types::transport::RAIL_MAIN;
    let c = scatter_decor_candidates(&lanes.ground(), 0, 0, 8, 42, &seed(1, 1.0, ALIGN), &TRAIN_RULES(), 1.0);
    let at = c.iter().find(|c| c.cell_x == 2 && c.cell_y == 3).expect("a train at (2, 3)");
    assert_on_axis(at.rotation, 0.0);
}

#[test]
fn scatter_decor_candidates_align_transport_headings_aren_t_all_identical_along_one_straight_road() {
    let mut lanes = Lanes::new(cube_types::common::VOID);
    for x in 0..8 {
        lanes.transport[2 * SPAN + x] = cube_types::transport::ROAD_MOTORWAY;
    }
    let c = scatter_decor_candidates(&lanes.ground(), 0, 0, 8, 42, &seed(1, 1.0, ALIGN), &CAR_RULES(), 1.0);
    let mut headings: Vec<u64> = c.iter().filter(|c| c.cell_y == 2).map(|c| c.rotation.to_bits()).collect();
    headings.sort_unstable();
    headings.dedup();
    assert!(headings.len() > 1, "expected at least one flipped heading among 8 candidates on the same road");
}

#[test]
fn scatter_decor_candidates_a_road_cell_not_road_motorway_is_never_a_car_candidate() {
    let mut lanes = Lanes::new(cube_types::common::VOID);
    lanes.transport[2 * SPAN + 2] = cube_types::transport::ROAD_DEFAULT;
    assert_eq!(scatter_decor_candidates(&lanes.ground(), 0, 0, 8, 42, &seed(1, 1.0, ALIGN), &CAR_RULES(), 1.0).len(), 0);
}
