//! `light_cs` and `light_mark_cs` by hand (frame/shader/light.wgsl, light_mark.wgsl) answer as
//! pxl-engine's light over the real world of tests/real_world. A slot of a tile the cast casts
//! takes its visible sample where its own ray enters it as the sample did (`cast::entry_with`)
//! at the band it asks (`Light::band`), its word `light::word(light::depth(..))`; else the sky
//! where the four samples about it (`light::samples_about`) went on to their reach at its band;
//! else it is listed for its walk. That composition is this test's own reading of light.wgsl
//! `light_cs`, which pxl-engine has no one function for. The samples are each walked by
//! `cast::first` over the world, and each texel's visible sample is the nearest of those about
//! it its ray enters, as the raster keeps it, some put out of place. A drawn pixel whose ray met
//! the world marks the tile it met it on (`Light::tile_of`), and where the head asks the
//! footprint, the tiles between it and each later neighbor's (`light::footprint`). Every word of
//! the map, the list and the misses' args is compared. The light's reciprocals are the host's
//! words (`Light::inverse`, `Light::t_max_inverse`), and a ray's start is summed in fixed point
//! (`Starts::start`), read off a probe and held to the Rust's exactly; a slot may take what
//! a fused sum gives it, each counted: its depth's product fused into the push after it, a
//! band's distance squared an ulp off; a tile a pixel meets the world within `EDGE` of may be
//! marked or not. Skipped where no adapter answers.

mod gpu;
mod keeping;
mod real_world;

use std::collections::BTreeSet;

use gpu::{array_view, cross, read_texture, texture, unit, Gpu, Kinds, LightBox, Tally};
use keeping::{q8, Asks, Keeping};
use pxl_engine::beam;
use pxl_engine::cast::{self, Detail, End, Hit, Node, Ray, Spacing, Trace, Until};
use pxl_engine::light::{self, Light};
use pxl_engine::overwrite::Cut;
use pxl_engine::{count, eye, param, pyramid, raster, ray, Zoom};
use pxl_render::frame::shader::record::{src, SHADER_LIGHT, SHADER_LIGHT_MARK};
use pxl_render::frame::shader::word::SQUARE_SIDE;

use real_world::{Built, World, FINEST};

/// East of X = 0 by the seam, and by X = 0, where X's roots part.
const EYE: [i32; 2] = [(1 << 23) - 300, (40 << 16) + 30];
const EYE_ZERO: [i32; 2] = [150, (40 << 16) + 30];
const SIZE: u32 = 256;
const CAST: u32 = 7;
const FRAME: u32 = 9;
/// Tiles a tile's coordinate may lie off an edge between tiles for a GPU's rounding to put it
/// on either side.
const EDGE: f64 = 1e-4;

fn noise(k: u32) -> u32 {
    let h = k.wrapping_mul(0x9e37_79b9).rotate_left(13) ^ 0x5bd1_e995;
    h.wrapping_mul(0x85eb_ca77) ^ h >> 15
}

/// A light's box about `origin`: the direction, a texel's side in units, how far back along
/// the light its near plane lies, how far its rays reach, and its corner.
fn lit(
    origin: [i32; 3],
    dir: [f32; 3],
    side: f32,
    back: f32,
    t_max: f32,
    corner: [i32; 2],
) -> LightBox {
    LightBox {
        size: SIZE,
        origin,
        dir,
        side,
        back,
        t_max,
        corner,
        reach_ndc: 1.0,
    }
}

/// A cast of the map's slots over the world.
struct SlotCase {
    name: &'static str,
    lit: LightBox,
    range: [Zoom; 2],
    counted: bool,
    /// Where the cast is within its epoch, one tile in so many claimed; none where it begins it.
    listed: Option<u32>,
}

/// The node, the asset and the axis of the sample at word `k`.
fn cell_of(list: &[u32], k: usize) -> (Node, [u32; 2], u32) {
    let n = k + beam::SAMPLE_NODE as usize;
    let node = Node {
        zoom: list[n + beam::NODE_ZOOM as usize],
        corner: [0, 1, 2].map(|a| list[n + beam::NODE_CORNER as usize + a]),
        roots: [0, 1, 2].map(|a| list[n + beam::NODE_ROOTS as usize + a] as i32),
    };
    let asset = [
        list[k + beam::SAMPLE_ASSET as usize],
        list[k + beam::SAMPLE_ASSET as usize + 1],
    ];
    (node, asset, list[k + beam::SAMPLE_AXIS as usize])
}

#[test]
fn hand_light_cs_answers_as_the_rust() {
    let Some(gpu) = Gpu::new() else {
        eprintln!("no adapter: skipped");
        return;
    };
    let pipe = gpu.pipeline(src(SHADER_LIGHT), "light_cs");
    let built = real_world::build(EYE);
    let over = |b: &Built, g: [i32; 2], up: i32| q8([g[0], g[1], b.top_at(g) + up]);
    let mut tally = Tally::default();
    let mut kinds = Kinds::default();
    let cases = [
        SlotCase {
            name: "about the eye, the epoch begun, counted",
            lit: lit(
                over(&built, [EYE[0] + 90, EYE[1] + 90], 40),
                [0.3, 0.2, -0.93],
                1.0,
                60.0,
                400.0,
                [1300, -900],
            ),
            range: [8, FINEST],
            counted: true,
            listed: None,
        },
        SlotCase {
            name: "reaching about the ground, the sky between",
            lit: lit(
                over(&built, [EYE[0] + 40, EYE[1] + 150], 2),
                [-0.2, 0.35, -0.9],
                0.5,
                30.0,
                29.5,
                [-77, 4093],
            ),
            range: [8, FINEST],
            counted: true,
            listed: None,
        },
        SlotCase {
            name: "short of the ground, the sky about every band's edge",
            lit: lit(
                over(&built, [EYE[0] + 90, EYE[1] + 90], 40),
                [0.3, 0.2, -0.93],
                0.75,
                60.0,
                20.0,
                [1300, -900],
            ),
            range: [8, FINEST],
            counted: true,
            listed: None,
        },
        SlotCase {
            name: "tiles claimed within the epoch, not counted",
            lit: lit(
                over(&built, [EYE[0] + 90, EYE[1] + 90], 40),
                [0.5, -0.1, -0.86],
                1.0,
                60.0,
                400.0,
                [1296, -904],
            ),
            range: [10, FINEST],
            counted: false,
            listed: Some(3),
        },
    ];
    for c in &cases {
        run_slots(&gpu, &pipe, built.world(), c, &mut tally, &mut kinds);
    }
    let zero = real_world::build(EYE_ZERO);
    let c = SlotCase {
        name: "over X = 0, the roots parting",
        lit: lit(
            q8([
                10,
                EYE_ZERO[1] + 90,
                zero.top_at([10, EYE_ZERO[1] + 90]) + 40,
            ]),
            [0.3, 0.2, -0.93],
            1.0,
            60.0,
            400.0,
            [1300, -900],
        ),
        range: [8, FINEST],
        counted: true,
        listed: Some(2),
    };
    run_slots(&gpu, &pipe, zero.world(), &c, &mut tally, &mut kinds);
    kinds.hold(&[
        "slots taken",
        "took",
        "sky",
        "walked",
        "walked, seeing none",
        "walked, its visible lacks",
        "walked, its visible not entered",
        "walked, its visible another band's",
        "walked, a sample about it another band's",
        "not needed",
        "past the list",
        "samples gone on",
        "samples stopped",
    ]);
    tally.end("light_cs");
}

fn run_slots(
    gpu: &Gpu,
    pipe: &wgpu::ComputePipeline,
    w: World,
    c: &SlotCase,
    tally: &mut Tally,
    kinds: &mut Kinds,
) {
    let (l, starts) = c.lit.lay();
    let inv = l.inverse();
    starts_alike(gpu, &l, &starts);
    let d = param::light::SAMPLE;
    let n = SIZE / d;
    let tiles = light::tiles(SIZE);
    let band_of = |p: [u32; 2]| {
        let tile = [(p[0] / light::TILE) as i32, (p[1] / light::TILE) as i32];
        l.band(SIZE, c.range, tile)
    };
    let texel_slot =
        |p: [u32; 2]| [0, 1].map(|k| (p[k] as i32).wrapping_add(l.corner[k]) as u32 & (SIZE - 1));

    // Every word the pass may not write marked; the samples walked over the world.
    let mut list: Vec<u32> = (0..light::list_words(SIZE)).map(noise).collect();
    let mut table = vec![0u32; pyramid::WORDS as usize];
    table[pyramid::FRAME as usize] = FRAME;
    let mut q = Keeping {
        w,
        cuts: [Cut::NONE; 2],
        table: &mut table,
        tag: pyramid::tag(FRAME, pyramid::LIGHT_SAMPLES_PASS),
        asks: Asks::default(),
        met: None,
        writes: true,
    };
    for j in 0..n {
        for i in 0..n {
            let p = l.texel(SIZE, [i * d, j * d]);
            let (o, r) = light::ray(&l, &starts, p);
            let b = band_of(p);
            let detail = Detail {
                range: [b, b],
                spacing: l.spacing(),
            };
            let met = cast::first(&mut q, o, &r, detail, Until::Opaque);
            let k = light::sample(SIZE, j * n + i) as usize;
            let node = k + beam::SAMPLE_NODE as usize;
            for a in 0..3 {
                list[node + beam::NODE_CORNER as usize + a] = met.hit.node.corner[a];
                list[node + beam::NODE_ROOTS as usize + a] = met.hit.node.roots[a] as u32;
            }
            let gone = met.ends == End::Miss;
            list[node + beam::NODE_ZOOM as usize] = if gone {
                beam::SAMPLE_NONE
            } else {
                met.hit.node.zoom
            };
            list[k + beam::SAMPLE_ASSET as usize] = met.hit.asset[0];
            list[k + beam::SAMPLE_ASSET as usize + 1] = met.hit.asset[1];
            list[k + beam::SAMPLE_LACKS as usize] = u32::from(met.ends == End::Lacks);
            list[k + beam::SAMPLE_AXIS as usize] = met.hit.axis;
            kinds.add("samples gone on", gone);
            kinds.add("samples stopped", !gone);
        }
    }
    let stopped: Vec<u32> = (0..n * n)
        .filter(|&s| {
            let k = light::sample(SIZE, s) + beam::SAMPLE_NODE + beam::NODE_ZOOM;
            list[k as usize] != beam::SAMPLE_NONE
        })
        .collect();

    // Each texel's visible sample, as the raster keeps it: the nearest of those about it whose
    // box its ray enters across the axis the sample entered by. Some none, some another's that
    // stopped, some marked lacking.
    let mut vis = vec![0u32; (SIZE * SIZE) as usize];
    for y in 0..SIZE {
        for x in 0..SIZE {
            let p = [x, y];
            let slot = texel_slot(p);
            let (o, r) = light::ray(&l, &starts, p);
            let s0 = [slot[0] / d, slot[1] / d];
            let mut best: Option<(f32, u32)> = None;
            for dj in -1..=2i32 {
                for di in -1..=2i32 {
                    let s = [
                        (s0[0] as i32 + di).rem_euclid(n as i32) as u32,
                        (s0[1] as i32 + dj).rem_euclid(n as i32) as u32,
                    ];
                    let idx = s[1] * n + s[0];
                    let k = light::sample(SIZE, idx) as usize;
                    if list[k + beam::SAMPLE_NODE as usize + beam::NODE_ZOOM as usize]
                        == beam::SAMPLE_NONE
                    {
                        continue;
                    }
                    let (node, asset, axis) = cell_of(&list, k);
                    let (t, e, enters) = cast::entry(o, &r, &node, asset[0]);
                    if enters && e == axis && best.is_none_or(|b| t < b.0) {
                        let lacks = list[k + beam::SAMPLE_LACKS as usize] != 0;
                        best = Some((t, raster::id_of(idx, lacks)));
                    }
                }
            }
            let at = y * SIZE + x;
            let h = noise(at ^ 0x1234_5678);
            let mut v = best.map_or(0, |b| b.1);
            match h % 29 {
                0 | 1 => v = 0,
                2 | 3 if !stopped.is_empty() => {
                    v = raster::id_of(stopped[(h >> 8) as usize % stopped.len()], false);
                }
                4 if v != 0 => v |= raster::LACK,
                _ => {}
            }
            vis[at as usize] = v;
        }
    }

    // The tiles the cast casts: all but some where it begins its epoch, else some of those it
    // claimed, each listed once, the room past them an unlisted tile's.
    let reads_at = light::reads_at(SIZE) as usize;
    let mut threads: Vec<Option<[u32; 2]>> = Vec::new();
    let groups = if let Some(every) = c.listed {
        let mut entries = Vec::new();
        for j in 0..tiles {
            for i in 0..tiles {
                let claimed = (i + 2 * j).is_multiple_of(every) || i == 0 || j == tiles - 1;
                if claimed {
                    entries.push(j << 16 | i);
                }
                let read = claimed && !(i + j).is_multiple_of(7);
                list[reads_at + (j * tiles + i) as usize] = if read { CAST } else { CAST - 1 };
            }
        }
        entries.reverse();
        let at = light::listed_at(SIZE, light::LISTED_CLAIMED) as usize;
        list[at] = entries.len() as u32;
        list[at + 1..at + 1 + entries.len()].copy_from_slice(&entries);
        let room = light::listed_room(SIZE, light::LISTED_CLAIMED) as usize;
        let unlisted = (0..tiles * tiles)
            .map(|k| ((k / tiles) << 16) | (k % tiles))
            .find(|e| !entries.contains(e))
            .expect("a tile not claimed");
        list[at + 1 + entries.len()..at + 1 + room].fill(unlisted);
        // Read, so a thread that took the room past the list would cast it.
        let u = (unlisted >> 16) * tiles + (unlisted & 0xffff);
        list[reads_at + u as usize] = CAST;
        let rows = (entries.len() as u32).div_ceil(light::LIST_ACROSS);
        for gy in 0..rows {
            for gx in 0..light::LIST_ACROSS {
                let e = (gy * light::LIST_ACROSS + gx) as usize;
                for k in 0..light::TILE * light::TILE {
                    threads.push(entries.get(e).map(|&t| {
                        let p = texel_slot([(t & 0xffff) * light::TILE, (t >> 16) * light::TILE]);
                        [p[0] + k % light::TILE, p[1] + k / light::TILE]
                    }));
                }
            }
        }
        [light::LIST_ACROSS, rows, 1]
    } else {
        for t in 0..tiles * tiles {
            let read = !noise(t ^ 0x77).is_multiple_of(5);
            list[reads_at + t as usize] = if read { CAST } else { CAST + 1 };
        }
        threads.extend((0..SIZE * SIZE).map(|k| Some([k % SIZE, k / SIZE])));
        [tiles, tiles, 1]
    };

    let mut eye_words = vec![0u32; eye::WORDS];
    eye_words[eye::LIGHT..eye::LIGHT + eye::LIGHT_WORDS].copy_from_slice(&l.words());
    eye_words[eye::RANGE] = c.range[0];
    eye_words[eye::RANGE + 1] = c.range[1];
    let mut head = vec![0u32; light::HEAD_WORDS];
    head[light::HEAD_CAST] = CAST;
    head[light::HEAD_COUNTED] = u32::from(c.counted);
    head[light::HEAD_EPOCH] = if c.listed.is_some() { CAST - 3 } else { CAST };
    for (k, v) in light::head_layout(SIZE) {
        head[k] = v;
    }
    head[light::STARTS_AT * 4..(light::STARTS_AT + light::STARTS_VEC4S) * 4]
        .copy_from_slice(&starts.words());
    let map0: Vec<u32> = (0..SIZE * SIZE)
        .map(|k| noise(k ^ 0xabc) | 1 << 31)
        .collect();

    // The slots the threads take and the cast needs.
    let mut needed = Vec::new();
    for slot in &threads {
        let Some(slot) = *slot else {
            kinds.add("past the list", true);
            continue;
        };
        if slot[0] >= SIZE || slot[1] >= SIZE {
            continue;
        }
        kinds.add("slots taken", true);
        let p = l.texel(SIZE, slot);
        let tile = (p[1] / light::TILE) * tiles + p[0] / light::TILE;
        if list[reads_at + tile as usize] == CAST {
            needed.push(slot);
        } else {
            kinds.add("not needed", true);
        }
    }

    // Each needed slot's answer from its ray's start `o`, every band's distance squared `k`
    // ulps off (`band_by`).
    let gone_on = |s: [u32; 2]| {
        let k = light::sample(SIZE, (s[1] % n) * n + s[0] % n) + beam::SAMPLE_NODE;
        list[(k + beam::NODE_ZOOM) as usize] == beam::SAMPLE_NONE
    };
    let answer = |slot: [u32; 2], o: [i32; 3], k: i32, inv: [f32; 3], kinds: &mut Kinds| {
        let band_of = |p: [u32; 2]| band_by(&l, c.range, p, k);
        let sample_band = |s: [u32; 2]| band_of(l.texel(SIZE, [s[0] * d, s[1] * d]));
        let p = l.texel(SIZE, slot);
        let r = Ray::new(l.dir, l.t_max);
        let band = band_of(p);
        let visible = vis[(p[1] * SIZE + p[0]) as usize];
        if visible != 0 && visible & raster::LACK == 0 {
            let s = raster::sample_of(visible);
            let (node, asset, axis) = cell_of(&list, light::sample(SIZE, s) as usize);
            let e = cast::entry_with(o, &r, inv, &node, asset[0]);
            let alike = sample_band([s % n, s / n]) == band;
            if e.enters && e.axis == axis && alike {
                kinds.add("took", true);
                return Answer::Entry(e.t, e.axis);
            }
            kinds.add(
                "walked, its visible not entered",
                !(e.enters && e.axis == axis),
            );
            kinds.add(
                "walked, its visible another band's",
                e.enters && e.axis == axis && !alike,
            );
        }
        kinds.add("walked, its visible lacks", visible & raster::LACK != 0);
        if visible == 0 {
            let four = light::samples_about(SIZE, slot, p);
            let gone = four.is_some_and(|f| f.iter().all(|&s| gone_on(s)));
            if gone && four.is_some_and(|f| f.iter().all(|&s| sample_band(s) == band)) {
                kinds.add("sky", true);
                return Answer::Sky;
            }
            kinds.add("walked, a sample about it another band's", gone);
            kinds.add("walked, seeing none", true);
        }
        kinds.add("walked", true);
        Answer::Walk
    };
    let wants: Vec<Answer> = needed
        .iter()
        .map(|&slot| {
            let (o, _) = light::ray(&l, &starts, l.texel(SIZE, slot));
            answer(slot, o, 0, inv, kinds)
        })
        .collect();
    // The pass over fresh words, the list's count starting at `start`: the map, the list and
    // the misses' args it leaves, and the slots it listed in their order.
    let cast = |start: u32| {
        let vis_t = texture(
            &gpu.device,
            &gpu.queue,
            [SIZE, SIZE],
            1,
            wgpu::TextureUsages::TEXTURE_BINDING,
            &vis,
        );
        let map_t = texture(
            &gpu.device,
            &gpu.queue,
            [SIZE, SIZE],
            1,
            wgpu::TextureUsages::STORAGE_BINDING,
            &map0,
        );
        let (vis_v, map_v) = (array_view(&vis_t), array_view(&map_t));
        let eye_buf = gpu.buffer(&eye_words, wgpu::BufferUsages::UNIFORM);
        let head_buf = gpu.buffer(&head, wgpu::BufferUsages::UNIFORM);
        let list_buf = gpu.buffer(&list, wgpu::BufferUsages::STORAGE);
        let mut misses0 = ray::MISS_ARGS_EMPTY.to_vec();
        misses0[ray::MISS_ARGS_CNT as usize] = start;
        let misses_buf = gpu.buffer(&misses0, wgpu::BufferUsages::STORAGE);
        gpu.dispatch(
            pipe,
            &[
                (0, eye_buf.as_entire_binding()),
                (8, list_buf.as_entire_binding()),
                (10, wgpu::BindingResource::TextureView(&map_v)),
                (11, head_buf.as_entire_binding()),
                (15, wgpu::BindingResource::TextureView(&vis_v)),
                (17, misses_buf.as_entire_binding()),
            ],
            groups,
        );
        let list_out = gpu.read(&list_buf);
        let misses = gpu.read(&misses_buf);
        let from = light::misses_at(SIZE) as usize;
        let to = from + (misses[ray::MISS_ARGS_CNT as usize] as usize).min((SIZE * SIZE) as usize);
        let had = list_out[(from + start as usize).min(to)..to].to_vec();
        (
            read_texture(&gpu.device, &gpu.queue, &map_t),
            list_out,
            misses,
            had,
        )
    };
    let (map, list_out, misses, had) = cast(0);
    let first = light::misses_at(SIZE) as usize;
    let had_set: BTreeSet<u32> = had.iter().copied().collect();

    // What the pass left of a slot, and whether it is answer `a`: its word for a depth within
    // the rounding a GPU gives it (`depth_words`).
    let got = |at: usize, slot: [u32; 2]| -> Answer {
        if had_set.contains(&(slot[1] << 16 | slot[0])) && map[at] == map0[at] {
            Answer::Walk
        } else if map[at] == light::word(1.0) {
            Answer::Sky
        } else {
            Answer::Word(map[at])
        }
    };
    let words_of = |t: f32, axis: u32| depth_words(&l, t, axis);
    let matches = |a: Answer, g: Answer| match (a, g) {
        (Answer::Entry(t, e), Answer::Word(w)) => words_of(t, e).1.contains(&w),
        (Answer::Entry(t, e), Answer::Sky) => words_of(t, e).1.contains(&light::word(1.0)),
        (a, g) => a == g,
    };

    let mut want_map = map0.clone();
    let mut walked = BTreeSet::new();
    let (mut took, mut sky) = (0u32, 0u32);
    let (mut exact, mut fused, mut banded) = (0u32, 0u32, 0u32);
    let mut nothing = Kinds::default();
    for (&slot, &want) in needed.iter().zip(&wants) {
        let p = l.texel(SIZE, slot);
        let (o, _) = light::ray(&l, &starts, p);
        let at = (slot[1] * SIZE + slot[0]) as usize;
        let g = got(at, slot);
        // The answer the slot is held to: its own, or where a GPU's fused sum in its depth or
        // its band may give another, the one the hand gave.
        let held = if matches(want, g) {
            match want {
                Answer::Entry(t, e) => {
                    let (w, _) = words_of(t, e);
                    if g == Answer::Word(w) || (w == light::word(1.0) && g == Answer::Sky) {
                        exact += 1;
                    } else {
                        fused += 1;
                    }
                }
                _ => exact += 1,
            }
            g
        } else if [-BAND_ULPS, BAND_ULPS]
            .iter()
            .any(|&k| matches(answer(slot, o, k, inv, &mut nothing), g))
        {
            banded += 1;
            g
        } else {
            match want {
                Answer::Entry(t, e) => Answer::Word(words_of(t, e).0),
                a => a,
            }
        };
        match held {
            Answer::Word(w) => {
                want_map[at] = w;
                took += 1;
            }
            Answer::Sky => {
                want_map[at] = light::word(1.0);
                if matches!(want, Answer::Entry(..)) {
                    took += 1;
                } else {
                    sky += 1;
                }
            }
            _ => {
                walked.insert(slot[1] << 16 | slot[0]);
            }
        }
    }

    let name = c.name;
    tally.compare(&format!("{name}: the map"), &want_map, &map, |k| {
        format!("slot {:?}", [k as u32 % SIZE, k as u32 / SIZE])
    });
    // The listed slots in the order the atomics took them: as a set, each once, then every
    // word of the list beside them as it was.
    let listed_alike = had.len() == had_set.len() && had_set == walked;
    tally.compare(
        &format!("{name}: the slots listed, as a set"),
        &[u32::from(true)],
        &[u32::from(listed_alike)],
        |_| format!("{} listed, {} to walk", had.len(), walked.len()),
    );
    let mut want_list = list.clone();
    want_list[first..first + had.len()].copy_from_slice(&had);
    tally.compare(&format!("{name}: the list"), &want_list, &list_out, |k| {
        format!("word {k}")
    });
    let walked_n = walked.len() as u32;
    let per = ray::MISS_LANES * ray::MISS_ACROSS;
    let args = |start: u32| {
        let mut want = ray::MISS_ARGS_EMPTY.to_vec();
        want[ray::MISS_ARGS_CNT as usize] = start + walked_n;
        if let Some(m) = (start..start + walked_n)
            .rev()
            .find(|p| p.is_multiple_of(per))
        {
            want[ray::MISS_ARGS_LIST as usize + 1] = m / per + 1;
        }
        if c.counted {
            let lane = |k: u32| (ray::MISS_ARGS_COUNT + k - count::REUSED) as usize;
            want[lane(count::REUSED)] = took;
            want[lane(count::SKY)] = sky;
            want[lane(count::WALKED)] = walked_n;
        }
        want
    };
    let want_misses = args(0);
    tally.compare(
        &format!("{name}: the misses' args"),
        &want_misses,
        &misses,
        |k| format!("word {k}"),
    );
    // Again, the count starting past a real frame's so the last slot listed is the first of a
    // row of the list's dispatch, which opens it.
    if walked_n > 0 && c.listed.is_none() {
        let start = (per - (walked_n - 1) % per) % per;
        let (_, _, misses, had) = cast(start);
        let set: BTreeSet<u32> = had.iter().copied().collect();
        tally.compare(
            &format!("{name}: listed from {start}"),
            &[u32::from(true)],
            &[u32::from(had.len() == set.len() && set == walked)],
            |_| format!("{} listed, {} to walk", had.len(), walked.len()),
        );
        tally.compare(
            &format!("{name}: the misses' args, listed from {start}"),
            &args(start),
            &misses,
            |k| format!("word {k}"),
        );
    }
    eprintln!(
        "{name}: {} needed: {took} took, {sky} sky, {walked_n} walked; {exact} exact, {fused} \
         a depth a product fused into a sum, {banded} a band's distance as a GPU rounds it",
        needed.len()
    );
    kinds.count("slots a depth a product fused into a sum", u64::from(fused));
    assert!(banded <= BANDS_OVER, "{name}: {banded} slots a band over");
}

/// A slot's answer: the depth `t` along its ray and the axis it enters its visible sample's
/// cell across, the sky, or its walk; and the word a pass left it.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Answer {
    Entry(f32, u32),
    Sky,
    Walk,
    Word(u32),
}

/// Ulps a band's distance squared is taken off either way: its sum of squares may be fused
/// (light_aim.wgsl `tile_band`).
const BAND_ULPS: i32 = 1;

/// The band of box texel `p`'s tile (`Light::band`), its distance squared `k` ulps off.
fn band_by(l: &Light, range: [Zoom; 2], p: [u32; 2], k: i32) -> Zoom {
    let tile = [(p[0] / light::TILE) as i32, (p[1] / light::TILE) as i32];
    let held = Detail {
        range,
        spacing: Spacing::NONE,
    };
    let r2 = l.band_dist2(SIZE, tile);
    held.at(f32::from_bits(r2.to_bits().wrapping_add_signed(k)))
}

/// Slots a case may hold to a band's distance squared an ulp off (`run_slots`), at most.
const BANDS_OVER: u32 = 16;

/// The word of a slot that took its visible sample `t` along its ray across `axis`
/// (`light::depth`, `light::word`), and every word a GPU may leave for it: its share of the
/// box's depth, `t` times the reach's reciprocal, rounded or fused into the push added, and
/// the product with `DEPTH_ONE` rounded or fused into the half added. Nothing is divided.
fn depth_words(l: &Light, t: f32, axis: u32) -> (u32, Vec<u32>) {
    let met = Trace {
        ends: End::Hit,
        hit: Hit {
            t,
            axis,
            node: Node::ROOT,
            at: (),
            asset: [0; 2],
        },
        visits: 0,
    };
    let want = light::word(light::depth(l, &met));
    let (scale, push) = (l.t_max_inverse(), pxl_engine::axis(l.bias, axis));
    let one = light::DEPTH_ONE as f32;
    let mut all = Vec::new();
    for d in [t * scale + push, t.mul_add(scale, push)] {
        let d = d.min(1.0);
        for v in [d * one + 0.5, d.mul_add(one, 0.5)] {
            all.push((v as u32).min(light::DEPTH_ONE));
        }
    }
    (want, all)
}

/// An eye over the world and the screen it draws, and the light's box about it.
struct Look {
    name: &'static str,
    origin: [i32; 3],
    look: [f32; 3],
    tan: [f32; 2],
    far: f32,
    size: [u32; 2],
    jitter: [f32; 2],
    footprint: bool,
    lit: LightBox,
}

/// The tiles a point `v` units from the eye may stand on: one, or either side of an edge it
/// lies within `EDGE` of.
fn tiles_about(l: &Light, v: [f32; 3]) -> Vec<[i32; 2]> {
    let want = l.tile_of(v);
    let d: [f64; 3] = [0, 1, 2].map(|k| f64::from(v[k]) - f64::from(l.first[k]));
    let dot = |a: [f64; 3], b: [f32; 3]| (0..3).map(|k| a[k] * f64::from(b[k])).sum::<f64>();
    let sq = |b: [f32; 3]| dot(b.map(f64::from), b);
    let at = [dot(d, l.across) / sq(l.across), dot(d, l.down) / sq(l.down)];
    let x = at.map(|a| (a + 0.5) / f64::from(light::TILE));
    let side = |k: usize| {
        let lo = (x[k] - EDGE).floor() as i32;
        let hi = (x[k] + EDGE).floor() as i32;
        if lo == hi {
            vec![want[k]]
        } else {
            vec![lo, hi]
        }
    };
    let (a, b) = (side(0), side(1));
    a.iter()
        .flat_map(|&i| b.iter().map(move |&j| [i, j]))
        .collect()
}

/// A pair of neighbors that marked a tile: its step, across, down and the two diagonals;
/// whether the later pixel lies on row 0; and whether it spans the footprint whole.
#[derive(Clone, Copy)]
struct Pair {
    step: usize,
    row_0: bool,
    whole: bool,
}

/// The tiles between `a` and `b` (`light::footprint`) put in `out`, and whether there are any.
fn between(a: [i32; 2], b: [i32; 2], out: &mut BTreeSet<[i32; 2]>) -> bool {
    let Some([lo, hi]) = light::footprint(a, b) else {
        return false;
    };
    for j in lo[1]..=hi[1] {
        for i in lo[0]..=hi[0] {
            out.insert([i, j]);
        }
    }
    true
}

#[test]
fn hand_light_mark_cs_answers_as_the_rust() {
    let Some(gpu) = Gpu::new() else {
        eprintln!("no adapter: skipped");
        return;
    };
    let pipe = gpu.pipeline(src(SHADER_LIGHT_MARK), "light_mark_cs");
    let built = real_world::build(EYE);
    let over = |g: [i32; 2], up: i32| q8([g[0], g[1], built.top_at(g) + up]);
    let lit = |dir: [f32; 3], side: f32| lit([0; 3], dir, side, 60.0, 400.0, [1300, -900]);
    let looks = [
        Look {
            name: "over the ground, the footprint",
            origin: over([EYE[0] + 90, EYE[1] + 90], 60),
            look: [0.5, 0.4, -0.77],
            tan: [0.3, 0.2],
            far: 2000.0,
            size: [96, 61],
            jitter: [0.0, 0.0],
            footprint: true,
            lit: lit([0.3, 0.2, -0.93], 1.0),
        },
        Look {
            name: "over the ground, jittered, its own tiles alone",
            origin: over([EYE[0] + 90, EYE[1] + 90], 60),
            look: [0.5, 0.4, -0.77],
            tan: [0.3, 0.2],
            far: 2000.0,
            size: [131, 77],
            jitter: [0.25, -0.375],
            footprint: false,
            lit: lit([0.3, 0.2, -0.93], 0.5),
        },
        Look {
            name: "looking far north, a box half as wide, the footprint, jittered",
            origin: over([EYE[0] + 90, EYE[1] + 90], 60),
            look: [0.1, 0.95, -0.3],
            tan: [0.6, 0.4],
            far: 900.0,
            size: [123, 70],
            jitter: [-0.125, 0.4375],
            footprint: true,
            lit: lit([-0.4, 0.1, -0.9], 0.5),
        },
        Look {
            name: "a coarse screen, wide, the footprint",
            origin: over([EYE[0] + 90, EYE[1] + 90], 60),
            look: [0.1, 0.95, -0.6],
            tan: [0.9, 0.6],
            far: 900.0,
            size: [14, 9],
            jitter: [0.375, -0.25],
            footprint: true,
            lit: lit([-0.4, 0.1, -0.9], 0.5),
        },
        Look {
            name: "a coarse screen turned across the box, the footprint",
            origin: over([EYE[0] + 90, EYE[1] + 90], 60),
            look: [0.7, 0.6, -0.6],
            tan: [0.9, 0.6],
            far: 900.0,
            size: [13, 10],
            jitter: [-0.25, 0.125],
            footprint: true,
            lit: lit([-0.4, 0.1, -0.9], 0.5),
        },
        Look {
            name: "two rows, the footprint, turned across the box",
            origin: over([EYE[0] + 90, EYE[1] + 90], 60),
            look: [0.7, 0.6, -0.6],
            tan: [0.9, 0.6],
            far: 900.0,
            size: [16, 2],
            jitter: [0.0, 0.0],
            footprint: true,
            lit: lit([-0.4, 0.1, -0.9], 0.5),
        },
        Look {
            name: "two rows, the footprint, looking down",
            origin: over([EYE[0] + 90, EYE[1] + 90], 60),
            look: [0.6, 0.5, -0.9],
            tan: [0.9, 0.6],
            far: 900.0,
            size: [15, 2],
            jitter: [0.0, 0.0],
            footprint: true,
            lit: lit([-0.4, 0.1, -0.9], 0.5),
        },
        Look {
            name: "two rows, the footprint, looking north",
            origin: over([EYE[0] + 90, EYE[1] + 90], 60),
            look: [0.1, 0.95, -0.6],
            tan: [0.9, 0.6],
            far: 900.0,
            size: [17, 2],
            jitter: [0.0, 0.0],
            footprint: true,
            lit: lit([-0.4, 0.1, -0.9], 0.5),
        },
        Look {
            name: "a coarse screen looking down, the footprint",
            origin: over([EYE[0] + 90, EYE[1] + 90], 60),
            look: [0.6, 0.5, -0.9],
            tan: [0.9, 0.6],
            far: 900.0,
            size: [12, 9],
            jitter: [0.125, 0.375],
            footprint: true,
            lit: lit([-0.4, 0.1, -0.9], 0.5),
        },
    ];
    let mut tally = Tally::default();
    let mut kinds = Kinds::default();
    for c in &looks {
        run_marks(&gpu, &pipe, built.world(), c, &mut tally, &mut kinds);
    }
    kinds.hold(&[
        "pixels met",
        "pixels met nothing",
        "pixels marked, not yet walked",
        "neighbors past the footprint",
        "tiles stood on by pairs alone",
        "tiles stood on by pairs across alone",
        "tiles stood on by pairs down alone",
        "tiles stood on by pairs down a diagonal alone",
        "tiles stood on by pairs up a diagonal alone",
        "tiles stood on by pairs up to row 0 alone",
        "tiles stood on by pairs a footprint apart alone",
        "tiles a pair a tile past the footprint lies between, not stood on",
        "points past the box",
    ]);
    tally.end("light_mark_cs");
}

fn run_marks(
    gpu: &Gpu,
    pipe: &wgpu::ComputePipeline,
    w: World,
    c: &Look,
    tally: &mut Tally,
    kinds: &mut Kinds,
) {
    let (l, _) = c.lit.lay();
    let forward = unit(c.look);
    let right = unit(cross(forward, [0.0, 0.0, 1.0]));
    let up = cross(right, forward);
    let range = [8, FINEST];
    let [wd, ht] = c.size;
    let texels = (wd * ht) as usize;

    // Each pixel's ray walked over the world: its first word and its t, as the cast leaves them;
    // some marked and not yet walked, some meeting nothing.
    let mut words: Vec<u32> = (0..ray::WORDS * wd * ht).map(noise).collect();
    let mut table = vec![0u32; pyramid::WORDS as usize];
    table[pyramid::FRAME as usize] = FRAME;
    let mut q = Keeping {
        w,
        cuts: [Cut::NONE; 2],
        table: &mut table,
        tag: pyramid::tag(FRAME, pyramid::MISSED_PASS),
        asks: Asks::default(),
        met: None,
        writes: true,
    };
    let detail = Detail {
        range,
        spacing: Spacing {
            angle: 0.002,
            across: 0.0,
        },
    };
    let tan_of = |p: [u32; 2]| {
        let at = [0, 1].map(|k| (p[k] as f32 + 0.5 + c.jitter[k]) / c.size[k] as f32 * 2.0);
        [(at[0] - 1.0) * c.tan[0], (1.0 - at[1]) * c.tan[1]]
    };
    let mut met: Vec<Option<Vec<[i32; 2]>>> = vec![None; texels];
    for y in 0..ht {
        for x in 0..wd {
            let k = (y * wd + x) as usize;
            let t = tan_of([x, y]);
            let dir = cast::through(forward, right, up, t[0], t[1]);
            let r = Ray::new(dir, c.far);
            let m = cast::first(&mut q, c.origin, &r, detail, Until::Opaque);
            let h = noise(k as u32 ^ 0x4242);
            let (first, at) = if h.is_multiple_of(13) {
                kinds.add("pixels marked, not yet walked", true);
                (ray::MISS, 0)
            } else if m.ends == End::Hit && !h.is_multiple_of(17) {
                let face = ray::face(r.dir, m.hit.axis);
                (
                    ray::first_word(m.hit.asset[0], face, m.hit.node.zoom),
                    m.hit.t.to_bits(),
                )
            } else {
                kinds.add("pixels met nothing", true);
                (0, 0)
            };
            words[k] = first;
            words[texels + k] = at;
            if first != 0 && first != ray::MISS {
                let v = dir.map(|a| a * m.hit.t);
                let about = tiles_about(&l, v);
                let n = light::tiles(SIZE) as i32;
                kinds.add(
                    "points past the box",
                    !about
                        .iter()
                        .all(|t| t[0] >= 0 && t[1] >= 0 && t[0] < n && t[1] < n),
                );
                kinds.add("pixels met", true);
                kinds.add("pixels by a tile's edge", about.len() > 1);
                met[k] = Some(about);
            }
        }
    }

    // The tiles every reading marks, and those some reading by a tile's edge does; and what
    // marked each that every reading marks: a pixel's own, or a pair of neighbors, by its step,
    // whether it is a pair over row 0 and whether it spans the whole footprint.
    let mut must = BTreeSet::new();
    let mut may = BTreeSet::new();
    let mut by: std::collections::BTreeMap<[i32; 2], Vec<Option<Pair>>> = Default::default();
    // The tiles a pair a tile past the footprint lies between.
    let mut past = BTreeSet::new();
    for y in 0..ht {
        for x in 0..wd {
            let Some(own) = &met[(y * wd + x) as usize] else {
                continue;
            };
            if own.len() == 1 {
                must.insert(own[0]);
                by.entry(own[0]).or_default().push(None);
            }
            may.extend(own.iter().copied());
            if !c.footprint {
                continue;
            }
            for (step, (dx, dy)) in [(1, 0), (0, 1), (1, 1), (1, -1)].into_iter().enumerate() {
                let (qx, qy) = (x as i32 + dx, y as i32 + dy);
                if qy < 0 || qx >= wd as i32 || qy >= ht as i32 {
                    continue;
                }
                let Some(other) = &met[(qy as u32 * wd + qx as u32) as usize] else {
                    continue;
                };
                if own.len() == 1 && other.len() == 1 {
                    let (a, b) = (own[0], other[0]);
                    let span = (a[0] - b[0]).abs().max((a[1] - b[1]).abs());
                    let mut sure = BTreeSet::new();
                    let stood = between(a, b, &mut sure);
                    kinds.add("neighbors past the footprint", !stood && a != b);
                    if span == light::FOOTPRINT_TILES + 1 {
                        let lo = [a[0].min(b[0]), a[1].min(b[1])];
                        let hi = [a[0].max(b[0]), a[1].max(b[1])];
                        for j in lo[1]..=hi[1] {
                            for i in lo[0]..=hi[0] {
                                past.insert([i, j]);
                            }
                        }
                    }
                    let label = Pair {
                        step,
                        row_0: qy == 0 && dy < 0,
                        whole: span == light::FOOTPRINT_TILES,
                    };
                    for t in &sure {
                        by.entry(*t).or_default().push(Some(label));
                    }
                    must.extend(sure);
                }
                for &a in own {
                    for &b in other {
                        between(a, b, &mut may);
                    }
                }
            }
        }
    }
    let mut head = vec![0u32; light::HEAD_WORDS];
    head[light::HEAD_CAST] = CAST;
    head[light::HEAD_FOOTPRINT] = u32::from(c.footprint);
    for (k, v) in light::head_layout(SIZE) {
        head[k] = v;
    }
    let mut eye_words = vec![0u32; eye::WORDS];
    for k in 0..3 {
        eye_words[eye::FORWARD + k] = forward[k].to_bits();
        eye_words[eye::RIGHT + k] = right[k].to_bits();
        eye_words[eye::UP + k] = up[k].to_bits();
    }
    eye_words[eye::TAN] = c.tan[0].to_bits();
    eye_words[eye::TAN + 1] = c.tan[1].to_bits();
    eye_words[eye::JITTER] = c.jitter[0].to_bits();
    eye_words[eye::JITTER + 1] = c.jitter[1].to_bits();
    eye_words[eye::RANGE] = range[0];
    eye_words[eye::RANGE + 1] = range[1];
    eye_words[eye::LIGHT..eye::LIGHT + eye::LIGHT_WORDS].copy_from_slice(&l.words());

    // The stands as an earlier cast left them, some this cast's number already; the rest of the
    // list words no thread may write.
    let n = light::tiles(SIZE);
    let stands = light::stands_at(SIZE) as usize;
    let mut list: Vec<u32> = (0..light::list_words(SIZE))
        .map(|k| noise(k ^ 0x99))
        .collect();
    for t in 0..(n * n) as usize {
        list[stands + t] = match noise(t as u32) % 7 {
            0 => CAST,
            1 => 0,
            2 => CAST + 1,
            _ => CAST - 1,
        };
    }
    let inside = |t: &[i32; 2]| t[0] >= 0 && t[1] >= 0 && t[0] < n as i32 && t[1] < n as i32;
    let word_of = |t: &[i32; 2]| stands + (t[1] as u32 * n + t[0] as u32) as usize;
    let mut want = list.clone();
    for t in must.iter().filter(|t| inside(t)) {
        want[word_of(t)] = CAST;
    }
    let either: BTreeSet<usize> = may
        .iter()
        .filter(|t| inside(t) && !must.contains(*t))
        .map(word_of)
        .collect();
    let stood = must.iter().filter(|t| inside(t)).count();
    assert!(stood > 0, "{}: no tile stood on in the box", c.name);
    // The tiles in the box marked by what `of` takes alone.
    let alone = |of: &dyn Fn(&Pair) -> bool| {
        by.iter()
            .filter(|(t, ways)| inside(t) && ways.iter().all(|w| w.as_ref().is_some_and(of)))
            .count()
    };
    let stood_alone = [
        ("tiles stood on by pairs alone", alone(&|_| true)),
        (
            "tiles stood on by pairs across alone",
            alone(&|w| w.step == 0),
        ),
        (
            "tiles stood on by pairs down alone",
            alone(&|w| w.step == 1),
        ),
        (
            "tiles stood on by pairs down a diagonal alone",
            alone(&|w| w.step == 2),
        ),
        (
            "tiles stood on by pairs up a diagonal alone",
            alone(&|w| w.step == 3),
        ),
        (
            "tiles stood on by pairs up to row 0 alone",
            alone(&|w| w.row_0),
        ),
        (
            "tiles stood on by pairs a footprint apart alone",
            alone(&|w| w.whole),
        ),
    ];
    for (k, n) in stood_alone {
        kinds.count(k, n as u64);
    }
    kinds.add(
        "tiles a pair a tile past the footprint lies between, not stood on",
        past.iter().any(|t| inside(t) && !must.contains(t)),
    );

    let rays = texture(
        &gpu.device,
        &gpu.queue,
        c.size,
        ray::WORDS,
        wgpu::TextureUsages::STORAGE_BINDING,
        &words,
    );
    let view = array_view(&rays);
    let eye_buf = gpu.buffer(&eye_words, wgpu::BufferUsages::UNIFORM);
    let head_buf = gpu.buffer(&head, wgpu::BufferUsages::UNIFORM);
    let list_buf = gpu.buffer(&list, wgpu::BufferUsages::STORAGE);
    let side = SQUARE_SIDE;
    gpu.dispatch(
        pipe,
        &[
            (0, eye_buf.as_entire_binding()),
            (3, wgpu::BindingResource::TextureView(&view)),
            (11, head_buf.as_entire_binding()),
            (12, list_buf.as_entire_binding()),
        ],
        [wd.div_ceil(side), ht.div_ceil(side), 1],
    );
    let out = gpu.read(&list_buf);
    let rays_out = read_texture(&gpu.device, &gpu.queue, &rays);
    let name = c.name;
    tally.compare_by(
        &format!(
            "{name}: the list, {stood} tiles stood on, {} by an edge",
            either.len()
        ),
        &want,
        &out,
        |k| match k.checked_sub(stands) {
            Some(t) if t < (n * n) as usize => format!("tile {:?}", [t as u32 % n, t as u32 / n]),
            _ => format!("word {k}"),
        },
        |k, w, h| w == h || (either.contains(&k) && h == CAST),
    );
    tally.compare(&format!("{name}: the rays"), &words, &rays_out, |k| {
        format!("word {k}")
    });
}

/// Every texel's start as the device takes it (light_aim.wgsl `texel_start`), read off a probe
/// appended to light.wgsl, is `Starts::start`'s: summed in fixed point, nothing rounds apart.
fn starts_alike(gpu: &Gpu, l: &Light, starts: &light::Starts) {
    const PROBE: &str = r"
@group(0) @binding(40) var<storage, read_write> probe: array<i32>;

@compute @workgroup_size(16, 16, 1)
fn probe_cs(@builtin(global_invocation_id) id: vec3<u32>) {
  let o = texel_start(light_corner() + vec2<i32>(id.xy));
  let k = 3u * (id.y * PROBE_SIZE + id.x);
  probe[k] = o.x;
  probe[k + 1u] = o.y;
  probe[k + 2u] = o.z;
}
";
    let module = format!(
        "{}\nconst PROBE_SIZE: u32 = {SIZE}u;\n{PROBE}",
        gpu::source(src(SHADER_LIGHT), "light_cs")
    );
    let p = gpu.pipeline(&module, "probe_cs");
    let mut eye_words = vec![0u32; eye::WORDS];
    eye_words[eye::LIGHT..eye::LIGHT + eye::LIGHT_WORDS].copy_from_slice(&l.words());
    let mut head = vec![0u32; light::HEAD_WORDS];
    head[light::STARTS_AT * 4..(light::STARTS_AT + light::STARTS_VEC4S) * 4]
        .copy_from_slice(&starts.words());
    let eye_buf = gpu.buffer(&eye_words, wgpu::BufferUsages::UNIFORM);
    let head_buf = gpu.buffer(&head, wgpu::BufferUsages::UNIFORM);
    let out = gpu.buffer(
        &vec![0u32; 3 * (SIZE * SIZE) as usize],
        wgpu::BufferUsages::STORAGE,
    );
    gpu.dispatch(
        &p,
        &[
            (0, eye_buf.as_entire_binding()),
            (11, head_buf.as_entire_binding()),
            (40, out.as_entire_binding()),
        ],
        [SIZE / 16, SIZE / 16, 1],
    );
    let w = gpu.read(&out);
    let off = (0..(SIZE * SIZE) as usize)
        .filter(|&k| {
            let p = [k as i32 % SIZE as i32, k as i32 / SIZE as i32];
            let want = starts.start([l.corner[0] + p[0], l.corner[1] + p[1]]);
            want != [0, 1, 2].map(|a| w[3 * k + a] as i32)
        })
        .count();
    assert_eq!(off, 0, "{off} texels' starts not the Rust's");
}
