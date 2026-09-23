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

#[test]
fn the_colour_is_the_faces_the_cells_turn_to_the_air() {
    // Red with three faces to the air against blue with one: three parts red.
    let mut acc = ZBucketAccum::default();
    acc.add(1.0, 3.0, linear_of(255, 0, 0));
    acc.add(1.0, 1.0, linear_of(0, 0, 255));
    let mix = acc.colour();
    let want = srgb_of([3.0 / 4.0 * linear_of(255, 0, 0)[0], 0.0, 1.0 / 4.0 * linear_of(0, 0, 255)[2]]);
    assert_eq!(mix, want);
    // And a bucket nothing faces falls back to what its volume holds.
    let mut buried = ZBucketAccum::default();
    buried.add(3.0, 0.0, linear_of(255, 0, 0));
    buried.add(1.0, 0.0, linear_of(0, 0, 255));
    assert_eq!(buried.colour(), srgb_of([3.0 / 4.0 * linear_of(255, 0, 0)[0], 0.0, 1.0 / 4.0 * linear_of(0, 0, 255)[2]]));
}

#[test]
fn a_full_cell_is_a_whole_box_and_a_sparse_one_is_the_cube_root_of_its_fill() {
    // The inset is what the run set carries: a fraction of a cell a side.
    let (offset, scale) = pack_inset(libm::cbrt(0.125));
    assert_eq!(unpack_sub_cell_size(scale), 0.5, "an eighth full is half a side");
    assert!(unpack_sub_cell_size(offset) > 0.24 && unpack_sub_cell_size(offset) < 0.26, "centred in its cell");
}

/// Bakes `set` at `lod_step`, its columns `scale` units across, and gives
/// back each box's words.
fn bake(set: &OwnedRunSet, lod_step: u32, scale: u32) -> Vec<[u32; BOX_WORDS as usize]> {
    let mut ctx = BakeContext::new();
    assert!(ctx.begin(&set.view(), lod_step, scale));
    let mut words: Vec<u32> = Vec::new();
    loop {
        match ctx.step(&set.view()) {
            Step::Working => {}
            Step::NeedMesh(n) => {
                words = vec![0; n as usize];
                // SAFETY: the words live as long as the bake, which finishes
                // below, and nothing else touches them.
                unsafe { ctx.provide_mesh(words.as_mut_ptr(), n) };
            }
            Step::Done => break,
        }
    }
    let mesh = ctx.finish();
    words.as_chunks::<{ BOX_WORDS as usize }>().0[..mesh.box_cnt as usize].to_vec()
}

#[test]
fn a_coarse_cells_box_shows_its_underside_and_is_lit_there_as_it_is_on_top() {
    // Four columns of ground one cell thick, in a cell four units tall: the
    // fill is a quarter, so the box is inset and hangs over its floor.
    let columns: Vec<Vec<Run>> = (0..16).map(|_| ground(0)).collect();
    let set = set_of(4, &columns);
    let boxes = bake(&set, 2, 1);
    let hanging: Vec<_> = boxes.iter().filter(|b| box_of(b).1[0] < 2.0).collect();
    assert!(!hanging.is_empty(), "the coarse tier drew an inset box");
    for b in hanging {
        let faces = b[8];
        assert!(faces & FACE_NZ != 0, "the box's underside is drawn");
        assert!(faces & UNDER_AS_TOP != 0, "and is lit as its top is");
    }
}

/// A box's corner and size in units, from its words.
fn box_of(b: &[u32; BOX_WORDS as usize]) -> ([f32; 3], [f32; 3]) {
    let at = |k: usize| f32::from_bits(b[k]);
    ([at(0), at(1), at(2)], [at(3), at(4), at(5)])
}

/// The boxes of a bake that span a whole cell of `cell` units: the floors.
fn floors(boxes: &[[u32; BOX_WORDS as usize]], cell: f32) -> usize {
    boxes
        .iter()
        .filter(|b| {
            let (_, size) = box_of(b);
            size[0] >= cell && size[1] >= cell
        })
        .count()
}

#[test]
fn the_cells_ground_is_carried_by_a_floor_that_spans_it() {
    // Ground one cell thick under a coarse cell four cells on a side: the
    // box is inset, and a floor spanning the cell is written under it.
    let columns: Vec<Vec<Run>> = (0..36).map(|_| ground(0)).collect();
    let set = set_of(6, &columns);
    let boxes = bake(&set, 4, 1);
    assert!(floors(&boxes, 4.0) > 0, "the cell's floor is drawn: {boxes:?}");
}

#[test]
fn a_bucket_over_another_lays_its_own_floor_too() {
    // The same ground with a bridge four cells over it: the deck is its own
    // bucket, and it carries a floor of its own colour like any other, so a
    // line of them does not read as a row of gaps
    // (issue/20260923-a-coarse-cell-is-isotropic.md).
    let columns: Vec<Vec<Run>> = (0..36)
        .map(|k| {
            let mut column = ground(0);
            if k % 6 == 3 {
                column.push(Run {
                    z0: 4,
                    z1: 4,
                    rgb: [200, 200, 200],
                    layer: 0,
                });
            }
            column
        })
        .collect();
    let set = set_of(6, &columns);
    let boxes = bake(&set, 4, 1);
    let deck: Vec<_> = boxes.iter().filter(|b| box_of(b).0[2] >= 3.5).collect();
    assert!(deck.iter().any(|b| box_of(b).1[0] < 4.0), "the deck's own box is inset");
    assert!(deck.iter().any(|b| box_of(b).1[0] >= 4.0), "and a floor of the deck's colour spans the cell under it: {deck:?}");
}

#[test]
fn a_box_that_merges_cells_carries_no_layer_of_its_own() {
    // Every tier over a leaf's own cells is its colour and nothing else: the
    // texture pack is for the cells, where a top and a side mean something.
    let columns: Vec<Vec<Run>> = (0..16).map(|_| ground(0)).collect();
    let set = set_of(4, &columns);
    for b in bake(&set, 2, 1) {
        assert_eq!(b[7], NO_LAYER * 0x0101_0101, "a coarse box has no layer");
    }
}
