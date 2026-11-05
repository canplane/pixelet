//! The light's slide by hand (frame/shader/light_slide.wgsl) against pxl-engine's light:
//! `light_slide_cs` sets a tile's read to none wherever the box slid it off or into another
//! band (`Light::keeps` false), so no cast reads a depth cast for another place. A tile it may
//! keep it may also set to none. And it sets to none every tile whose rays may meet a column of
//! the world that changed (`Light::reaches`), the box not slid. Skipped where no adapter
//! answers.

mod gpu;

use pxl_engine::light::{self, Light};
use pxl_engine::{eye, Zoom, LEAF_CELL};
use pxl_render::frame::shader::record::{src, SHADER_LIGHT_SLIDE};
use pxl_render::frame::shader::word::SQUARE_SIDE;
use pxl_render::gpu::shadow;

use gpu::{mix, Gpu, LightBox, Tally};

/// The shipped map's side, and its tiles a side.
const SIZE: u32 = shadow::SIZE;
const N: u32 = light::tiles(SIZE);
/// The eye's zooms, which every band is held to.
const RANGE: [Zoom; 2] = [8, LEAF_CELL + 3];

/// A light box along `dir` about the eye, `side` units a texel, its corner at `corner` and
/// its receivers reading within `reach_ndc` of its half-width.
fn box_of(dir: [f32; 3], side: f32, reach_ndc: f32, corner: [i32; 2]) -> Light {
    let b = LightBox {
        size: SIZE,
        origin: [0; 3],
        dir,
        side,
        back: 60.0,
        t_max: 400.0,
        corner,
        reach_ndc,
    };
    b.lay().0
}

fn eye_words(l: &Light) -> Vec<u32> {
    let mut w = vec![0u32; eye::WORDS];
    w[eye::LIGHT..eye::LIGHT + eye::LIGHT_WORDS].copy_from_slice(&l.words());
    w[eye::RANGE] = RANGE[0];
    w[eye::RANGE + 1] = RANGE[1];
    w
}

fn head_words(current: u32, epoch: u32, was: [i32; 2]) -> Vec<u32> {
    let mut head = vec![0u32; light::HEAD_WORDS];
    head[light::HEAD_CAST] = current;
    head[light::HEAD_EPOCH] = epoch;
    for (k, v) in light::head_layout(SIZE) {
        head[k] = v;
    }
    head[light::HEAD_CORNER_WAS] = was[0] as u32;
    head[light::HEAD_CORNER_WAS + 1] = was[1] as u32;
    head
}

fn run(gpu: &Gpu, p: &wgpu::ComputePipeline, l: &Light, list: &[u32], was: [i32; 2]) -> Vec<u32> {
    run_with(gpu, p, l, list, head_words(40, 37, was))
}

fn run_with(
    gpu: &Gpu,
    p: &wgpu::ComputePipeline,
    l: &Light,
    list: &[u32],
    head: Vec<u32>,
) -> Vec<u32> {
    let eye = gpu.buffer(&eye_words(l), wgpu::BufferUsages::UNIFORM);
    let head = gpu.buffer(&head, wgpu::BufferUsages::UNIFORM);
    let list = gpu.buffer(list, wgpu::BufferUsages::STORAGE);
    gpu.dispatch(
        p,
        &[
            (0, eye.as_entire_binding()),
            (11, head.as_entire_binding()),
            (12, list.as_entire_binding()),
        ],
        [N.div_ceil(SQUARE_SIDE), N.div_ceil(SQUARE_SIDE), 1],
    );
    gpu.read(&list)
}

#[test]
fn hand_slide_forgets_what_moved() {
    let Some(gpu) = Gpu::new() else {
        eprintln!("no adapter: skipped");
        return;
    };
    let p = gpu.pipeline(src(SHADER_LIGHT_SLIDE), "light_slide_cs");
    let t = light::TILE as i32;
    let mut tally = Tally::default();
    let (mut kept, mut off, mut banded) = (0u32, 0u32, 0u32);
    // Where the corner was, and where it is.
    let slides: [(&str, [i32; 2], [i32; 2]); 7] = [
        ("a tile across", [1304, -896], [1304 + t, -896]),
        ("a tile up", [1304, -896], [1304, -896 - t]),
        (
            "three across and two down",
            [1304, -896],
            [1304 - 3 * t, -896 + 2 * t],
        ),
        ("over X = 0", [-2 * t, 40 * t], [3 * t, 39 * t]),
        (
            "over the i32 seam",
            [i32::MAX - t + 1, 0],
            [(i32::MAX - t + 1).wrapping_add(2 * t), t],
        ),
        ("far, nine tiles each way", [0, 0], [9 * t, -9 * t]),
        ("past the box", [0, 0], [(N as i32 + 1) * t, 0]),
    ];
    for (n, (name, was, corner)) in slides.into_iter().enumerate() {
        let l = box_of([0.3, 0.2, -0.93], 0.0625, 0.75, corner);
        let list: Vec<u32> = (0..light::list_words(SIZE))
            .map(|k| mix(k, n as u32) | 1)
            .collect();
        let have = run(&gpu, &p, &l, &list, was);
        let reads = light::reads_at(SIZE);
        let (mut want, mut got) = (Vec::new(), Vec::new());
        let (mut case_kept, mut case_off) = (0, 0);
        for j in 0..N {
            for i in 0..N {
                if l.keeps(SIZE, RANGE, [i, j], was) {
                    case_kept += 1;
                    continue;
                }
                want.push(0);
                got.push(have[(reads + j * N + i) as usize]);
                case_off += 1;
                let d = [
                    corner[0].wrapping_sub(was[0]) / t,
                    corner[1].wrapping_sub(was[1]) / t,
                ];
                let from = [i as i32 + d[0], j as i32 + d[1]];
                if (0..N as i32).contains(&from[0]) && (0..N as i32).contains(&from[1]) {
                    banded += 1;
                }
            }
        }
        eprintln!("{name}: {case_kept} may keep, {case_off} set to none");
        tally.compare(name, &want, &got, |k| format!("tile set to none {k}"));
        kept += case_kept;
        off += case_off;
    }
    eprintln!(
        "slides: {kept} tiles may keep, {off} set to none, {banded} of them moved into another band"
    );
    assert!(kept > 0 && off > 0 && banded > 0);
    tally.end("slides");
}

/// A world's change: its name, the light's direction, and its columns.
type Change = (&'static str, [f32; 3], &'static [[f32; 4]]);

#[test]
fn hand_slide_forgets_what_a_world_change_reaches() {
    let Some(gpu) = Gpu::new() else {
        eprintln!("no adapter: skipped");
        return;
    };
    let p = gpu.pipeline(src(SHADER_LIGHT_SLIDE), "light_slide_cs");
    let mut tally = Tally::default();
    let (mut kept, mut off, mut extra) = (0u32, 0u32, 0u32);
    let corner = [1304, -896];
    // Columns from the eye, in units: a brick's, an edit's a few cells wide, one far off the
    // box, and the world's whole width.
    let changes: [Change; 4] = [
        (
            "a brick under the eye, the light high",
            [0.3, 0.2, -0.93],
            &[[0.0, 0.0, 64.0, 64.0]],
        ),
        (
            "two edits, the light low",
            [0.8, 0.1, -0.3],
            &[[-20.0, 5.0, -17.0, 9.0], [30.0, -40.0, 31.0, -39.0]],
        ),
        (
            "far off the box",
            [0.3, 0.2, -0.93],
            &[[9000.0, 9000.0, 9064.0, 9064.0]],
        ),
        (
            "the world's width",
            [0.3, 0.2, -0.93],
            &[[-1.0e30, 10.0, 1.0e30, 74.0]],
        ),
    ];
    for (n, (name, dir, columns)) in changes.into_iter().enumerate() {
        let l = box_of(dir, 0.0625, 0.75, corner);
        let list: Vec<u32> = (0..light::list_words(SIZE))
            .map(|k| mix(k, n as u32) | 1)
            .collect();
        let mut head = head_words(40, 37, corner);
        let at = 4 * light::COLUMNS_AT;
        head[at] = columns.len() as u32;
        for (k, c) in columns.iter().enumerate() {
            head[at + 4 * (1 + k)..at + 4 * (2 + k)].copy_from_slice(&c.map(f32::to_bits));
        }
        let have = run_with(&gpu, &p, &l, &list, head);
        let reads = light::reads_at(SIZE);
        let (mut want, mut got) = (Vec::new(), Vec::new());
        let (mut case_kept, mut case_off) = (0, 0);
        for j in 0..N {
            for i in 0..N {
                let word = have[(reads + j * N + i) as usize];
                if !columns.iter().any(|&c| l.reaches(SIZE, RANGE, [i, j], c)) {
                    case_kept += 1;
                    extra += u32::from(word == 0);
                    continue;
                }
                want.push(0);
                got.push(word);
                case_off += 1;
            }
        }
        eprintln!("{name}: {case_kept} kept, {case_off} set to none");
        tally.compare(name, &want, &got, |k| format!("tile set to none {k}"));
        kept += case_kept;
        off += case_off;
    }
    eprintln!("world changes: {kept} tiles kept, {off} set to none, {extra} kept set to none");
    assert!(kept > 0 && off > 0, "too few to say");
    assert!(extra <= 16, "{extra} tiles no column reaches set to none");
    tally.end("world changes");
}
