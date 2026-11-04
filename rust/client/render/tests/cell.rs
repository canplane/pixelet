//! `cell_cs` by hand (frame/shader/cell.wgsl) finds where each of the eye's cells' rays start
//! as pxl-shader's `observer::cell` does for the eye, over pxl-engine's `beam::Grid`: the
//! least of its window's beam rays' hand-on distances, that corner, and whether its rays
//! resume. Over grids at the screens a frame lays (wide, tall, narrow, a lattice of one cell
//! and of two), the distances as beam rays leave them (ties, none met, nought). Every word of
//! the beam rays' buffer compared. Skipped where no adapter answers.

mod gpu;

use pxl_engine::beam::Grid;
use pxl_engine::cast::{Detail, Spacing};
use pxl_engine::eye;
use pxl_engine::grid::LATTICE_MAX;
use pxl_render::frame::shader::record::{src, SHADER_CELL};
use pxl_render::frame::shader::word::CELL_SIDE;

use gpu::{Gpu, Tally};

/// The screens' half-angles' tangents across and up: a 16:9 screen at 90° across, a phone
/// held upright, a wide one, a narrow zoom, and one at 120° across.
const TANS: [[f32; 2]; 5] = [
    [1.0, 0.5625],
    [0.36, 0.64],
    [2.4, 0.9],
    [0.12, 0.08],
    [1.732_050_8, 0.97],
];

/// The ulps of its ceiling's argument from a whole number within which a window may be the
/// next one.
const EDGE_ULPS: f64 = 32.0;

fn mix(a: u32, b: u32) -> u32 {
    let mut h = a.wrapping_mul(0x9E37_79B1) ^ b.wrapping_mul(0x85EB_CA77);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^ h >> 12
}

/// The distance corner `c`'s beam ray hands on, as `beam_cs` leaves it: anywhere out to
/// the eye's reach, the same as its neighbor's in runs, none met (infinite), the largest,
/// at the eye; so rarely the least of a window that the window's side tells its least.
fn hand(c: u32, seed: u32) -> f32 {
    let h = mix(c / 3, seed);
    match h % 1024 {
        0 => 0.0,
        1..=16 => f32::MAX,
        17..=80 => f32::INFINITY,
        // Runs of three corners alike.
        81..=300 => (h >> 10) as f32 / 4096.0,
        _ => (mix(c, seed) >> 4) as f32 / 1000.0,
    }
}

/// The eye's words for a grid: its tangents and the grid's, the rest nought.
fn eye_words(g: &Grid) -> Vec<u32> {
    let mut w = vec![0u32; eye::WORDS];
    w[eye::TAN] = g.tan[0].to_bits();
    w[eye::TAN + 1] = g.tan[1].to_bits();
    w[eye::GRID..eye::GRID + eye::GRID_WORDS].copy_from_slice(&g.words());
    w
}

/// Cell `c`'s start, its window `k` corners past it a side (pxl-shader observer.rs `cell`
/// over eye.rs `Eye`'s window and detail): the least distance, its corner, and whether its
/// window's beam rays asked as its rays ask.
fn start(g: &Grid, words: &[u32], c: [u32; 2], k: u32) -> [u32; 3] {
    let detail = Detail {
        range: [0, 0],
        spacing: Spacing {
            angle: f32::from_bits(g.words()[0]),
            across: 0.0,
        },
    };
    let across = g.corners[0];
    let [x, y] = g.first(c);
    let ([x, y], side) = ([x.wrapping_sub(k), y.wrapping_sub(k)], 2 + 2 * k);
    let t_of = |i: u32| f32::from_bits(words[g.hand_at(i) as usize]);
    let mut from = y * across + x;
    let mut least = t_of(from);
    let mut alike = true;
    for j in 0..side {
        for i in 0..side {
            let at = (y + j) * across + x + i;
            if t_of(at) < least {
                (least, from) = (t_of(at), at);
            }
            alike = alike && detail.alike(detail);
        }
    }
    [least.to_bits(), from, u32::from(alike)]
}

/// Whether cell `c`'s window `k` corners past it a side lies within the grid's corners.
fn fits(g: &Grid, c: [u32; 2], k: u32) -> bool {
    let [x, y] = g.first(c);
    x >= k && y >= k && x + k + 2 <= g.corners[0] && y + k + 2 <= g.corners[1]
}

/// Whether cell `c`'s window takes the ceiling of a number within `EDGE_ULPS` of a whole
/// one (`Grid::cell_window`, taken in f64 here): a GPU's square root and divisions are not
/// the Rust's to the last place, and a product it fuses into a sum is rounded once.
fn on_edge(g: &Grid, c: [u32; 2]) -> bool {
    let w = g.words();
    let f = |k: usize| f64::from(f32::from_bits(w[k]));
    let (s, radius, radius_tan) = (f(0), f(2), f(3));
    let far = |t: f32, x: u32| {
        let t = f64::from(t);
        (-t + f64::from(x) * s)
            .abs()
            .max((-t + f64::from(x + 1) * s).abs())
    };
    let at = [far(g.tan[0], c[0]), far(g.tan[1], c[1])];
    let edge = (at[0] * at[0] + at[1] * at[1]).sqrt();
    let beyond = (edge + radius_tan) / (1.0 - edge * radius_tan);
    let v = std::f64::consts::FRAC_1_SQRT_2 + radius * (1.0 + beyond * beyond) / s;
    let ulp = {
        let x = v as f32;
        f64::from(f32::from_bits(x.to_bits() + 1) - x)
    };
    (v - v.round_ties_even()).abs() <= EDGE_ULPS * ulp
}

#[test]
fn hand_cell_starts_as_observer_cell() {
    let Some(gpu) = Gpu::new() else {
        eprintln!("no adapter: skipped");
        return;
    };
    let p = gpu.pipeline(src(SHADER_CELL), "cell_cs");
    let mut tally = Tally::default();
    let (mut cells, mut edges, mut edges_apart, mut ties) = (0u32, 0u32, 0u32, 0u32);
    for (n, tan) in TANS.into_iter().enumerate() {
        for lattice in 1..=LATTICE_MAX {
            let g = Grid::of(tan, lattice);
            let seed = n as u32 * 16 + lattice;
            let mut words: Vec<u32> = (0..g.size()).map(|k| mix(k, !seed)).collect();
            for c in 0..g.corners[0] * g.corners[1] {
                words[g.hand_at(c) as usize] = hand(c, seed).to_bits();
            }
            let eye_buf = gpu.buffer(&eye_words(&g), wgpu::BufferUsages::UNIFORM);
            let beams_buf = gpu.buffer(&words, wgpu::BufferUsages::STORAGE);
            gpu.dispatch(
                &p,
                &[
                    (0, eye_buf.as_entire_binding()),
                    (8, beams_buf.as_entire_binding()),
                ],
                [
                    g.span[0].div_ceil(CELL_SIDE),
                    g.span[1].div_ceil(CELL_SIDE),
                    1,
                ],
            );
            let have = gpu.read(&beams_buf);
            // Each cell's start, and where its window is on the edge, the start of the
            // windows either side.
            let mut want = words.clone();
            let mut either = vec![None; words.len()];
            for y in 0..g.span[1] {
                for x in 0..g.span[0] {
                    let (c, at) = ([x, y], g.cell_at([x, y]) as usize);
                    let k = g.cell_window(c);
                    let s = start(&g, &words, c, k);
                    want[at..at + 3].copy_from_slice(&s);
                    cells += 1;
                    assert!(fits(&g, c, k), "cell {c:?}'s window lies in the grid");
                    if fits(&g, c, k + 1) {
                        ties += u32::from(start(&g, &words, c, k + 1)[0] == s[0]);
                    }
                    if on_edge(&g, c) {
                        edges += 1;
                        let near = |k: u32| {
                            if fits(&g, c, k) {
                                start(&g, &words, c, k)
                            } else {
                                s
                            }
                        };
                        let other = [near(k.wrapping_sub(1)), near(k + 1)];
                        edges_apart += u32::from(have[at..at + 3] != s);
                        for w in 0..3 {
                            either[at + w] = Some([other[0][w], other[1][w]]);
                        }
                    }
                }
            }
            let case = format!(
                "tan {tan:?}, lattice {lattice}, {} cells",
                g.span[0] * g.span[1]
            );
            tally.compare_by(
                &case,
                &want,
                &have,
                |k| format!("word {k}"),
                |k, w, h| w == h || either[k].is_some_and(|o: [u32; 2]| o.contains(&h)),
            );
        }
    }
    eprintln!("{cells} cells, {ties} whose least is the next window's too, {edges} windows on the edge, {edges_apart} of them the next");
    tally.end("cells");
}
