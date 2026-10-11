// pxl-render/mesh/chunk.test.rs

use super::*;

struct Run {
    z0: i32,
    z1: i32,
    rgb: [u8; 3],
    layer: u32,
}

/// Every run is whole-cell terrain.
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
            set.side.push(Side::BASE);
            set.sub_s.push(SUB_CELL_NONE);
            for lane in [&mut set.sub_ox, &mut set.sub_oy, &mut set.sub_oz] {
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

fn bake_in_blocks(set: &OwnedRunSet, lod_step: u32, block: u32) -> Vec<u32> {
    let mut bake = BakeContext::default();
    let view = set.view();
    assert!(bake.begin(&view, lod_step, 1.0, 1.0, lod_step == 1, true, (8, 512), [i32::MIN, i32::MAX]).is_ok());
    let words_a_box = box_words(lod_step == 1);
    let mut buf = vec![0u32; (block * words_a_box) as usize];
    let mut out: Vec<u32> = Vec::new();
    let mut handed = 0usize;
    loop {
        match bake.step(&view) {
            Step::Working => {}
            Step::NeedMesh(words) => {
                let n = words.min(block * words_a_box);
                // SAFETY: `buf`, untouched until the block is full or the bake is done.
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
            Step::Refused => panic!("the heap refused a test bake"),
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

/// A mesh handed a block at a time is the mesh handed whole, whatever the block size.
#[test]
fn a_mesh_written_through_blocks_is_the_mesh_written_whole() {
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

/// A fold's fixed point: a cell full of one leaf folds to that leaf, its
/// swatch and its texture, not a summary of it.
#[test]
fn a_cell_full_of_one_leaf_folds_to_that_leaf() {
    let (span, taps) = (10, taps_of(5));
    let columns: Vec<Vec<Run>> = (0..span * span)
        .map(|_| {
            vec![Run {
                z0: 0,
                z1: 7,
                rgb: [10, 20, 30],
                layer: taps,
            }]
        })
        .collect();
    let set = set_of(span, &columns);
    let mut bake = BakeContext::default();
    let view = set.view();
    assert!(bake.begin(&view, 2, 1.0, 1.0, true, true, (8, 512), [i32::MIN, i32::MAX]).is_ok());
    let mut buf: Vec<u32> = Vec::new();
    loop {
        match bake.step(&view) {
            Step::Working => {}
            Step::NeedMesh(n) => {
                buf = vec![0; n as usize];
                // SAFETY: `buf`, untouched until the bake is done.
                unsafe { bake.provide_mesh(buf.as_mut_ptr(), n) };
            }
            Step::Full => panic!("one block holds the mesh"),
            Step::Refused => panic!("the heap refused a test bake"),
            Step::Done => break,
        }
    }
    let mesh = bake.finish();
    let boxes = &buf[..(mesh.box_cnt * FINE_BOX_WORDS) as usize];
    assert!(!boxes.is_empty());
    for m in boxes.chunks(FINE_BOX_WORDS as usize) {
        assert_eq!((m[3].to_le_bytes(), m[4]), ([10, 20, 30, 0], taps));
    }
}

fn bake_whole(set: &OwnedRunSet, z_unit: f64) -> (Vec<u32>, Heights) {
    bake_window(set, z_unit, [i32::MIN, i32::MAX])
}

fn bake_window(set: &OwnedRunSet, z_unit: f64, window: [i32; 2]) -> (Vec<u32>, Heights) {
    let mut bake = BakeContext::default();
    let view = set.view();
    assert!(bake.begin(&view, 1, 1.0, z_unit, true, true, (8, 512), window).is_ok());
    let mut buf: Vec<u32> = Vec::new();
    loop {
        match bake.step(&view) {
            Step::Working => {}
            Step::NeedMesh(n) => {
                buf = vec![0; n as usize];
                // SAFETY: `buf`, untouched until the bake is done.
                unsafe { bake.provide_mesh(buf.as_mut_ptr(), n) };
            }
            Step::Full => panic!("one block holds the mesh"),
            Step::Refused => panic!("the heap refused a test bake"),
            Step::Done => break,
        }
    }
    let mesh = bake.finish();
    buf.truncate((mesh.box_cnt * FINE_BOX_WORDS) as usize);
    (buf, mesh.heights)
}

/// A column laid in strata, a color a column so no box spans two: stone,
/// soil and its look on top, the face `top` high.
fn strata_field(span: u32, top: impl Fn(u32) -> i32) -> OwnedRunSet {
    let none = taps_of(NO_LAYER as u8);
    let columns: Vec<Vec<Run>> = (0..span * span)
        .map(|k| {
            let (t, c) = (top(k), k as u8);
            vec![
                Run {
                    z0: 0,
                    z1: t - 4,
                    rgb: [c, 1, 0],
                    layer: none,
                },
                Run {
                    z0: t - 3,
                    z1: t - 1,
                    rgb: [c, 2, 0],
                    layer: none,
                },
                Run {
                    z0: t,
                    z1: t,
                    rgb: [c, 3, 0],
                    layer: none,
                },
            ]
        })
        .collect();
    set_of(span, &columns)
}

/// A side buried in the ground is no face, however the column beside is laid:
/// strata meet at heights of each column's own, so no one run of it covers.
#[test]
fn a_side_buried_in_strata_is_no_face() {
    let span = 12;
    let set = strata_field(span, |k| 8 + ((k * 7919) % 3) as i32);
    let (mesh, heights) = bake_whole(&set, 1.0);
    let inner = (span - 2) as f64;
    // Every column stands 8 high at least: under that, only the rim shows.
    for m in mesh.chunks(FINE_BOX_WORDS as usize) {
        let (min, size) = box_bounds(m, 1.0, heights);
        let rim = min[0] < 1e-3 || min[1] < 1e-3 || min[0] + size[0] > inner - 1e-3 || min[1] + size[1] > inner - 1e-3;
        let sides = m[5] & (FACE_PX | FACE_NX | FACE_PY | FACE_NY);
        assert!(rim || sides == 0 || min[2] + size[2] > 8.0 - 1e-3, "a buried side drawn: {min:?} {size:?} {sides:#x}");
    }
}

/// Boxes stacked in a column meet at one height step, neither over nor apart,
/// however their heights round.
#[test]
fn stacked_boxes_meet() {
    let set = strata_field(10, |k| 40 + ((k * 7919) % 7) as i32);
    let (mesh, _) = bake_whole(&set, 0.37);
    let mut stacks: std::collections::BTreeMap<(u32, u32), Vec<(u32, u32)>> = Default::default();
    for m in mesh.chunks(FINE_BOX_WORDS as usize) {
        stacks.entry((m[0], m[1])).or_default().push((m[2] & 0xffff, m[2] >> 16));
    }
    for (at, mut stack) in stacks {
        stack.sort();
        // A buried box is culled, leaving a gap of whole cells.
        for pair in stack.windows(2) {
            let (end, next) = (pair[0].0 + pair[0].1, pair[1].0);
            assert!(next == end || next > end + 1, "boxes at {at:?} meet off a step: {stack:?}");
        }
    }
}

/// A chunk draws its own heights, the faces at its floor and roof as the
/// cells past them say: hidden where ground goes on, drawn where it stops.
#[test]
fn a_chunk_floor_and_roof_show_where_the_ground_past_them_stops() {
    let bake = |z0: i32, z1: i32| {
        let columns: Vec<Vec<Run>> = (0..9).map(|k| if k == 4 { vec![Run { z0, z1, rgb: [1, 2, 3], layer: 0 }] } else { Vec::new() }).collect();
        let (mesh, heights) = bake_window(&set_of(3, &columns), 1.0, [0, 4]);
        let m = mesh.chunks(FINE_BOX_WORDS as usize).next().map(|m| m.to_vec()).expect("a box");
        let (min, size) = box_bounds(&m, 1.0, heights);
        ((min[2], min[2] + size[2]), m[5] & FACE_NZ != 0, m[5] & FACE_PZ != 0)
    };
    assert_eq!(bake(-1, 4), ((0.0, 4.0), false, false), "ground through both");
    assert_eq!(bake(1, 4), ((1.0, 4.0), true, false), "a cave's roof");
    assert_eq!(bake(-1, 2), ((0.0, 3.0), false, true), "the ground's face");
}
