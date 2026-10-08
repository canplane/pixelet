// pxl-geo-bake/terrain/pipeline/ground.test.rs

use super::*;
use crate::terrain::pipeline::feature::Rank;

/// Column 10 of a way 3 cells wide ending on a cell edge at y 20.
fn painted(cut: [bool; 2]) -> Vec<i64> {
    let mut cells = vec![0 as GroundId; TILE_AREA];
    render_linear_feat(&[[10.5, 10.0], [10.5, 20.0]], 1, &mut cells, 3.0, cut, None);
    (0..SPAN).filter(|&y| cells[cell_idx(10, y)] == 1).collect()
}

#[test]
fn a_way_cut_by_a_flag_stops_at_the_cut_and_a_free_end_rounds_past_it() {
    assert_eq!(painted([false, true]).last(), Some(&19));
    assert!(*painted([false, false]).last().unwrap() > 19);
}

/// An area of the source whose whole is `way_area`, clipped to `at`.
fn area(at: [f64; 4], way_area: f64, look: GroundId, layer: Layer, id: &str) -> Feature {
    let [x0, y0, x1, y1] = at;
    Feature {
        points: vec![[x0, y0], [x1, y0], [x1, y1], [x0, y1]],
        holes: Vec::new(),
        closed: true,
        width: 0.0,
        cut: [false; 2],
        cell: look,
        rank: Rank::by_area(layer, way_area, "", id),
    }
}

fn way(y: f64, width: f64, look: GroundId, id: &str) -> Feature {
    Feature {
        points: vec![[10.0, y], [90.0, y]],
        holes: Vec::new(),
        closed: false,
        width,
        cut: [false; 2],
        cell: look,
        rank: Rank::new(Layer::Road, 100, "", id),
    }
}

fn at(g: &Ground, x: i64, y: i64) -> GroundId {
    g.cells[cell_idx(x, y)]
}

#[test]
fn the_sea_is_the_coast_s_not_a_color_painted_over_land() {
    let features = [
        area([0.0, 0.0, 100.0, 100.0], 10_000.0, ground::water, Layer::Landcover, "pond"),
        area([40.0, 40.0, 60.0, 60.0], 400.0, ground::grass, Layer::Landcover, "grass"),
    ];
    let shore = Coast::Shore(vec![(vec![[0.0, 0.0], [200.0, 0.0], [200.0, 200.0], [0.0, 200.0]], Vec::new())]);
    for coast in [Coast::Land, shore] {
        let g = bake_ground(&features, &coast);
        assert_eq!(at(&g, 50, 50), ground::grass);
        assert_eq!(at(&g, 10, 10), ground::water);
    }
}

#[test]
fn a_source_ranks_alike_on_every_tile_it_is_clipped_to() {
    // A pond's sliver on this tile is smaller than the grass over it; the pond's whole is not.
    let features = [
        area([0.0, 0.0, 10.0, 100.0], 10_000.0, ground::water, Layer::Landcover, "pond"),
        area([0.0, 40.0, 30.0, 60.0], 1_200.0, ground::grass, Layer::Landcover, "grass"),
    ];
    assert_eq!(at(&bake_ground(&features, &Coast::Land), 5, 50), ground::grass);
}

#[test]
fn a_tile_s_ground_does_not_depend_on_its_rows_order() {
    let mut features = vec![
        area([10.0, 10.0, 30.0, 30.0], 400.0, ground::park, Layer::Landcover, "a"),
        area([10.0, 10.0, 30.0, 30.0], 400.0, ground::grass, Layer::Landcover, "b"),
        area([20.0, 20.0, 40.0, 40.0], 400.0, ground::building, Layer::Building, "c"),
        area([20.0, 20.0, 40.0, 40.0], 400.0, ground::building_major, Layer::Building, "d"),
        // Two ways of one rank over one cell: which shows is no row's choice.
        way(70.0, 4.0, ground::residential_road, "south"),
        way(71.0, 4.0, ground::secondary, "north"),
    ];
    let bake = |features: &[Feature]| bake_ground(features, &Coast::Land).cells.into_owned();
    let forward = bake(&features);
    features.reverse();
    assert_eq!(bake(&features), forward);
}
