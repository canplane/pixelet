// pxl-spec/space/lod.test.rs

use super::*;

/// The ground tile a distance folds from, past the pyramid's ends too.
fn zoom_at(r: f64) -> i32 {
    GROUND.grid_z(cell_at(r))
}

/// Feet on a tile's corners, edges and inside, either side of the origin.
fn feet(zoom: Zoom) -> Vec<[f64; 2]> {
    let span = units(zoom);
    let fractions = [0.0, 1e-9, 0.25, 0.5, 0.75, 1.0 - 1e-9];
    let mut out = Vec::new();
    for [px, py] in [[0.0, 0.0], [-3.0, 5.0], [17.0, -11.0]] {
        for fx in fractions {
            for fy in fractions {
                out.push([(px + fx) * span, (py + fy) * span]);
            }
        }
    }
    out
}

fn demands(zoom: Zoom, foot: [f64; 2]) -> Vec<Tile> {
    tiles_within(zoom, foot, demand_reach(zoom)).collect()
}

#[test]
fn a_zoom_demands_at_most_its_reach_in_tiles_wherever_the_foot_stands() {
    for zoom in TOP_Z..=LEAF_Z {
        for foot in feet(zoom) {
            let n = demands(zoom, foot).len() as u32;
            assert!(n <= REACH_TILES, "z{zoom} at {foot:?}: {n} tiles");
        }
    }
}

#[test]
fn the_parents_of_a_zooms_tiles_lie_in_the_next_coarser_ones() {
    for zoom in TOP_Z + 1..=LEAF_Z {
        for foot in feet(zoom) {
            let above = demands(zoom - 1, foot);
            for p in demands(zoom, foot) {
                assert!(above.contains(&p.parent()), "z{zoom} at {foot:?}: {p:?}'s parent outside");
            }
        }
    }
}

/// Wherever an observation hits, the tile it asks for is within that zoom's reach.
#[test]
fn a_hit_asks_for_a_tile_inside_the_fault_bound() {
    for zoom in TOP_Z..=LEAF_Z {
        for foot in feet(zoom) {
            let tiles = demands(zoom, foot);
            for height in [0.0, 3.0, 1000.0] {
                for k in 0..64 {
                    let angle = k as f64 * 0.37;
                    for run in [0.5, 7.0, 100.0, 383.9, 900.0, 5_000.0, 40_000.0, 300_000.0] {
                        let at = [foot[0] + run * libm::cos(angle), foot[1] + run * libm::sin(angle)];
                        let asked = zoom_at(libm::hypot(run, height)).min(LEAF_Z as i32);
                        if asked != zoom as i32 {
                            continue;
                        }
                        let [x, y] = [at[0] / units(zoom), at[1] / units(zoom)].map(|c| c.floor() as i64);
                        assert!(tiles.contains(&tile_at(zoom, [x, y])), "z{zoom}: a hit {run} along the ground from {foot:?}");
                    }
                }
            }
        }
    }
}

/// The reach is exactly where a zoom stops being asked for.
#[test]
fn the_fault_bound_is_where_a_zoom_ends_and_holds_no_chunk() {
    for zoom in TOP_Z..=LEAF_Z {
        let r = demand_reach(zoom);
        assert_eq!(zoom_at(r * (1.0 - 1e-12)), zoom as i32, "z{zoom} is asked for up to its radius");
        assert_eq!(zoom_at(r), zoom as i32 - 1, "and not at it");
    }
}

#[test]
fn a_zoom_is_the_distance_alone_and_the_same_ratio_at_every_zoom() {
    let mut last = i32::MAX;
    // Inside the world: no cell is coarser than z0's.
    for r in (0..).map(|k| 0.01 * 1.01f64.powi(k)).take_while(|r| r * 32.0 < split_distance(0)) {
        let z = zoom_at(r);
        assert!(z <= last, "{r}: z{z} after z{last}");
        last = z;
        for n in [-3, 1, 5] {
            assert_eq!(zoom_at(r * 2f64.powi(n)), z - n, "{r} scaled by 2^{n}");
        }
    }
    for z in TOP_Z as i32..=LEAF_Z as i32 + 3 {
        assert_eq!(zoom_at(split_distance(GROUND.grid_cell(z as Zoom))), z, "a zoom begins at its split distance");
        assert_eq!(zoom_at(split_distance(GROUND.grid_cell(z as Zoom - 1)) * (1.0 - 1e-12)), z, "and ends where the next coarser begins");
    }
}
