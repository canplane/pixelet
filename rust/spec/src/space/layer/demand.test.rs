// pxl-spec/space/layer/demand.test.rs

use super::*;
use crate::space::camera::grid::*;
use crate::space::coord::zoom::FINEST_ZOOM;
use crate::space::layer::lattice::demand_reach;
use crate::space::layer::lattice::GROUND;
use crate::space::layer::tile::*;
use pxl_spec_core::param::tile::TOP_ZOOM;

/// The ground tile a distance folds from, past the pyramid's ends too, as
/// the rule reads it: squared.
fn zoom_at(r: f64) -> i32 {
    GROUND.tile_zoom_of_grid(cell_at(r * r))
}

/// The distance just short of `r` as the rule reads a distance: the f32
/// before it (pxl-spec-core `grid::cell_at`).
fn short_of(r: f64) -> f64 {
    f64::from(f32::from_bits((r as f32).to_bits() - 1))
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
    tiles_within(zoom, foot, demand_reach(GROUND, zoom)).collect()
}

#[test]
fn a_zoom_demands_at_most_its_reach_in_tiles_wherever_the_foot_stands() {
    for zoom in TOP_ZOOM..=LEAF_ZOOM {
        for foot in feet(zoom) {
            let n = demands(zoom, foot).len() as u32;
            assert!(n <= REACH_TILES, "z{zoom} at {foot:?}: {n} tiles");
        }
    }
}

#[test]
fn the_parents_of_a_zooms_tiles_lie_in_the_next_coarser_ones() {
    for zoom in TOP_ZOOM + 1..=LEAF_ZOOM {
        for foot in feet(zoom) {
            let above = demands(zoom - 1, foot);
            for p in demands(zoom, foot) {
                assert!(
                    above.contains(&p.parent()),
                    "z{zoom} at {foot:?}: {p:?}'s parent outside"
                );
            }
        }
    }
}

/// The reach, over the window's margin, is exactly where the view stops
/// asking for a zoom.
#[test]
fn the_fault_bound_is_where_a_zoom_ends() {
    for zoom in TOP_ZOOM..=LEAF_ZOOM {
        let r = demand_reach(GROUND, zoom) / WINDOW_MARGIN;
        assert_eq!(
            zoom_at(short_of(r)),
            zoom as i32,
            "z{zoom} is asked for up to its radius"
        );
        assert_eq!(zoom_at(r), zoom as i32 - 1, "and not at it");
    }
}

#[test]
fn a_zoom_is_the_distance_alone_and_the_same_ratio_at_every_zoom() {
    let mut last = i32::MAX;
    // Inside the world: no cell is coarser than z0's, and none finer than a
    // position names (`FINEST_ZOOM`), even scaled finer by 2^3 below.
    for r in core::iter::successors(Some(split_distance(FINEST_ZOOM - 3)), |r| Some(r * 1.01))
        .take_while(|r| r * 32.0 < split_distance(0))
    {
        let z = zoom_at(r);
        assert!(z <= last, "{r}: z{z} after z{last}");
        last = z;
        for n in [-3, 1, 5] {
            assert_eq!(zoom_at(libm::ldexp(r, n)), z - n, "{r} scaled by 2^{n}");
        }
    }
    for z in TOP_ZOOM as i32..=LEAF_ZOOM as i32 + 3 {
        assert_eq!(
            zoom_at(split_distance(GROUND.grid_zoom(z as Zoom))),
            z,
            "a zoom begins at its split distance"
        );
        assert_eq!(
            zoom_at(short_of(split_distance(GROUND.grid_zoom(z as Zoom - 1)))),
            z,
            "and ends where the next coarser begins"
        );
    }
}
