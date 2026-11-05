//! The light's reads by hand (frame/shader/light_propagate.wgsl, light_slide.wgsl) against
//! pxl-engine's light: `light_propagate_cs` claims each tile a receiver reads
//! (`Light::reads_tile`, `Light::up`, `Light::up_at`, `light::READ_TILES`) that no cast of the
//! epoch read, marks it and its square (`light::reads_at`, `light::coarse_square`), and either
//! copies the whole dispatch in (`light::ARGS_WHOLE`) or lists what feeds it at each level
//! within the reach each feeds (`light::CELL_REACH`, `light::CORNER_REACH`), as the beam's, the
//! cells' and the slots' passes ask it the other way (light_list.wgsl `light_needs`); the
//! lists' rows and their marks are this test's own reading of light_propagate.wgsl `enlist`,
//! which pxl-engine has no function for. `light_slide_cs` sets a tile's read to none where the
//! box slid it off or into another band (`Light::keeps`). Every word of the list's buffer and
//! of the args compared, a list's entries in their order of claim apart, which the GPU's
//! atomics decide. Skipped where no adapter answers.

mod gpu;

use pxl_content::Pixelet;
use pxl_engine::light::{self, Light};
use pxl_engine::param::light::SAMPLE;
use pxl_engine::{eye, Zoom, LEAF_CELL};
use pxl_render::frame::shader::record::{src, SHADER_LIGHT_PROPAGATE, SHADER_LIGHT_SLIDE};
use pxl_render::frame::shader::word::SQUARE_SIDE;
use pxl_render::gpu::shadow;

use gpu::{mix, ulps, Gpu, LightBox, Tally};

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

fn tile_at(first: u32, [i, j]: [i32; 2]) -> Option<usize> {
    let n = N as i32;
    (i >= 0 && j >= 0 && i < n && j < n).then(|| (first as i32 + j * n + i) as usize)
}

/// What a propagation is handed: the light, the cast's number and its epoch's, the list's
/// buffer with its stands, reads, marks and lists laid, and the args.
struct Case {
    name: &'static str,
    light: Light,
    current: u32,
    epoch: u32,
    list: Vec<u32>,
    args: Vec<u32>,
}

/// The tiles receivers stand on about the eye, as light_mark.wgsl marks them off where the
/// eye's rays meet the world (`Light::tile_of`): ground a few units of bumps under the eye
/// out to `reach` units, decor scattered over it, and stands an earlier cast left.
fn stand(list: &mut [u32], l: &Light, current: u32, reach: f32, seed: u32) {
    let at = light::stands_at(SIZE);
    let mut put = |v: [f32; 3]| {
        if let Some(w) = tile_at(at, l.tile_of(v)) {
            list[w] = current;
        }
    };
    let step = 0.37;
    let k = (reach / step) as i32;
    for y in -k..=k {
        for x in -k..=k {
            let (x, y) = (x as f32 * step, y as f32 * step);
            if x * x + y * y > reach * reach {
                continue;
            }
            // The ground the eye's pixels meet, nearer the eye more of it.
            let z = -2.0 + 0.8 * libm::sinf(x * 0.21) + 0.6 * libm::cosf(y * 0.17);
            if mix((x * 100.0) as i32 as u32, (y * 100.0) as i32 as u32 ^ seed).is_multiple_of(4) {
                put([x, y, z]);
            }
            // Decor standing on it here and there.
            if mix((x * 37.0) as i32 as u32, (y * 53.0) as i32 as u32 ^ seed).is_multiple_of(97) {
                put([x + 0.5, y - 0.25, z]);
            }
        }
    }
    // Stands an earlier cast left, in rows across the box, and the box's edges stood on.
    for j in 0..N {
        for i in 0..N {
            let w = (at + j * N + i) as usize;
            if list[w] != current && mix(i * 7 + j * 3, seed).is_multiple_of(23) {
                list[w] = current.wrapping_sub(1 + mix(i, j) % 3);
            }
        }
    }
    for t in [0, N / 2, N - 1] {
        for e in [[0, t], [N - 1, t], [t, 0], [t, N - 1]] {
            list[(at + e[1] * N + e[0]) as usize] = current;
        }
    }
}

/// The reads as earlier casts left them: none, an older epoch's, its own epoch's, the cast
/// before's, and this cast's own.
fn read_before(list: &mut [u32], current: u32, epoch: u32, seed: u32) {
    let at = light::reads_at(SIZE) as usize;
    for k in 0..(N * N) as usize {
        list[at + k] = match mix(k as u32, seed ^ 0x51) % 8 {
            0 | 1 => 0,
            2 => epoch.saturating_sub(5),
            3 => epoch.saturating_sub(1),
            4 => epoch,
            5 => current.wrapping_sub(1).max(epoch),
            6 => current,
            _ => epoch.saturating_sub(2),
        };
    }
}

/// Each list's marks as earlier casts left them, one a later cast's, the places this cast
/// listed already among its entries, in no order, and the rows the args opened for them.
fn mark_before(list: &mut [u32], args: &mut [u32], current: u32, seed: u32) {
    for which in [
        light::LISTED_CORNERS,
        light::LISTED_FED,
        light::LISTED_CLAIMED,
    ] {
        let marks = light::listed_marks_at(SIZE, which) as usize;
        let at = light::listed_at(SIZE, which) as usize;
        let room = light::listed_room(SIZE, which);
        let across = if which == light::LISTED_CORNERS {
            light::beam_across(SIZE)
        } else {
            N
        };
        let mut listed = Vec::new();
        for p in 0..room {
            let h = mix(p, seed ^ which);
            list[marks + p as usize] = match h % 16 {
                0 => current,
                1..=4 => current.wrapping_sub(1),
                5 => current.wrapping_sub(9),
                6 => current.wrapping_add(1),
                _ => 0,
            };
            if h.is_multiple_of(16) {
                listed.push(((p / across) << 16) | (p % across));
            }
        }
        listed.sort_by_key(|&e| mix(e, seed));
        list[at] = listed.len() as u32;
        list[at + 1..at + 1 + listed.len()].copy_from_slice(&listed);
    }
    for w in [
        light::ARGS_BEAM,
        light::ARGS_CELLS,
        light::ARGS_SAMPLE,
        light::ARGS_SAMPLE_BOX,
        light::ARGS_SLOT,
    ] {
        args[w as usize + 1] = 1;
    }
}

/// The claimed list holding as many entries as its pass's rows take whole
/// (`light::LIST_ACROSS` · `light::LISTED_LANES` threads a row, `light::TILE` squared an
/// entry), tile `but` not among them: the next claim opens a row of its own.
fn rows_full(list: &mut [u32], current: u32, but: usize) {
    let marks = light::listed_marks_at(SIZE, light::LISTED_CLAIMED) as usize;
    let at = light::listed_at(SIZE, light::LISTED_CLAIMED) as usize;
    let room = (N * N) as usize;
    list[marks..marks + room].fill(0);
    let whole = (light::LIST_ACROSS * light::LISTED_LANES / (light::TILE * light::TILE)) as usize;
    let places: Vec<usize> = (1..)
        .map(|k| k * 37 % room)
        .filter(|&p| p != but)
        .take(whole)
        .collect();
    list[at] = whole as u32;
    for (e, &p) in places.iter().enumerate() {
        list[marks + p] = current;
        list[at + 1 + e] = ((p as u32 / N) << 16) | (p as u32 % N);
    }
}

/// The args as a cast starts with them: a list's row across and no row, then each whole.
fn args_start() -> Vec<u32> {
    let mut args = vec![0u32; light::ARGS_WORDS as usize];
    for w in (0..light::ARGS_WHOLE).step_by(3) {
        args[w as usize..w as usize + 3].copy_from_slice(&[light::LIST_ACROSS, 0, 1]);
    }
    for k in 0..light::ARGS_WHOLE {
        args[(light::ARGS_WHOLE + k) as usize] = 100 + 7 * k;
    }
    args
}

/// What a propagation leaves, tallied by what each tile was.
#[derive(Default, Debug)]
struct Told {
    /// Past the disk a receiver reads.
    past: u32,
    /// Within it, read by none.
    unread: u32,
    /// Read, and held from a cast of the epoch.
    held: u32,
    /// Read, and the cast's to cast.
    claimed: u32,
    /// Entries listed anew, a list each.
    listed: [u32; 3],
    /// Tiles read through a step of the way up past the tile stood on.
    read_up: u32,
    /// Entries listed where a later cast's mark stood.
    over_mark: u32,
}

/// The propagation as pxl-engine's light lays its rule, over `c`'s buffers.
fn propagate(c: &Case) -> (Vec<u32>, Vec<u32>, Told) {
    let (l, current, epoch) = (&c.light, c.current, c.epoch);
    let (mut list, mut args) = (c.list.clone(), c.args.clone());
    let mut told = Told::default();
    let up = l.up::<Pixelet>();
    let stands = light::stands_at(SIZE);
    let stood = |t: [i32; 2]| tile_at(stands, t).is_some_and(|w| c.list[w] == current);
    let mut claimed = vec![false; (N * N) as usize];
    let [first, last] = light::READ_TILES;
    for j in 0..N {
        for i in 0..N {
            let t = [i as i32, j as i32];
            if !l.reads_tile(SIZE, t) {
                told.past += 1;
                continue;
            }
            let by = (0..=up.1).find(|&k| {
                let o = Light::up_at(up, k);
                (first..=last)
                    .any(|b| (first..=last).any(|a| stood([t[0] - o[0] + a, t[1] - o[1] + b])))
            });
            let Some(k) = by else {
                told.unread += 1;
                continue;
            };
            told.read_up += u32::from(Light::up_at(up, k) != [0, 0]);
            let w = (light::reads_at(SIZE) + j * N + i) as usize;
            if list[w] >= epoch {
                told.held += 1;
                continue;
            }
            told.claimed += 1;
            list[w] = current;
            list[light::coarse_square(SIZE, [i, j]) as usize] = current;
            if current == epoch {
                let whole = light::ARGS_WHOLE as usize;
                let (to, from) = args.split_at_mut(whole);
                to.copy_from_slice(&from[..whole]);
            } else {
                claimed[(j * N + i) as usize] = true;
            }
        }
    }
    if current == epoch {
        return (list, args, told);
    }
    // What feeds a claimed tile, by the reach it feeds within about the tile it lies on, as
    // the passes that take the lists ask it (light_list.wgsl `light_needs`).
    let any_claimed = |lo: [i32; 2], hi: [i32; 2]| {
        (lo[1].max(0)..=hi[1].min(N as i32 - 1)).any(|j| {
            (lo[0].max(0)..=hi[0].min(N as i32 - 1))
                .any(|i| claimed[(j as u32 * N + i as u32) as usize])
        })
    };
    let per = light::LIST_ACROSS * light::LISTED_LANES;
    let s = light::TILE / SAMPLE;
    for (n, which) in [
        light::LISTED_CORNERS,
        light::LISTED_FED,
        light::LISTED_CLAIMED,
    ]
    .into_iter()
    .enumerate()
    {
        let marks = light::listed_marks_at(SIZE, which) as usize;
        let at = light::listed_at(SIZE, which) as usize;
        let across = if which == light::LISTED_CORNERS {
            light::beam_across(SIZE)
        } else {
            N
        };
        for p in 0..across * across {
            let q = [p % across, p / across];
            let needed = if which == light::LISTED_CORNERS {
                let on = light::corner_at(q).map(|v| v.div_euclid(light::TILE as i32));
                let [lo, hi] = light::CORNER_REACH;
                any_claimed([on[0] + lo, on[1] + lo], [on[0] + hi, on[1] + hi])
            } else if which == light::LISTED_FED {
                let on = [q[0] as i32, q[1] as i32];
                let [lo, hi] = light::CELL_REACH;
                any_claimed([on[0] + lo, on[1] + lo], [on[0] + hi, on[1] + hi])
            } else {
                claimed[p as usize]
            };
            let was = if needed {
                std::mem::replace(&mut list[marks + p as usize], current)
            } else {
                current
            };
            if was != current {
                told.over_mark += u32::from(was > current);
                let e = list[at] as usize;
                list[at] += 1;
                list[at + 1 + e] = (q[1] << 16) | q[0];
                told.listed[n] += 1;
            }
        }
        if told.listed[n] == 0 {
            continue;
        }
        let count = list[at];
        let opened: &[(u32, u32)] = if which == light::LISTED_CLAIMED {
            &[(light::ARGS_SLOT, light::TILE)]
        } else if which == light::LISTED_FED {
            &[
                (light::ARGS_CELLS, 1),
                (light::ARGS_SAMPLE, s),
                (light::ARGS_SAMPLE_BOX, s),
            ]
        } else {
            &[(light::ARGS_BEAM, 1)]
        };
        for &(w, a) in opened {
            let rows = (count * a * a).div_ceil(per);
            let r = &mut args[w as usize + 1];
            *r = (*r).max(rows);
        }
    }
    (list, args, told)
}

/// Each list's entries in one order, the GPU's claims falling in any.
fn sorted(mut list: Vec<u32>) -> Vec<u32> {
    for which in [
        light::LISTED_CORNERS,
        light::LISTED_FED,
        light::LISTED_CLAIMED,
    ] {
        let at = light::listed_at(SIZE, which) as usize;
        let count = (list[at] as usize).min(light::listed_room(SIZE, which) as usize);
        list[at + 1..at + 1 + count].sort_unstable();
    }
    list
}

/// Where word `k` of the list's buffer lies.
fn word_of(k: usize) -> String {
    let k = k as u32;
    let parts = [
        ("stands", light::stands_at(SIZE)),
        ("reads", light::reads_at(SIZE)),
        ("squares", light::coarse_at(SIZE)),
        (
            "corners' marks",
            light::listed_marks_at(SIZE, light::LISTED_CORNERS),
        ),
        (
            "fed's marks",
            light::listed_marks_at(SIZE, light::LISTED_FED),
        ),
        (
            "claimed's marks",
            light::listed_marks_at(SIZE, light::LISTED_CLAIMED),
        ),
        (
            "corners' list",
            light::listed_at(SIZE, light::LISTED_CORNERS),
        ),
        ("fed's list", light::listed_at(SIZE, light::LISTED_FED)),
        (
            "claimed's list",
            light::listed_at(SIZE, light::LISTED_CLAIMED),
        ),
        ("lefts", light::lefts_at(SIZE)),
    ];
    match parts.iter().rev().find(|p| k >= p.1) {
        Some((name, at)) => format!("{name} + {}", k - at),
        None => format!("word {k}"),
    }
}

fn run(
    gpu: &Gpu,
    p: &wgpu::ComputePipeline,
    c: &Case,
    args: Option<&[u32]>,
    was: [i32; 2],
) -> [Vec<u32>; 2] {
    let eye = gpu.buffer(&eye_words(&c.light), wgpu::BufferUsages::UNIFORM);
    let head = gpu.buffer(
        &head_words(c.current, c.epoch, was),
        wgpu::BufferUsages::UNIFORM,
    );
    let list = gpu.buffer(&c.list, wgpu::BufferUsages::STORAGE);
    let args_buf = args.map(|a| gpu.buffer(a, wgpu::BufferUsages::STORAGE));
    let mut binds = vec![
        (0, eye.as_entire_binding()),
        (11, head.as_entire_binding()),
        (12, list.as_entire_binding()),
    ];
    if let Some(a) = &args_buf {
        binds.push((21, a.as_entire_binding()));
    }
    gpu.dispatch(
        p,
        &binds,
        [N.div_ceil(SQUARE_SIDE), N.div_ceil(SQUARE_SIDE), 1],
    );
    [
        gpu.read(&list),
        args_buf.map_or(Vec::new(), |a| gpu.read(&a)),
    ]
}

/// The lights the cases are cast under: the sun high, low over the box's corner, straight
/// down (no way up), and low along an axis, each reading the whole box or a disk within it.
fn lights() -> [(&'static str, Light); 4] {
    [
        (
            "high",
            box_of([0.3, 0.2, -0.93], 0.0625, 0.75, [1304, -896]),
        ),
        (
            "low",
            box_of([0.7, -0.55, -0.2], 0.0625, 1.0, [-2048, 4096]),
        ),
        (
            "straight down",
            box_of([0.0, 0.0, -1.0], 0.0625, 0.6, [0, 0]),
        ),
        (
            "low along x",
            box_of([-0.96, 0.0, -0.28], 0.125, 0.9, [8, -8]),
        ),
    ]
}

/// The ulps of a step's way up from a whole number of tiles within which the device's
/// division, `k` over the steps, may floor it to the other tile: WGSL's bound on a division,
/// 2.5 ulps, past the 13 measured on Metal.
const EDGE_ULPS: u32 = 16;

/// The steps of the way `up` whose offset along an axis lies within `EDGE_ULPS` of a whole
/// number of tiles other than none, which the device's division leaves none (`Light::up_at`).
fn steps_at_edge(up: ([f32; 2], u32)) -> u32 {
    let (way, steps) = up;
    (1..=steps)
        .filter(|&k| {
            let f = k as f32 / steps as f32;
            way.iter().any(|&w| {
                let v = w * f;
                v != 0.0 && ulps(v, v.round_ties_even()) <= EDGE_ULPS
            })
        })
        .count() as u32
}

#[test]
fn hand_propagation_claims_as_the_rust() {
    let Some(gpu) = Gpu::new() else {
        eprintln!("no adapter: skipped");
        return;
    };
    let p = gpu.pipeline(src(SHADER_LIGHT_PROPAGATE), "light_propagate_cs");
    let mut tally = Tally::default();
    let mut all = Told::default();
    let mut steps_most = 0;
    let mut at_edge = 0;
    for (n, (name, l)) in lights().into_iter().enumerate() {
        steps_most = steps_most.max(l.up::<Pixelet>().1);
        at_edge += steps_at_edge(l.up::<Pixelet>());
        let seed = n as u32 * 31 + 5;
        // The epoch begun, within it, within it with no tile left to claim, and with one,
        // the list it claims onto a whole number of its pass's rows full; and the cast's number
        // by the last a word holds, a later cast's mark past it.
        for (kind, current, epoch) in [
            ("begins its epoch", 40, 40),
            ("within its epoch", 40, 37),
            ("all held", 40, 3),
            ("one read anew", 40, 3),
            ("its number by the last", u32::MAX - 1, u32::MAX - 4),
        ] {
            let mut list = vec![0u32; light::list_words(SIZE) as usize];
            let mut args = args_start();
            let half = SIZE as f32 / 2.0 * libm::sqrtf(l.across.iter().map(|v| v * v).sum());
            stand(&mut list, &l, current, half * l.reach_ndc, seed);
            let middle = (N / 2 * N + N / 2) as usize;
            if epoch == 3 {
                let at = light::reads_at(SIZE) as usize;
                list[at..at + (N * N) as usize].fill(7);
                if kind == "one read anew" {
                    list[at + middle] = 0;
                }
            } else {
                read_before(&mut list, current, epoch, seed);
            }
            if current != epoch {
                mark_before(&mut list, &mut args, current, seed);
            }
            if kind == "one read anew" {
                rows_full(&mut list, current, middle);
            }
            let c = Case {
                name,
                light: l,
                current,
                epoch,
                list,
                args,
            };
            let (want, want_args, told) = propagate(&c);
            let [have, have_args] = run(&gpu, &p, &c, Some(&c.args), [0, 0]);
            let case = format!("{}, {kind}", c.name);
            eprintln!("{case}: {told:?}");
            tally.compare(&case, &sorted(want), &sorted(have), word_of);
            tally.compare(&format!("{case}, args"), &want_args, &have_args, |k| {
                format!("arg {k}")
            });
            match kind {
                "all held" => assert!(told.claimed == 0 && told.held > 0, "{case}: {told:?}"),
                "one read anew" => {
                    assert!(told.claimed == 1 && told.listed[2] == 1, "{case}: {told:?}");
                }
                _ => assert!(told.claimed > 0 && told.held > 0, "{case}: {told:?}"),
            }
            all.past += told.past;
            all.unread += told.unread;
            all.held += told.held;
            all.claimed += told.claimed;
            all.read_up += told.read_up;
            all.over_mark += told.over_mark;
            for k in 0..3 {
                all.listed[k] += told.listed[k];
            }
        }
    }
    eprintln!("all: {all:?}, the longest way up {steps_most} steps, {at_edge} at an edge");
    assert!(all.past > 0 && all.unread > 0, "{all:?}");
    assert!(all.listed.iter().all(|&k| k > 0), "{all:?}");
    assert!(all.read_up > 0 && steps_most > 1, "{all:?}");
    assert!(all.over_mark > 0, "{all:?}");
    // The device's division moves no step of these lights to another tile, so their words
    // compare exactly.
    assert_eq!(at_edge, 0, "steps within EDGE_ULPS of a whole tile");
    tally.end("propagation");
}

#[test]
fn hand_slide_keeps_as_the_rust() {
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
        let c = Case {
            name,
            light: l,
            current: 40,
            epoch: 37,
            list,
            args: Vec::new(),
        };
        let mut want = c.list.clone();
        let reads = light::reads_at(SIZE);
        let (mut case_kept, mut case_off) = (0, 0);
        for j in 0..N {
            for i in 0..N {
                if l.keeps(SIZE, RANGE, [i, j], was) {
                    case_kept += 1;
                    continue;
                }
                want[(reads + j * N + i) as usize] = 0;
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
        let [have, _] = run(&gpu, &p, &c, None, was);
        eprintln!("{name}: {case_kept} kept, {case_off} set to none");
        tally.compare(name, &want, &have, word_of);
        kept += case_kept;
        off += case_off;
    }
    eprintln!(
        "slides: {kept} tiles kept, {off} set to none, {banded} of them moved into another band"
    );
    assert!(kept > 0 && off > 0 && banded > 0);
    tally.end("slides");
}
