//! A handed brick's fill by hand answers as pxl-engine's: `height_cs`
//! (frame/shader/height.wgsl) as `fill::heights` and `fill::put_palette_word`, `bound_cs`
//! (frame/shader/bound.wgsl) as `fill::level`, a level a dispatch, over the spans
//! `fill::spans` keeps. The records are the real world's of tests/real_world, a frame's
//! handed bricks at once as the renderer hands them (gpu/ray/ground.rs `grids`): every
//! zoom of the eye's clip levels, bricks at their tile's edges, west and east of the seam,
//! on the service area's edge, with ways and without, palette indices that tie, steep
//! ground. Every word of the bricks' lanes and of the palette compared. Skipped where no
//! adapter answers.

mod gpu;
mod real_world;

use pxl_engine::area::Shard;
use pxl_engine::clipmap::{
    self as clip, BRICK, BRICK_CELLS, BRICK_LEVELS, BRICK_SHIFT, BRICK_WORDS, ELE_AT, FILL_CAP,
    FIRST_CELL, HANDED_WORDS, PALETTE_AT, SPANS_COLUMNS, TREE_AT,
};
use pxl_engine::lattice::CELL_CENTER;
use pxl_engine::{fill, ground, wrap_idx, Zoom, LEAF_CELL};
use pxl_render::frame::shader::record::{src, SHADER_BOUND, SHADER_HEIGHT};
use pxl_render::frame::shader::word::{HEIGHT_SIDE, LEVEL_SIDE};

use gpu::{Gpu, Tally};
use real_world::{mix, Built};

/// East of X = 0 by the seam on the far side; real_world's own eye is west of it.
const EYE_EAST: [i32; 2] = [(1 << 23) - 300, (40 << 16) + 30];

/// A brick a frame hands: its cells' zoom, which brick, whether it has ways, the slot it
/// fills, and how its palette indices and samples are made over.
#[derive(Clone, Copy)]
struct Hand {
    cell: Zoom,
    q: [i32; 2],
    lined: bool,
    slot: u32,
    ties: bool,
    steep: bool,
}

/// The brick holding the eye's leaf cell `eye` at `cell`, moved `d` bricks.
fn brick_about(eye: [i32; 2], cell: Zoom, d: [i32; 2]) -> [i32; 2] {
    let s = LEAF_CELL - cell + BRICK_SHIFT;
    [
        wrap_idx(clip::bricks_zoom(cell), (eye[0] >> s) + d[0]),
        (eye[1] >> s) + d[1],
    ]
}

/// The frames of bricks handed about `eye`, `FILL_CAP` a frame at most.
fn frames(eye: [i32; 2]) -> Vec<Vec<Hand>> {
    let mut hands = Vec::new();
    let mut slot = 0;
    for cell in FIRST_CELL..=LEAF_CELL {
        for (k, d) in [[0, 0], [-1, 0], [1, 1], [0, -1]].into_iter().enumerate() {
            let h = mix(cell * 7 + k as u32, eye);
            hands.push(Hand {
                cell,
                q: brick_about(eye, cell, d),
                lined: k != 3,
                slot,
                ties: h.is_multiple_of(3),
                steep: h % 5 == 1,
            });
            // Slots apart and in no order, a frame's spread over the bricks' buffer.
            slot += 1 + h % 3;
        }
    }
    // The bricks of a tile, its first and last at its edges, at the leaves, where the
    // service area's south edge cuts them.
    for x in 0..8 {
        let q = brick_about(eye, LEAF_CELL, [0, 0]);
        hands.push(Hand {
            cell: LEAF_CELL,
            q: [(q[0] & !7) + x, q[1] - 1 + x % 2],
            lined: true,
            slot,
            ties: x % 2 == 0,
            steep: x % 3 == 0,
        });
        slot += 1;
    }
    // A frame of one, and the rest a frame of `FILL_CAP` each.
    let mut out = vec![hands[..1].to_vec()];
    out.extend(hands[1..].chunks(FILL_CAP as usize).map(|c| c.to_vec()));
    out
}

/// The palette indices a cell is made over with where its brick's tie: halves, quarters,
/// three and one, and none known, a cell in eight.
fn tied(i: u32, word: u32) -> u32 {
    let [a, b, c, _] = ground::palette_indices(word);
    let (b, c) = (
        if b == a { a + 1 } else { b },
        if c == a { a + 2 } else { c },
    );
    let four = match mix(i, [3, 4]) % 8 {
        0 => [a, a, b, b],
        1 => [a, b, b, a],
        2 => [a, b, c, a ^ 1],
        3 => [b, a, a, a],
        4 => [ground::UNKNOWN; 4],
        5 => [a, b, a, b],
        _ => return word,
    };
    ground::word(four)
}

/// A frame's fresh list, its records as `real_world::handed` lays them, their words as the
/// kernel writes them.
fn fresh(b: &Built, eye: [i32; 2], frame: &[Hand]) -> Vec<u32> {
    let mut fresh = vec![0u32; (FILL_CAP * HANDED_WORDS) as usize];
    let mut ways = Vec::new();
    for (k, h) in frame.iter().enumerate() {
        let rec = &mut fresh[k * HANDED_WORDS as usize..(k + 1) * HANDED_WORDS as usize];
        real_world::handed(eye, rec, h.cell, h.q, h.lined, &mut ways);
        let first = [
            wrap_idx(h.cell, h.q[0] << BRICK_SHIFT),
            h.q[1] << BRICK_SHIFT,
        ];
        let place = [(h.q[0] & 7) as u32, (h.q[1] & 7) as u32];
        let shard = Shard::UNKNOWN.at(&b.world(), (clip::bricks_zoom(h.cell), h.q));
        rec[..clip::FILL_WORDS as usize].copy_from_slice(&clip::fill_words(
            h.slot,
            clip::place_word(place),
            h.cell,
            first,
            shard,
        ));
        if h.ties {
            for i in 0..BRICK_CELLS {
                let w = &mut rec[(PALETTE_AT + i) as usize];
                *w = tied(i ^ h.slot, *w);
            }
        }
        if h.steep {
            // Mountains and the deep sea, as far as the Earth's go in leaf cells.
            for w in &mut rec[ELE_AT as usize..clip::LINES_AT as usize] {
                let half = |v: u32| {
                    let s = (v as u16 as i16 as i32 * 5).clamp(-5000, 4000);
                    s as i16 as u16 as u32
                };
                *w = half(*w) | half(*w >> 16) << 16;
            }
        }
    }
    fresh
}

/// The ulps of its sum from a half within which a top may round to either whole cell.
const EDGE_ULPS: f32 = 4.0;

/// Whether column `i` of `h`'s brick, handed in the record at `rec`, draws its top off a
/// sum within `EDGE_ULPS` of a half (`ground::bicubic`, its sum taken in f64 here): the
/// Rust rounds each product and each sum in f32, a GPU that fuses a product into its sum
/// (Metal does) rounds once, and the two land on either side of the half.
fn on_edge(fresh: &[u32], rec: usize, h: &Hand, i: u32) -> bool {
    let ele =
        |s: usize| ((fresh[rec + ELE_AT as usize + s / 2] >> (16 * (s % 2))) << 16) as i32 >> 16;
    let [x, y] = clip::of_idx(i);
    let place = [(h.q[0] & 7) as u32, (h.q[1] & 7) as u32];
    let c = [place[0] * BRICK + x, place[1] * BRICK + y].map(|v| v as f32 + CELL_CENTER);
    let ((x0, fx), (y0, fy)) = (ground::window(c[0]), ground::window(c[1]));
    let (wx, wy) = (ground::catmull_rom(fx), ground::catmull_rom(fy));
    let (mut sum, mut lo, mut hi) = (0f64, i32::MAX, i32::MIN);
    for j in 0..4 {
        let mut row = 0f64;
        for k in 0..4 {
            let v = ele(ground::ele_idx(
                ground::clamp_idx(x0 + k),
                ground::clamp_idx(y0 + j),
            ));
            (lo, hi) = (lo.min(v), hi.max(v));
            row += f64::from(v) * f64::from(wx[k as usize]);
        }
        sum += row * f64::from(wy[j as usize]);
    }
    let held = sum.clamp(f64::from(lo), f64::from(hi));
    let ulp = {
        let f = held.abs() as f32;
        f64::from(f32::from_bits(f.to_bits() + 1) - f)
    };
    (held - (held.floor() + 0.5)).abs() <= f64::from(EDGE_ULPS) * ulp
}

/// Word `k` of the bricks' lanes, by the slot and what it holds.
fn lane_name(k: usize) -> String {
    let (slot, w) = (k as u32 / BRICK_WORDS, k as u32 % BRICK_WORDS);
    let what = if w < clip::BOUNDS_AT {
        format!("column {}", w - clip::COLUMNS_AT)
    } else if w < clip::LAID_AT {
        format!("level word {}", w - clip::BOUNDS_AT)
    } else if w < clip::ROOT_AT {
        format!("laid word {}", w - clip::LAID_AT)
    } else {
        "root".to_string()
    };
    format!("slot {slot} {what}")
}

/// The frames' bricks' lanes before their fill: words no fill writes, so one it writes
/// where it should not shows.
fn lanes(frames: &[Vec<Hand>]) -> Vec<u32> {
    let slots = frames
        .iter()
        .flatten()
        .map(|h| h.slot + 1)
        .max()
        .unwrap_or(0);
    (0..slots * BRICK_WORDS)
        .map(|k| mix(k, [9, 9]) | 0x8000_0000)
        .collect()
}

#[test]
fn hand_fill_answers_as_fill() {
    let Some(gpu) = Gpu::new() else {
        eprintln!("no adapter: skipped");
        return;
    };
    let height = gpu.pipeline(src(SHADER_HEIGHT), "height_cs");
    let bound = gpu.pipeline(src(SHADER_BOUND), "bound_cs");
    let store = wgpu::BufferUsages::STORAGE;
    let (mut tops, mut levels) = (Tally::default(), Tally::default());
    let (mut unknown, mut walled, mut lined_columns, mut edges, mut edges_apart) =
        (0u32, 0u32, 0u32, 0u32, 0u32);
    // A brick's scratch is a `FILL_CAP`th of the frame's, past its spans by some.
    let scratch_len = FILL_CAP * (TREE_AT + 37);
    for (side, eye) in [
        ("west by the seam", real_world::EYE),
        ("east by the seam", EYE_EAST),
    ] {
        let b = real_world::build(eye);
        let frames = frames(eye);
        let mut lane = lanes(&frames);
        let palette_len = lane.len() as u32 / BRICK_WORDS * BRICK_CELLS;
        let mut palette: Vec<u32> = (0..palette_len).map(|k| mix(k, [2, 7])).collect();
        for (f, frame) in frames.iter().enumerate() {
            let n = frame.len() as u32;
            let fresh = fresh(&b, eye, frame);
            let case = format!("{side}, frame {f} of {n}");

            // The tops, the laid cells and the palette indices.
            let (lane_buf, palette_buf, fresh_buf) = (
                gpu.buffer(&lane, store),
                gpu.buffer(&palette, store),
                gpu.buffer(&fresh, store),
            );
            let mut edge = vec![false; lane.len()];
            for k in 0..n {
                for y in 0..BRICK {
                    for x in 0..BRICK {
                        fill::heights(&mut lane, &fresh, k, [x, y]);
                        fill::put_palette_word(&mut palette, &fresh, k, [x, y]);
                    }
                }
                let rec = (k * HANDED_WORDS) as usize;
                let h = &frame[k as usize];
                for i in 0..BRICK_CELLS {
                    if on_edge(&fresh, rec, h, i) {
                        edge[(h.slot * BRICK_WORDS + clip::COLUMNS_AT + i) as usize] = true;
                        edges += 1;
                    }
                }
                unknown += (0..BRICK_CELLS)
                    .filter(|&i| {
                        let cell = fresh[rec + clip::FILL_CELL as usize];
                        ground::own(fresh[rec + (PALETTE_AT + i) as usize], cell) == ground::UNKNOWN
                    })
                    .count() as u32;
            }
            gpu.dispatch(
                &height,
                &[
                    (0, fresh_buf.as_entire_binding()),
                    (1, lane_buf.as_entire_binding()),
                    (2, palette_buf.as_entire_binding()),
                ],
                [BRICK.div_ceil(HEIGHT_SIDE), BRICK.div_ceil(HEIGHT_SIDE), n],
            );
            // A top on the edge rounds to either whole cell: within one, as an i16.
            let got = gpu.read(&lane_buf);
            edges_apart += (0..lane.len())
                .filter(|&k| edge[k] && lane[k] != got[k])
                .count() as u32;
            tops.compare_by(
                &format!("{case}: tops"),
                &lane,
                &got,
                lane_name,
                |k, w, h| w == h || edge[k] && (w as i16).abs_diff(h as i16) == 1,
            );
            tops.compare(
                &format!("{case}: palette"),
                &palette,
                &gpu.read(&palette_buf),
                |k| {
                    format!(
                        "slot {} cell {}",
                        k as u32 / BRICK_CELLS,
                        k as u32 % BRICK_CELLS
                    )
                },
            );

            // The spans as the ways' pass keeps them, then the levels, a dispatch each.
            let mut scratch: Vec<u32> = (0..scratch_len).map(|k| mix(k, [4, 4])).collect();
            for k in 0..n {
                for i in 0..SPANS_COLUMNS {
                    fill::spans(&lane, &fresh, &mut scratch, k, i);
                }
                let at = fill::scratch_at(&scratch, k);
                for i in 0..SPANS_COLUMNS {
                    let head = scratch[(at + i * clip::SPANS_COLUMN_WORDS) as usize];
                    walled += u32::from(head >> 16 == 0x8000);
                    lined_columns += u32::from(head & fill::SPANS_CNT != 0);
                }
            }
            let (lane_buf, scratch_buf) = (gpu.buffer(&lane, store), gpu.buffer(&scratch, store));
            for j in 0..=BRICK_LEVELS {
                let side_n = BRICK >> j;
                for k in 0..n {
                    let h = &frame[k as usize];
                    let spans = fill::Scratch {
                        words: &scratch,
                        at: fill::scratch_at(&scratch, k),
                    };
                    for e in 0..side_n * side_n {
                        fill::level(
                            &mut lane,
                            h.slot * BRICK_WORDS,
                            &spans,
                            h.cell,
                            j,
                            [e % side_n, e / side_n],
                        );
                    }
                }
                let level_buf = gpu.buffer(&[j, 0, 0, 0], wgpu::BufferUsages::UNIFORM);
                gpu.dispatch(
                    &bound,
                    &[
                        (0, level_buf.as_entire_binding()),
                        (1, fresh_buf.as_entire_binding()),
                        (2, lane_buf.as_entire_binding()),
                        (3, scratch_buf.as_entire_binding()),
                    ],
                    [side_n.div_ceil(LEVEL_SIDE), side_n.div_ceil(LEVEL_SIDE), n],
                );
                levels.compare(
                    &format!("{case}: level {j}"),
                    &lane,
                    &gpu.read(&lane_buf),
                    lane_name,
                );
            }
        }
    }
    eprintln!("{unknown} cells of no known look, {walled} columns of no face, {lined_columns} columns with spans, {edges} tops within {EDGE_ULPS} ulps of a half, {edges_apart} of them rounded the other way");
    assert!(
        unknown > 0 && walled > 0 && lined_columns > 0,
        "every kind of column met"
    );
    tops.end("tops");
    levels.end("levels");
}
