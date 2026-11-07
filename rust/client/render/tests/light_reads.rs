//! The light's slide by hand (frame/shader/light_slide.wgsl) against pxl-engine's light:
//! `light_slide_cs` sets a tile's read to none wherever the box slid it off or into another
//! band (`Light::keeps` false), so no cast reads a depth cast for another place. A tile it may
//! keep it may also set to none. And it sets to none every tile whose rays may meet a column of
//! the world that changed (`Light::reaches`), the box not slid; and names the cast as the last
//! its slide ran in (`light::SLIDE_RAN`). Skipped where no adapter answers.

mod gpu;

use pxl_content::generation::{TALLEST, WIDEST};
use pxl_engine::light::{self, Light};
use pxl_engine::{eye, param, ray, Zoom, LEAF_CELL};
use pxl_render::frame::shader::record::{src, SHADER_LIGHT_PROPAGATE, SHADER_LIGHT_SLIDE};
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

/// The head of cast `current` in `epoch`, the box's corner the cast before's `was` and its
/// eye's zooms `range_was`.
fn head_words(current: u32, epoch: u32, was: [i32; 2], range_was: [Zoom; 2]) -> Vec<u32> {
    let mut head = vec![0u32; light::HEAD_WORDS];
    head[light::HEAD_CAST] = current;
    head[light::HEAD_EPOCH] = epoch;
    for (k, v) in light::head_layout(SIZE) {
        head[k] = v;
    }
    head[light::HEAD_CORNER_WAS] = was[0] as u32;
    head[light::HEAD_CORNER_WAS + 1] = was[1] as u32;
    head[light::HEAD_RANGE_WAS] = range_was[0];
    head[light::HEAD_RANGE_WAS + 1] = range_was[1];
    head
}

fn run(
    gpu: &Gpu,
    p: &wgpu::ComputePipeline,
    l: &Light,
    list: &[u32],
    was: [i32; 2],
    range_was: [Zoom; 2],
) -> Vec<u32> {
    run_with(gpu, p, l, list, head_words(40, 37, was, range_was))
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

/// A slide: its name, where the box's corner was and where it is, and the eye's zooms the cast
/// before was cast under.
type Slide = (&'static str, [i32; 2], [i32; 2], [Zoom; 2]);

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
    // Where the corner was, and where it is; the eye's zooms the cast before was cast under.
    let step = [RANGE[0] - 1, RANGE[1] - 1];
    let slides: [Slide; 9] = [
        ("a tile across", [1304, -896], [1304 + t, -896], RANGE),
        ("a tile up", [1304, -896], [1304, -896 - t], RANGE),
        (
            "three across and two down",
            [1304, -896],
            [1304 - 3 * t, -896 + 2 * t],
            RANGE,
        ),
        ("over X = 0", [-2 * t, 40 * t], [3 * t, 39 * t], RANGE),
        (
            "over the i32 seam",
            [i32::MAX - t + 1, 0],
            [(i32::MAX - t + 1).wrapping_add(2 * t), t],
            RANGE,
        ),
        ("far, nine tiles each way", [0, 0], [9 * t, -9 * t], RANGE),
        ("past the box", [0, 0], [(N as i32 + 1) * t, 0], RANGE),
        (
            "a zoom step, the box standing",
            [1304, -896],
            [1304, -896],
            step,
        ),
        (
            "a zoom step and a tile across",
            [1304, -896],
            [1304 + t, -896],
            step,
        ),
    ];
    for (n, (name, was, corner, range_was)) in slides.into_iter().enumerate() {
        let l = box_of([0.3, 0.2, -0.93], 0.0625, 0.75, corner);
        let list: Vec<u32> = (0..light::list_words(SIZE))
            .map(|k| mix(k, n as u32) | 1)
            .collect();
        let have = run(&gpu, &p, &l, &list, was, range_was);
        // The slide's first thread names the cast the slide last ran in (`light::SLIDE_RAN`).
        let ran = (light::slide_at(SIZE) + light::SLIDE_RAN) as usize;
        tally.compare(
            &format!("{name}: the slide ran"),
            &[40],
            &[have[ran]],
            |_| "the last cast the slide ran in".into(),
        );
        let reads = light::reads_at(SIZE);
        let (mut want, mut got) = (Vec::new(), Vec::new());
        let (mut case_kept, mut case_off) = (0, 0);
        for j in 0..N {
            for i in 0..N {
                if l.keeps(SIZE, [RANGE, range_was], [i, j], was) {
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
        let mut head = head_words(40, 37, corner, RANGE);
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

/// Where a point `v` units from the eye lies on the box, in texels from texel (0, 0)'s middle,
/// as light_box.wgsl `texel_at` reads it.
fn texel_at(l: &Light, v: [f32; 3]) -> [f32; 2] {
    let d = [0, 1, 2].map(|k| v[k] - l.first[k]);
    let dot = |a: [f32; 3]| d[0] * a[0] + d[1] * a[1] + d[2] * a[2];
    [
        dot(l.across) * l.across2_inverse(),
        dot(l.down) * l.down2_inverse(),
    ]
}

/// The propagation by hand (frame/shader/light_propagate.wgsl), a thread a stood tile
/// (`light::LISTED_STOOD`), against this test's reading of its rule: a tile is read where it
/// lies within the disk a receiver reads grown by `light::TILE_READ_PAD` and within the taps
/// (`light::READ_TILES`) of a step up the way up decor's tallest from a stood tile; each read
/// tile listed once (`light::LISTED_READ`), the lay opened to as many (`light::ARGS_LAY`). The
/// stood tiles propagated in two casts, as a cast adds to what the casts before stood on and
/// propagated (`light::kept_at`): the second takes only the tiles past the count kept, and the
/// reads the two list are those all of them read. Then (`light_claim_cs`) each tile read claimed
/// where its word names a cast before the reads' epoch, the later of the host's and the one a
/// slide unrun began (`light::reads_epoch`): its word the cast's, and listed
/// (`light::LISTED_CLAIMED`). A step by a tile's edge may read either tile. Every texel a placed
/// decor's pixel reads lies in a tile read, the decor stood on its footprint alone
/// (`Light::corner_tiles`). Skipped where no adapter answers.
#[test]
fn hand_propagation_reads_what_stands() {
    let Some(gpu) = Gpu::new() else {
        eprintln!("no adapter: skipped");
        return;
    };
    let p = gpu.pipeline(src(SHADER_LIGHT_PROPAGATE), "light_propagate_cs");
    let claim = gpu.pipeline(src(SHADER_LIGHT_PROPAGATE), "light_claim_cs");
    let mut tally = Tally::default();
    let (current, epoch) = (40u32, 30u32);
    for (case, dir, reads_from) in [
        ("the sun high, the host's epoch", [0.3, 0.2, -0.93], 0u32),
        ("the sun low, a slide unrun", [0.8, 0.3, -0.25], 38),
    ] {
        let l = box_of(dir, 0.0625, 0.75, [1304, -896]);
        let half = N as i32 / 2;
        // Some tiles stood on about the box's middle, each once.
        let mut stood: std::collections::BTreeSet<[i32; 2]> = (0..300u32)
            .map(|k| {
                let r = mix(k, 7);
                [
                    half - 40 + (r % 80) as i32,
                    half - 40 + (r >> 8 & 0x7f) as i32 % 80,
                ]
            })
            .collect();
        // Placed decor, each stood on its footprint, its base anywhere on a tile.
        let wide = WIDEST as f32 / 2.0;
        let bases: Vec<[f32; 3]> = (0..64u32)
            .map(|k| [11, 13, 17].map(|s| (mix(k, s) % 4001) as f32 / 100.0 - 20.0))
            .collect();
        for b in &bases {
            let foot = |d: f32| [b[0] + d, b[1] + d, b[2]];
            let [first, last] = l
                .corner_tiles(SIZE, foot(-wide), foot(wide), false)
                .expect("a base on the box");
            for j in first[1]..=last[1] {
                for i in first[0]..=last[0] {
                    stood.insert([i, j]);
                }
            }
        }
        let stood: Vec<[i32; 2]> = stood.into_iter().collect();
        // The way up, its steps and each step's tiles, either side where a step lies by an
        // edge.
        let low = texel_at(&l, [0.0; 3]);
        let top = texel_at(&l, [0.0, 0.0, TALLEST as f32]);
        let way = [0, 1].map(|k| (top[k] - low[k]) / light::TILE as f32);
        let steps = way[0].abs().max(way[1].abs()).ceil() as u32;
        let ups: Vec<(Vec<[i32; 2]>, bool)> = (0..=steps)
            .map(|k| {
                let f = if steps == 0 {
                    0.0
                } else {
                    k as f32 / steps as f32
                };
                let at = |k: usize, e: f32| (way[k] * f + e).floor() as i32;
                let xs = [at(0, -1e-4), at(0, 1e-4)];
                let ys = [at(1, -1e-4), at(1, 1e-4)];
                let mut o = vec![
                    [xs[0], ys[0]],
                    [xs[1], ys[0]],
                    [xs[0], ys[1]],
                    [xs[1], ys[1]],
                ];
                o.dedup();
                o.sort();
                o.dedup();
                (o.clone(), o.len() == 1)
            })
            .collect();
        let reads_disk = |t: [i32; 2]| {
            let middle = [0, 1].map(|k| t[k] * light::TILE as i32 + light::TILE as i32 / 2);
            l.read(SIZE, middle, light::TILE_READ_PAD)
        };
        let inside = |t: [i32; 2]| t[0] >= 0 && t[1] >= 0 && t[0] < N as i32 && t[1] < N as i32;
        let (mut must, mut may) = (
            std::collections::BTreeSet::new(),
            std::collections::BTreeSet::new(),
        );
        let [first, last] = light::READ_TILES;
        for s in &stood {
            for (offsets, sure) in &ups {
                for o in offsets {
                    for b in first..=last {
                        for a in first..=last {
                            let t = [s[0] + o[0] - a, s[1] + o[1] - b];
                            if inside(t) && reads_disk(t) {
                                may.insert(t);
                                if *sure {
                                    must.insert(t);
                                }
                            }
                        }
                    }
                }
            }
        }
        // The list: the stood tiles listed, the reads some cast's each, the slide's words.
        let mut list: Vec<u32> = (0..light::buffer_words(SIZE)).map(|k| mix(k, 3)).collect();
        let stood_at = light::listed_at(SIZE, light::LISTED_STOOD) as usize;
        list[stood_at] = stood.len() as u32;
        for (e, t) in stood.iter().enumerate() {
            list[stood_at + 1 + e] = (t[1] as u32) << 16 | t[0] as u32;
        }
        let reads = light::reads_at(SIZE) as usize;
        let was: Vec<u32> = (0..N * N)
            .map(|t| [0, 29, 30, 37, 38, 41][(mix(t, 5) % 6) as usize])
            .collect();
        list[reads..reads + (N * N) as usize].copy_from_slice(&was);
        let slide = light::slide_at(SIZE) as usize;
        list[slide + light::READS_FROM as usize] = reads_from;
        let mut args = vec![0u32; light::args_words(SIZE) as usize];
        args[light::ARGS_LAY as usize] = light::LAY_VERTICES;
        // The stood tiles' first half propagated by a cast before, which stood the world's
        // receivers afresh (`light::HEAD_WORLD`); none of them yet.
        let kept = light::kept_at(SIZE) as usize;
        let half = stood.len() / 2;
        list[stood_at] = half as u32;
        list[kept + light::STOOD_CNT as usize] = 0;
        let mut head = head_words(current, epoch, [1304, -896], RANGE);
        head[light::HEAD_SLIDE_OWED] = 0;
        head[light::HEAD_WORLD] = current - 3;
        let eye = gpu.buffer(&eye_words(&l), wgpu::BufferUsages::UNIFORM);
        let head = gpu.buffer(&head, wgpu::BufferUsages::UNIFORM);
        let list_buf = gpu.buffer(&list, wgpu::BufferUsages::STORAGE);
        let args_buf = gpu.buffer(&args, wgpu::BufferUsages::STORAGE);
        let per = ray::MISS_LANES * ray::MISS_ACROSS;
        let propagation = |list: &wgpu::Buffer, p: &wgpu::ComputePipeline, groups: [u32; 3]| {
            gpu.dispatch(
                p,
                &[
                    (0, eye.as_entire_binding()),
                    (11, head.as_entire_binding()),
                    (12, list.as_entire_binding()),
                    (21, args_buf.as_entire_binding()),
                ],
                groups,
            );
        };
        let rows = |cnt: usize| [ray::MISS_ACROSS, (cnt as u32).div_ceil(per), 1];
        propagation(&list_buf, &p, rows(half));
        // Then this cast: the rest stood on, the first half's count kept as propagated, the
        // reads kept with their count (the args, as the host copies the count back in).
        let mut now = gpu.read(&list_buf);
        now[stood_at] = stood.len() as u32;
        now[kept + light::STOOD_CNT as usize] = half as u32;
        let list_buf = gpu.buffer(&now, wgpu::BufferUsages::STORAGE);
        propagation(&list_buf, &p, rows(stood.len()));
        propagation(
            &list_buf,
            &claim,
            [(N * N).div_ceil(light::LISTED_LANES), 1, 1],
        );
        let out = gpu.read(&list_buf);
        let out_args = gpu.read(&args_buf);
        let read_cnt = out_args[(light::ARGS_COUNTS + light::LISTED_READ) as usize] as usize;
        let at = light::listed_at(SIZE, light::LISTED_READ) as usize;
        let listed: Vec<[i32; 2]> = out[at + 1..at + 1 + read_cnt]
            .iter()
            .map(|&w| [(w & 0xffff) as i32, (w >> 16) as i32])
            .collect();
        let got: std::collections::BTreeSet<[i32; 2]> = listed.iter().copied().collect();
        // What a decor's pixel reads: anywhere on its model, from the tenth of a unit a pine
        // sinks under its base to the tallest, a texel off along its normal, a tap and the
        // neighbor it filters in each way (pxl-render shadow.wgsl `shadow_visibility`).
        let (reach, tile) = (param::light::SPREAD + 2.0, light::TILE as f32);
        let mut unread = Vec::new();
        for (n, b) in bases.iter().enumerate() {
            for k in 0..256u32 {
                let share = |s: u32| (mix(k + 256 * n as u32, s) % 1001) as f32 / 1000.0;
                let at = [
                    b[0] + wide * [-1.0, 1.0, 2.0 * share(19) - 1.0][k as usize % 3],
                    b[1] + wide * [-1.0, 1.0, 2.0 * share(23) - 1.0][k as usize / 3 % 3],
                    b[2] + [-0.1, TALLEST as f32, share(29) * TALLEST as f32][k as usize / 9 % 3],
                ];
                let t = texel_at(&l, at);
                for d in [[-1.0, -1.0], [-1.0, 1.0], [1.0, -1.0], [1.0, 1.0]] {
                    let c = [0, 1].map(|a| ((t[a] + 0.5 + d[a] * reach) / tile).floor() as i32);
                    if inside(c) && reads_disk(c) && !got.contains(&c) {
                        unread.push(c);
                    }
                }
            }
        }
        tally.compare(
            &format!("{case}: every tile a decor's pixel reads is read"),
            &[0],
            &[unread.len() as u32],
            |_| format!("unread {unread:?}"),
        );
        let ep = light::reads_epoch(epoch, reads_from);
        let missing: Vec<_> = must.difference(&got).copied().collect();
        let extra: Vec<_> = got.difference(&may).copied().collect();
        let lay = out_args[light::ARGS_LAY as usize + 1];
        tally.compare(
            &format!("{case}: the tiles read, each once, the lay's instances as many"),
            &[read_cnt as u32, read_cnt as u32],
            &[got.len() as u32, lay],
            |k| ["listed once", "the lay's instances"][k].into(),
        );
        tally.compare(
            &format!("{case}: the tiles read, none missing, none past"),
            &[0, 0],
            &[missing.len() as u32, extra.len() as u32],
            |k| [format!("missing {missing:?}"), format!("past {extra:?}")][k].clone(),
        );
        // Each tile's read word: the cast's where it is read and named a cast before the reads'
        // epoch, as it was elsewhere.
        let (mut want, mut have) = (Vec::new(), Vec::new());
        for j in 0..N as i32 {
            for i in 0..N as i32 {
                let k = (j as u32 * N + i as u32) as usize;
                let claims = got.contains(&[i, j]) && was[k] < ep;
                want.push(if claims { current } else { was[k] });
                have.push(out[reads + k]);
            }
        }
        let claimed = want.iter().zip(&was).filter(|(w, v)| w != v).count() as u32;
        tally.compare(&format!("{case}: the reads"), &want, &have, |k| {
            format!("tile {} {}", k as u32 % N, k as u32 / N)
        });
        tally.compare(
            &format!("{case}: the claimed listed"),
            &[claimed],
            &[out_args[(light::ARGS_COUNTS + light::LISTED_CLAIMED) as usize]],
            |_| "the claimed list's count".into(),
        );
        assert!(
            claimed > 0 && read_cnt > stood.len(),
            "{case}: {claimed} {read_cnt}"
        );
    }
    tally.end("light_propagate_cs");
}
