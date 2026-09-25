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

/// A bake's boxes, with what their packed words are read back against.
struct Baked {
    /// Every box's words, laid end to end in the mesh's own format.
    words: Vec<u32>,
    box_words: usize,
    unit: f64,
    heights: Heights,
}

impl Baked {
    fn boxes(&self) -> impl Iterator<Item = &[u32]> {
        self.words.chunks_exact(self.box_words)
    }

    /// A box's corner and size in units.
    fn at(&self, b: &[u32]) -> (V3, [f64; 3]) {
        box_bounds(b, self.unit, self.heights)
    }

    /// The boxes that span a whole cell of `cell` units: the floors.
    fn floors(&self, cell: f64) -> usize {
        self.boxes().filter(|b| self.at(b).1[0] >= cell && self.at(b).1[1] >= cell).count()
    }
}

/// Bakes `set` at `lod_step`, its columns `scale` units across, with the
/// texture pack drawn.
fn bake(set: &OwnedRunSet, lod_step: u32, scale: u32) -> Baked {
    bake_in(set, lod_step, scale, true)
}

/// The same, saying whether the pack is drawn — which is what a summary's
/// colour is taken from.
fn bake_in(set: &OwnedRunSet, lod_step: u32, scale: u32, textured: bool) -> Baked {
    let mut ctx = BakeContext::new();
    assert!(ctx.begin(&set.view(), lod_step, scale, textured));
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
    let box_words = mesh.box_words as usize;
    words.truncate(mesh.box_cnt as usize * box_words);
    Baked {
        words,
        box_words,
        unit: (lod_step * scale) as f64,
        heights: mesh.heights,
    }
}

#[test]
fn a_coarse_cells_box_shows_its_underside_and_is_lit_there_as_it_is_on_top() {
    // Four columns of ground one cell thick, in a cell four units tall: the
    // fill is a quarter, so the box is inset and hangs over its floor.
    let columns: Vec<Vec<Run>> = (0..16).map(|_| ground(0)).collect();
    let set = set_of(4, &columns);
    let baked = bake(&set, 2, 1);
    let hanging: Vec<_> = baked.boxes().filter(|b| baked.at(b).1[0] < 2.0).collect();
    assert!(!hanging.is_empty(), "the coarse tier drew an inset box");
    for b in hanging {
        let faces = b[0];
        assert!(faces & FACE_NZ != 0, "the box's underside is drawn");
        assert!(faces & UNDER_AS_TOP != 0, "and is lit as its top is");
    }
}

#[test]
fn the_cells_ground_is_carried_by_a_floor_that_spans_it() {
    // Ground one cell thick under a coarse cell four cells on a side: the
    // box is inset, and a floor spanning the cell is written under it.
    let columns: Vec<Vec<Run>> = (0..36).map(|_| ground(0)).collect();
    let set = set_of(6, &columns);
    let baked = bake(&set, 4, 1);
    assert!(baked.floors(4.0) > 0, "the cell's floor is drawn: {:?}", baked.words);
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
    let baked = bake(&set, 4, 1);
    let deck: Vec<_> = baked.boxes().filter(|b| baked.at(b).0[2] >= 3.5).collect();
    assert!(deck.iter().any(|b| baked.at(b).1[0] < 4.0), "the deck's own box is inset");
    assert!(deck.iter().any(|b| baked.at(b).1[0] >= 4.0), "and a floor of the deck's colour spans the cell under it: {deck:?}");
}

#[test]
fn a_box_that_merges_cells_carries_no_layer_of_its_own() {
    // Every tier over a leaf's own cells is its colour and nothing else: the
    // texture pack is for the cells, where a top and a side mean something.
    let columns: Vec<Vec<Run>> = (0..16).map(|_| ground(0)).collect();
    let set = set_of(4, &columns);
    let baked = bake(&set, 2, 1);
    assert_eq!(baked.box_words, COARSE_BOX_WORDS as usize, "a coarse box carries no taps at all");
}

#[test]
fn a_summary_is_drawn_in_the_tile_where_the_pack_is_drawn_and_in_the_swatch_where_it_is_not() {
    // A run whose catalog swatch is nothing like its layer's tile. With the
    // pack drawn the summary must follow the tile, because that is what the
    // leaves under it show; with it off it must follow the swatch, for the
    // same reason. The box carries one colour either way — which one is the
    // bake's, and the setting re-sources every chunk when it changes.
    let swatch = [255, 0, 255];
    let tile = layer_mean(0, swatch);
    assert_ne!(tile, swatch, "layer 0 has a tile of its own, or this test says nothing");

    let columns: Vec<Vec<Run>> = (0..16).map(|_| vec![Run { z0: 0, z1: 0, rgb: swatch, layer: 0 }]).collect();
    let set = set_of(4, &columns);
    for (textured, want) in [(true, tile), (false, swatch)] {
        let baked = bake_in(&set, 2, 1, textured);
        let word = baked.boxes().next().expect("a coarse box")[1];
        let shown = [(word & 255) as u8, ((word >> 8) & 255) as u8, ((word >> 16) & 255) as u8];
        assert_eq!(shown, srgb_of(linear_of(want[0], want[1], want[2])), "textured {textured}");
    }
}

#[test]
fn a_cells_floor_is_its_bottom_face_and_the_box_stands_clear_of_it() {
    // Ground one cell thick under a coarse cell four cells on a side: what
    // spans the cell rises no higher than the cell's bottom — the skirt and
    // the face on it — and the inset box stands over it with nothing between.
    let columns: Vec<Vec<Run>> = (0..36).map(|_| ground(0)).collect();
    let set = set_of(6, &columns);
    let baked = bake(&set, 4, 1);
    let floors: Vec<_> = baked.boxes().filter(|b| baked.at(b).1[0] >= 4.0).collect();
    assert!(!floors.is_empty(), "the cell lays a floor");
    for b in floors {
        let (min, size) = baked.at(b);
        assert!(
            min[2] + size[2] <= baked.heights.step,
            "the floor reaches no higher than the cell's bottom, a height step aside: {min:?} {size:?}"
        );
    }
    let boxes: Vec<_> = baked.boxes().filter(|b| baked.at(b).1[0] < 4.0).collect();
    assert!(boxes.iter().all(|b| baked.at(b).0[2] > 0.0), "the box stands clear of the floor");
}

#[test]
fn a_cell_of_placed_things_alone_lays_no_floor_and_floats() {
    // Ground at the bottom, and a placed cube eight cells up in one column:
    // its bucket holds no ground, so its box floats at the cell's centre with
    // nothing under it, and is lit underneath as the placed thing it is.
    let columns: Vec<Vec<Run>> = (0..36)
        .map(|k| {
            let mut column = ground(0);
            if k == 14 {
                column.push(Run {
                    z0: 9,
                    z1: 9,
                    rgb: [200, 0, 0],
                    layer: 0,
                });
            }
            column
        })
        .collect();
    let mut set = set_of(6, &columns);
    for i in 0..set.z0.len() {
        if set.z0[i] == 9 {
            set.material_class[i] = MATERIAL_OBJECT;
        }
    }
    let baked = bake(&set, 4, 1);
    let up_there: Vec<_> = baked.boxes().filter(|b| baked.at(b).0[2] + baked.at(b).1[2] > 4.0).collect();
    assert!(!up_there.is_empty(), "the placed cube's box is drawn");
    for b in up_there {
        let size = baked.at(b).1;
        assert!(size[0] < 4.0, "no floor spans its cell: {size:?}");
        assert!(b[0] & FACE_NZ != 0 && b[0] & UNDER_AS_TOP == 0, "its underside is drawn as its own");
    }
}
