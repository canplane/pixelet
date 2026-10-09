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
    assert!(bake.begin(&view, lod_step, 1.0, 1.0, lod_step == 1, true, (8, 512), None).is_ok());
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

fn east_faces(set: &OwnedRunSet, lod_step: u32) -> Vec<(f64, f64)> {
    let mut bake = BakeContext::default();
    let view = set.view();
    assert!(bake.begin(&view, lod_step, 1.0, 1.0, lod_step == 1, true, (8, 512), None).is_ok());
    let words = box_words(lod_step == 1);
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
    let unit = lod_step as f64;
    let mut out: Vec<(f64, f64)> = buf[..(mesh.box_cnt * words) as usize]
        .chunks(words as usize)
        .filter(|m| (if words == COARSE_BOX_WORDS { m[0] } else { m[5] }) & FACE_PX != 0)
        .map(|m| {
            let (min, size) = box_bounds(m, unit, mesh.heights);
            (min[2], min[2] + size[2])
        })
        .collect();
    out.sort_by(|a, b| a.0.total_cmp(&b.0));
    out
}

fn covers(spans: &[(f64, f64)], lo: f64, hi: f64) -> bool {
    let mut reached = lo;
    for &(a, b) in spans {
        if a > reached + 1e-6 {
            break;
        }
        reached = reached.max(b);
    }
    reached + 1e-6 >= hi
}

/// Where the chunk beside stands lower than this one's floor, the edge wall
/// reaches down to it, so no open interval is left between them.
#[test]
fn an_edge_wall_reaches_down_to_the_ground_beside_it() {
    for (span, lod_step) in [(3u32, 1u32), (4, 2)] {
        let east = |k: u32| k % span == span - 1 && k / span > 0 && k / span < span - 1;
        let columns: Vec<Vec<Run>> = (0..span * span)
            .map(|k| {
                let z0 = if east(k) { 0 } else { 16 };
                let z1 = if east(k) { 0 } else { 19 };
                vec![Run { z0, z1, rgb: [0, 0, 0], layer: 0 }]
            })
            .collect();
        let set = set_of(span, &columns);
        let faces = east_faces(&set, lod_step);
        // To within a step of the heights' encoding.
        assert!(covers(&faces, 1.0 + 1e-3, 20.0), "step {lod_step}: east faces {faces:?} leave the drop to the ground beside open");
    }
}

/// A fold's fixed point: a cell full of one leaf folds to that leaf, its
/// swatch and its texture, not a summary of it.
#[test]
fn a_cell_full_of_one_leaf_folds_to_that_leaf() {
    let (span, taps) = (10, taps_of(5));
    let columns: Vec<Vec<Run>> = (0..span * span).map(|_| vec![Run { z0: 0, z1: 7, rgb: [10, 20, 30], layer: taps }]).collect();
    let set = set_of(span, &columns);
    let mut bake = BakeContext::default();
    let view = set.view();
    assert!(bake.begin(&view, 2, 1.0, 1.0, true, true, (8, 512), None).is_ok());
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
