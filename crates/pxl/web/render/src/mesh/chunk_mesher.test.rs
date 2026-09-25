// pxl-render/mesh/chunk_mesher.test.rs

use super::*;

/// One run as a column holds it: its heights, its colour and its layer.
struct Run {
    z0: i32,
    z1: i32,
    rgb: [u8; 3],
    layer: u32,
}

/// A set of `span`² columns, `runs` a column in the order the columns are
/// indexed. Every run is whole-cell terrain.
fn set_of(span: u32, columns: &[Vec<Run>]) -> OwnedRunSet {
    let mut set = OwnedRunSet {
        span,
        apron: 1,
        ..OwnedRunSet::default()
    };
    for column in columns {
        set.start.push(set.z0.len() as u32);
        for run in column {
            set.z0.push(run.z0);
            set.z1.push(run.z1);
            set.r.push(run.rgb[0]);
            set.g.push(run.rgb[1]);
            set.b.push(run.rgb[2]);
            set.layer_taps.push(run.layer);
            set.material_class.push(MATERIAL_LAND);
            set.sub_sz.push(SUB_CELL_NONE);
            for lane in [&mut set.sub_sx, &mut set.sub_sy, &mut set.sub_ox, &mut set.sub_oy, &mut set.sub_oz] {
                lane.push(0);
            }
        }
    }
    set.start.push(set.z0.len() as u32);
    set
}

fn ground(z1: i32) -> Vec<Run> {
    vec![Run { z0: 0, z1, rgb: [0, 0, 0], layer: 0 }]
}

#[test]
fn a_cell_walled_in_on_every_side_shows_only_its_top_and_its_bottom() {
    // Three columns a side, the middle one as tall as its neighbours.
    let columns: Vec<Vec<Run>> = (0..9).map(|_| ground(3)).collect();
    let set = set_of(3, &columns);
    let view = set.view();
    let faces = exposed_faces(&view, 4, 0, 0, 3);
    assert_eq!(faces, 2.0, "a buried run keeps its top and its bottom alone");
}

#[test]
fn a_wall_shows_the_side_it_stands_above() {
    // The middle column stands four cells over its neighbours' one.
    let columns: Vec<Vec<Run>> = (0..9).map(|k| ground(if k == 4 { 4 } else { 0 })).collect();
    let set = set_of(3, &columns);
    let view = set.view();
    // Its top, its bottom, and four sides of the four cells standing clear.
    assert_eq!(exposed_faces(&view, 4, 0, 0, 4), 2.0 + 4.0 * 4.0);
}

#[test]
fn a_column_off_the_sets_edge_is_air_and_leaves_the_side_open() {
    let columns: Vec<Vec<Run>> = (0..9).map(|_| ground(0)).collect();
    let set = set_of(3, &columns);
    let view = set.view();
    // A corner column: two of its four sides have no column beyond them.
    assert_eq!(exposed_faces(&view, 0, 0, 0, 0), 2.0 + 2.0);
}
