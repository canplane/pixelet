// pxl-render/mesh/chunk_mesher.test.rs

use super::*;

/// One run as a column holds it: its heights, its color and its layer.
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
    // Three columns a side, the middle one as tall as its neighbors.
    let columns: Vec<Vec<Run>> = (0..9).map(|_| ground(3)).collect();
    let set = set_of(3, &columns);
    let view = set.view();
    let faces = exposed_faces(&view, 4, 0, 0, 3);
    assert_eq!(faces, 2.0, "a buried run keeps its top and its bottom alone");
}

#[test]
fn a_wall_shows_the_side_it_stands_above() {
    // The middle column stands four cells over its neighbors' one.
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

/// Bakes `set` at `lod_step`, the mesh handed a block of `block` boxes at a
/// time, and the words it wrote, in order.
fn bake_in_blocks(set: &OwnedRunSet, lod_step: u32, block: u32) -> Vec<u32> {
    let mut bake = BakeContext::new();
    let view = set.view();
    assert!(bake.begin(&view, lod_step, 1, true, None));
    let words_a_box = box_words(lod_step);
    let mut buf = vec![0u32; (block * words_a_box) as usize];
    let mut out: Vec<u32> = Vec::new();
    let mut handed = 0usize;
    loop {
        match bake.step(&view) {
            Step::Working => {}
            Step::NeedMesh(words) => {
                let n = words.min(block * words_a_box);
                // SAFETY: `buf`, untouched until the block is full or the bake
                // is done.
                unsafe { bake.provide_mesh(buf.as_mut_ptr(), n) };
                handed = n as usize;
            }
            Step::Full => {
                out.extend_from_slice(&buf[..handed]);
                let n = bake.left_words().min(block * words_a_box);
                // SAFETY: as above.
                unsafe { bake.provide_mesh(buf.as_mut_ptr(), n) };
                handed = n as usize;
            }
            Step::Done => {
                let mesh = bake.finish();
                let written = mesh.box_cnt as usize * words_a_box as usize - out.len();
                out.extend_from_slice(&buf[..written.min(handed)]);
                assert_eq!(out.len(), mesh.box_cnt as usize * words_a_box as usize);
                return out;
            }
        }
    }
}

/// A mesh handed a block at a time is the mesh handed whole: the same boxes
/// in the same order, whatever the block — one box, a few, or all of it —
/// at a leaf's own cells and coarser.
#[test]
fn a_mesh_written_through_blocks_is_the_mesh_written_whole() {
    // Sixteen columns and the ring about them, which every step divides.
    let span = 18;
    let columns: Vec<Vec<Run>> = (0..span * span).map(|k| ground(1 + ((k * 7919) % 13) as i32)).collect();
    let set = set_of(span, &columns);
    for lod_step in [1, 2, 4] {
        let whole = bake_in_blocks(&set, lod_step, u32::MAX / 64);
        assert!(!whole.is_empty(), "nothing baked at step {lod_step}");
        for block in [1, 3, 17, 64] {
            assert_eq!(bake_in_blocks(&set, lod_step, block), whole, "step {lod_step}, blocks of {block}");
        }
    }
}
