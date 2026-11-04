//! `fold_cs` by hand (frame/shader/fold.wgsl) folds the eye's clip levels over their bricks
//! as pxl-engine's `clipmap::fold` does, a level a dispatch as the renderer runs it
//! (gpu/ray/cast.rs `fold`): over the real world of tests/real_world on both sides of the
//! seam, and over its table as the kernel leaves it between folds, a clip level moved a brick
//! before its bricks are held, slots let go, a clip level laid out or not. Every word of
//! the table compared after each fold. Skipped where no adapter answers.

mod gpu;
mod real_world;

use pxl_engine::clipmap::{
    self as clip, ACROSS, ENTRIES, EYE_CLIP_LEVELS, FIRST_CELL, JOINED_AT, JOINED_ENTRIES,
    JOINED_LEVELS, ORIGIN_AT,
};
use pxl_engine::{wrap_idx, LEAF_CELL};
use pxl_render::frame::shader::record::{src, SHADER_FOLD};
use pxl_render::frame::shader::word::FOLD_LANES;

use gpu::{Gpu, Tally};
use real_world::{mix, Built, World};

/// East of X = 0 by the seam on the far side, a few cells over the service area's south
/// edge; real_world's own eye is west of it by the seam.
const EYE_EAST: [i32; 2] = [(1 << 23) - 300, (40 << 16) + 30];

/// The table folded as `clipmap::fold` folds it, a level at a time, every clip level's
/// entries of a level read off the levels under it.
fn fold_rust(b: &Built, table: &mut [[i32; 4]]) {
    for l in 1..=JOINED_LEVELS {
        for cell in FIRST_CELL..=LEAF_CELL {
            let entries: Vec<_> = {
                let w = World {
                    clipmap: table,
                    ground: &b.ground,
                    palette: &b.palette,
                    shards: &b.shards,
                    overwrites: &b.overwrites,
                    pool: &b.pool,
                };
                (0..(ACROSS >> l) * (ACROSS >> l))
                    .map(|e| clip::fold(&w, cell, l, e))
                    .collect()
            };
            let at = JOINED_AT + (cell - FIRST_CELL) * JOINED_ENTRIES + clip::joined_idx(l, [0, 0]);
            for (e, words) in entries.into_iter().enumerate() {
                table[at as usize + e] = words.map(|v| v as i32);
            }
        }
    }
}

/// The table folded by hand, as the renderer dispatches it.
fn fold_hand(
    gpu: &Gpu,
    p: &wgpu::ComputePipeline,
    ground: &wgpu::Buffer,
    table: &[[i32; 4]],
) -> Vec<u32> {
    let flat: Vec<u32> = table.iter().flatten().map(|&v| v as u32).collect();
    let buf = gpu.buffer(&flat, wgpu::BufferUsages::STORAGE);
    for l in 1..=JOINED_LEVELS {
        let blocks = (ACROSS >> l) * (ACROSS >> l);
        gpu.dispatch(
            p,
            &[
                (1, buf.as_entire_binding()),
                (2, ground.as_entire_binding()),
            ],
            [(blocks * EYE_CLIP_LEVELS).div_ceil(FOLD_LANES), 1, l],
        );
    }
    gpu.read(&buf)
}

/// Row `k` of the table, word `w`, by what it holds.
fn row_name(k: usize) -> String {
    let (row, word) = ((k / 4) as u32, k % 4);
    if row >= JOINED_AT {
        let e = row - JOINED_AT;
        format!(
            "zoom {} entry {} word {word}",
            FIRST_CELL + e / JOINED_ENTRIES,
            e % JOINED_ENTRIES
        )
    } else if row >= ORIGIN_AT {
        format!("origin {} word {word}", row - ORIGIN_AT)
    } else {
        format!("row {row} word {word}")
    }
}

/// The table's rows of the clip level at `cell`'s entries.
fn entry_rows(cell: u32) -> std::ops::Range<usize> {
    let first = ((cell - FIRST_CELL) * ENTRIES) as usize;
    first..first + ENTRIES as usize
}

/// The ways the kernel leaves the table between folds, each a case over `b`'s.
fn cases(b: &Built) -> Vec<(&'static str, Vec<[i32; 4]>)> {
    let built = b.clipmap.clone();
    let mut out = vec![("as built", built.clone())];
    // Every joined row other than what a fold writes, so none is read before it is written.
    let mut t = built.clone();
    for (k, row) in t[JOINED_AT as usize..].iter_mut().enumerate() {
        *row = [mix(k as u32, [1, 2]) as i32, -(k as i32), 7, -9];
    }
    out.push(("joined rows stale", t));
    // A clip level moved a brick east, or north and west, its entries still naming the
    // bricks it held before.
    for (name, d) in [
        ("moved a brick east", [1, 0]),
        ("moved north and west", [-1, 1]),
    ] {
        let mut t = built.clone();
        for cell in FIRST_CELL..=LEAF_CELL {
            if !mix(cell, d).is_multiple_of(3) {
                let o = &mut t[(ORIGIN_AT + cell - FIRST_CELL) as usize];
                o[0] = wrap_idx(clip::bricks_zoom(cell), o[0] + d[0]);
                o[1] += d[1];
            }
        }
        out.push((name, t));
    }
    // Slots let go: some entries no longer present, so a brick is read off a coarser clip
    // level, or off none.
    let mut t = built.clone();
    for (k, row) in t[..(EYE_CLIP_LEVELS * ENTRIES) as usize]
        .iter_mut()
        .enumerate()
    {
        if mix(k as u32, [5, 3]).is_multiple_of(3) {
            row[2] = 0;
        }
    }
    out.push(("slots let go", t));
    // A zoom with no brick present, so every block over it reads the next coarser's.
    let mut t = built.clone();
    for cell in [FIRST_CELL + 3, LEAF_CELL - 1] {
        for row in &mut t[entry_rows(cell)] {
            row[2] = 0;
        }
    }
    out.push(("zooms with nothing present", t));
    // The coarsest zoom with nothing present: what no clip level holds bounds nothing.
    let mut t = built.clone();
    for row in &mut t[entry_rows(FIRST_CELL)] {
        row[2] = 0;
    }
    out.push(("the coarsest with nothing present", t));
    // Clip levels laid out that were not, and not that were.
    let mut t = built.clone();
    for cell in FIRST_CELL..=LEAF_CELL {
        let o = &mut t[(ORIGIN_AT + cell - FIRST_CELL) as usize];
        o[2] = i32::from(o[2] == 0);
    }
    out.push(("laid out the other way", t));
    out
}

#[test]
fn hand_fold_folds_as_clipmap_fold() {
    let Some(gpu) = Gpu::new() else {
        eprintln!("no adapter: skipped");
        return;
    };
    let p = gpu.pipeline(src(SHADER_FOLD), "fold_cs");
    let mut tally = Tally::default();
    for (side, eye) in [
        ("west by the seam", real_world::EYE),
        ("east by the seam", EYE_EAST),
    ] {
        let b = real_world::build(eye);
        let ground = gpu.buffer(&b.ground, wgpu::BufferUsages::STORAGE);
        let mut zero_entries = 0;
        for (name, table) in cases(&b) {
            // The renderer's table: its rows to the last fold's (gpu/ray/cast.rs `make`).
            assert_eq!(
                table.len(),
                (JOINED_AT + EYE_CLIP_LEVELS * JOINED_ENTRIES) as usize
            );
            let mut want = table.clone();
            fold_rust(&b, &mut want);
            zero_entries += want[JOINED_AT as usize..]
                .iter()
                .filter(|r| **r == [0; 4])
                .count();
            let want: Vec<u32> = want.iter().flatten().map(|&v| v as u32).collect();
            let have = fold_hand(&gpu, &p, &ground, &table);
            tally.compare(&format!("{side}, {name}"), &want, &have, row_name);
        }
        eprintln!("{side}: {zero_entries} entries none");
        assert!(zero_entries > 0, "{side}: some entry is none");
    }
    tally.end("fold");
}
