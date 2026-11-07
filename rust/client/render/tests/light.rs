//! `light_cs` and the receivers' passes by hand (frame/shader/light.wgsl, light_mark.wgsl)
//! answer as pxl-engine's light over the real world of tests/real_world. A slot of a tile the
//! cast casts takes its visible voxel, a drawable entry of the node cache, where its own ray enters it
//! as the sample did
//! (`cast::entry_with`) at the band it asks (`Light::band`), its word
//! `light::word(light::depth(..))`; else the sky where the four samples about it
//! (`light::samples_about`) went on to their reach at its band; else it is listed for its walk.
//! That composition is this test's own reading of light.wgsl `light_cs`, which pxl-engine has no
//! one function for. The samples are each walked by `cast::first` over the world, and each
//! texel's visible voxel is the nearest of those about it its ray enters, as the raster keeps
//! it, some put out of place. Every word of the map is compared, and the slots listed for their
//! walk as a set. The light's reciprocals are the host's words (`Light::inverse`,
//! `Light::t_max_inverse`), and a ray's start is summed in fixed point (`Starts::start`), read
//! off a probe and held to the Rust's exactly; a slot may take what a fused sum gives it, each
//! counted: its depth's product fused into the push after it; a slot's band is exact
//! (`Light::band`). The receivers stand on the tiles `Light::box_tiles` gives, each listed once
//! (`hand_light_marks_answer_as_the_rust`). Skipped where no adapter answers.

mod gpu;
mod real_world;

use std::collections::BTreeSet;

use gpu::{array_view, cross, lay_entry, read_texture, texture, unit, Gpu, Kinds, LightBox, Tally};
use pxl_engine::answer::Kind;
use pxl_engine::beam;
use pxl_engine::cast::{self, Detail, End, Hit, Node, Ray, Spacing, Trace, Until};
use pxl_engine::light::{self, Light};
use pxl_engine::overwrite::Cut;
use pxl_engine::{eye, node_cache, param, raster, ray, record, Zoom};
use pxl_render::frame::shader::record::{src, SHADER_LIGHT, SHADER_LIGHT_MARK, SHADER_RECORD_DRAW};
use pxl_render::frame::shader::word::SQUARE_SIDE;

use real_world::{q8, Asking, Built, World, FINEST};

/// East of X = 0 by the seam, and by X = 0, where X's roots part.
const EYE: [i32; 2] = [(1 << 23) - 300, (40 << 16) + 30];
const EYE_ZERO: [i32; 2] = [150, (40 << 16) + 30];
const SIZE: u32 = 256;
const CAST: u32 = 7;
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
    /// Where the cast is within its epoch, one tile in so many claimed; none where it begins
    /// it, every tile read claimed.
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
            name: "about the eye, the epoch begun",
            lit: lit(
                over(&built, [EYE[0] + 90, EYE[1] + 90], 40),
                [0.3, 0.2, -0.93],
                1.0,
                60.0,
                400.0,
                [1300, -900],
            ),
            range: [8, FINEST],
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
            listed: None,
        },
        SlotCase {
            name: "tiles claimed within the epoch",
            lit: lit(
                over(&built, [EYE[0] + 90, EYE[1] + 90], 40),
                [0.5, -0.1, -0.86],
                1.0,
                60.0,
                400.0,
                [1296, -904],
            ),
            range: [10, FINEST],
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
        "walked, its visible not the stop at its band",
        "walked, a sample about it another band's",
        "past the list",
        "samples gone on",
        "samples stopped",
        "kept",
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

    // The list's words noise, the samples walked over the world.
    let mut list: Vec<u32> = (0..light::list_words(SIZE)).map(noise).collect();
    // Each tile of the slots' grid at a place of its own, its slots' states in the cast's
    // region (`light::places_at`, `light::region_words`): none parked, and in one tile in seven
    // every slot kept, its ray finished in the cast before and carried with its tile
    // (`ray::KEPT`, light_propagate.wgsl `claim`).
    list.resize(light::buffer_words(SIZE) as usize, 0);
    let places = light::places_at(SIZE) as usize;
    for t in 0..tiles * tiles {
        list[places + t as usize] = t;
    }
    let kept = |slot: [u32; 2]| {
        let t = (slot[1] / light::TILE) * tiles + slot[0] / light::TILE;
        noise(t ^ 0x6b65).is_multiple_of(7)
    };
    for y in 0..SIZE {
        for x in 0..SIZE {
            if kept([x, y]) {
                let place = (y / light::TILE) * tiles + x / light::TILE;
                let k = (y % light::TILE) * light::TILE + x % light::TILE;
                let at = light::list_words(SIZE)
                    + ((CAST % 2) * light::parked_room() + place * light::TILE * light::TILE + k)
                        * ray::STATE_WORDS;
                list[(at + ray::STATE_WORDS - 1) as usize] = ray::KEPT;
            }
        }
    }
    // The node cache, a drawable entry a sample at the sample's number, as the slots read it by
    // the visible id (pxl-engine `record`): its node, from where the sample's ray starts, its
    // answer, and its state the face the sample entered and whether it stopped coarser than its
    // band (`record::light_whole`).
    let mut rec = vec![0u32; node_cache::WORDS as usize];
    rec[node_cache::FRAME as usize] = CAST;
    let mut origins = vec![[0i32; 3]; (n * n) as usize];
    let mut q = Asking {
        w,
        cuts: [Cut::NONE; 2],
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
            origins[(j * n + i) as usize] = o;
            if !gone {
                let kind = if met.ends == End::Lacks {
                    Kind::Lacks
                } else {
                    Kind::Cell
                };
                let state = raster::faces([ray::face(l.dir, met.hit.axis)])
                    | if record::light_whole(met.hit.node.zoom, b) {
                        record::WHOLE
                    } else {
                        0
                    };
                lay_entry(
                    &mut rec,
                    j * n + i,
                    o,
                    &met.hit.node,
                    (kind, met.hit.asset),
                    state,
                );
            }
        }
    }
    let stopped: Vec<u32> = (0..n * n)
        .filter(|&s| {
            let k = light::sample(SIZE, s) + beam::SAMPLE_NODE + beam::NODE_ZOOM;
            list[k as usize] != beam::SAMPLE_NONE
        })
        .collect();

    // Each texel's visible voxel, as the raster keeps it: the nearest of those about it whose
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

    // The tiles the cast casts, each listed once, the room past them an unlisted tile's: where
    // it begins its epoch, those read, all but some; else some of the box, each read, as a
    // claimed tile is (light_propagate.wgsl).
    let reads_at = light::reads_at(SIZE) as usize;
    let mut threads: Vec<Option<[u32; 2]>> = Vec::new();
    let mut entries = Vec::new();
    for j in 0..tiles {
        for i in 0..tiles {
            let t = j * tiles + i;
            let (claimed, word) = match c.listed {
                Some(every) => {
                    let claimed = (i + 2 * j).is_multiple_of(every) || i == 0 || j == tiles - 1;
                    (claimed, if claimed { CAST } else { CAST - 1 })
                }
                None => {
                    let read = !noise(t ^ 0x77).is_multiple_of(5);
                    (read, if read { CAST } else { CAST - 1 })
                }
            };
            if claimed {
                entries.push(j << 16 | i);
            }
            list[reads_at + t as usize] = word;
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
    let groups = [light::LIST_ACROSS, rows, 1];

    let mut eye_words = vec![0u32; eye::WORDS];
    eye_words[eye::LIGHT..eye::LIGHT + eye::LIGHT_WORDS].copy_from_slice(&l.words());
    eye_words[eye::RANGE] = c.range[0];
    eye_words[eye::RANGE + 1] = c.range[1];
    let mut head = vec![0u32; light::HEAD_WORDS];
    head[light::HEAD_CAST] = CAST;
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

    // Each needed slot's answer from its ray's start `o`.
    let gone_on = |s: [u32; 2]| {
        let k = light::sample(SIZE, (s[1] % n) * n + s[0] % n) + beam::SAMPLE_NODE;
        list[(k + beam::NODE_ZOOM) as usize] == beam::SAMPLE_NONE
    };
    let answer = |slot: [u32; 2], o: [i32; 3], inv: [f32; 3], kinds: &mut Kinds| {
        let band_of = |p: [u32; 2]| {
            let tile = [(p[0] / light::TILE) as i32, (p[1] / light::TILE) as i32];
            l.band(SIZE, c.range, tile)
        };
        let sample_band = |s: [u32; 2]| band_of(l.texel(SIZE, [s[0] * d, s[1] * d]));
        let p = l.texel(SIZE, slot);
        let r = Ray::new(l.dir, l.t_max);
        let band = band_of(p);
        let visible = vis[(p[1] * SIZE + p[0]) as usize];
        if visible != 0 && visible & raster::LACK == 0 {
            let s = raster::voxel_of(visible);
            let (node, asset, axis) = cell_of(&list, light::sample(SIZE, s) as usize);
            // The node as the slot's ray holds it, its roots from where that ray starts.
            let whole = record::light_whole(node.zoom, sample_band([s % n, s / n]));
            let node = node.moved(origins[s as usize], o);
            let e = cast::entry_with(o, &r, inv, &node, asset[0]);
            // The node a ray asking the slot's band stops on (`record::band_fresh`).
            let alike = record::band_fresh(node.zoom, whole, band);
            if e.enters && e.axis == axis && alike {
                kinds.add("took", true);
                return Answer::Entry(e.t, e.axis);
            }
            kinds.add(
                "walked, its visible not entered",
                !(e.enters && e.axis == axis),
            );
            kinds.add(
                "walked, its visible not the stop at its band",
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
            if kept(slot) {
                kinds.add("kept", true);
                return Answer::Kept;
            }
            let (o, _) = light::ray(&l, &starts, l.texel(SIZE, slot));
            answer(slot, o, inv, kinds)
        })
        .collect();
    // The pass over fresh words: the map it leaves and the slots it listed.
    let (map, had) = {
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
        let misses_buf = gpu.buffer(&ray::MISS_ARGS_EMPTY, wgpu::BufferUsages::STORAGE);
        let rec_buf = gpu.buffer(&rec, wgpu::BufferUsages::STORAGE);
        gpu.dispatch(
            pipe,
            &[
                (0, eye_buf.as_entire_binding()),
                (8, list_buf.as_entire_binding()),
                (10, wgpu::BindingResource::TextureView(&map_v)),
                (11, head_buf.as_entire_binding()),
                (15, wgpu::BindingResource::TextureView(&vis_v)),
                (17, misses_buf.as_entire_binding()),
                (9, rec_buf.as_entire_binding()),
            ],
            groups,
        );
        let list_out = gpu.read(&list_buf);
        let misses = gpu.read(&misses_buf);
        // The slots it listed for their walk.
        let from = light::misses_at(SIZE) as usize;
        let to = from + (misses[ray::MISS_ARGS_CNT as usize] as usize).min((SIZE * SIZE) as usize);
        let had = list_out[from..to].to_vec();
        (read_texture(&gpu.device, &gpu.queue, &map_t), had)
    };
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
    let (mut exact, mut fused) = (0u32, 0u32);
    for (&slot, &want) in needed.iter().zip(&wants) {
        // A kept slot's word stands, and it is not listed.
        if want == Answer::Kept {
            continue;
        }
        let at = (slot[1] * SIZE + slot[0]) as usize;
        let g = got(at, slot);
        // The answer the slot is held to: its own, or where a GPU's fused sum in its depth may
        // give another word, the one the hand gave.
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
    // The listed slots as a set, each once.
    let listed_alike = had.len() == had_set.len() && had_set == walked;
    tally.compare(
        &format!("{name}: the slots listed, as a set"),
        &[u32::from(true)],
        &[u32::from(listed_alike)],
        |_| format!("{} listed, {} to walk", had.len(), walked.len()),
    );
    let walked_n = walked.len() as u32;
    eprintln!(
        "{name}: {} needed: {took} took, {sky} sky, {walked_n} walked; {exact} exact, {fused} \
         a depth a product fused into a sum",
        needed.len()
    );
    kinds.count("slots a depth a product fused into a sum", u64::from(fused));
}

/// A slot's answer: the depth `t` along its ray and the axis it enters its visible voxel's
/// cell across, the sky, its walk, or its word as the cast before left it, kept; and the word a
/// pass left it.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Answer {
    Entry(f32, u32),
    Sky,
    Walk,
    Kept,
    Word(u32),
}

/// The word of a slot that took its visible voxel `t` along its ray across `axis`
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

/// The tiles a set of receivers must stand on, and those they may, by a tile's edge or the
/// reach's.
#[derive(Default)]
struct Stood {
    must: BTreeSet<[i32; 2]>,
    may: BTreeSet<[i32; 2]>,
}

impl Stood {
    fn put(&mut self, t: [i32; 2], sure: bool) {
        if sure {
            self.must.insert(t);
        }
        self.may.insert(t);
    }

    /// The tiles from `first` to `last`, each `sure` or not.
    fn put_range(&mut self, [first, last]: [[i32; 2]; 2], sure: bool) {
        for y in first[1]..=last[1] {
            for x in first[0]..=last[0] {
                self.put([x, y], sure);
            }
        }
    }
}

/// Whether a point `d` units from the eye lies within `reach` of it: yes, no, or by the reach's
/// edge, where a GPU's rounding may put it either side.
fn within_by(d: f32, reach: f32) -> (bool, bool) {
    let edge = 1e-4 * reach;
    (d <= reach - edge, d <= reach + edge)
}

/// The receivers' passes by hand (light_mark.wgsl, light_stood.wgsl) stand on the tiles
/// pxl-engine's one rule gives (`Light::box_tiles`, the footprint where the head asks it), each
/// listed once (`light::LISTED_STOOD`), the count in the stood list's args
/// (`light::STOOD_CNT`), over the real world of tests/real_world: the pixels the eye walked
/// apart, off their walked bits past the grid's words (`light_mark_walked_cs`, `ray::hole_bit`),
/// each hit a box of one point; and every box the eye's draw writes, a held voxel in
/// view each, its planes as `cast::asset_box` lays them, stood as it is written, where the head
/// says the world's receivers stand (record_draw.wgsl `record_draw_cs`,
/// `light::HEAD_STANDS_WORLD`). The cast's first stood tile opens the slide
/// and begins the reads' epoch anew where a slide went unrun (`light::READS_FROM`); none
/// stood, none listed and the slide shut, as past the reach. A tile a point lies within `EDGE`
/// of, or a point by the reach, may be stood on or not. Skipped where no adapter answers.
#[test]
fn hand_light_marks_answer_as_the_rust() {
    let Some(gpu) = Gpu::new() else {
        eprintln!("no adapter: skipped");
        return;
    };
    let pipes = [
        gpu.pipeline(src(SHADER_LIGHT_MARK), "light_mark_walked_cs"),
        gpu.pipeline(src(SHADER_RECORD_DRAW), "record_draw_cs"),
    ];
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
        Look {
            name: "looking up from the ground, the sky over the hills",
            origin: over([EYE[0] + 90, EYE[1] + 90], 60),
            look: [0.1, 0.8, 0.6],
            tan: [0.9, 0.6],
            far: 900.0,
            size: [40, 27],
            jitter: [0.25, 0.125],
            footprint: true,
            lit: lit([0.3, 0.2, -0.93], 1.0),
        },
        Look {
            name: "high over the ground, nothing within the reach",
            origin: over([EYE[0] + 90, EYE[1] + 90], 2000),
            look: [0.3, 0.2, -0.93],
            tan: [0.3, 0.2],
            far: 4000.0,
            size: [40, 27],
            jitter: [0.125, 0.0],
            footprint: true,
            lit: lit([0.3, 0.2, -0.93], 1.0),
        },
    ];
    let mut tally = Tally::default();
    let mut kinds = Kinds::default();
    for c in &looks {
        for by in [Marks::Walked, Marks::Voxels] {
            run_marks(&gpu, &pipes, built.world(), c, by, &mut tally, &mut kinds);
        }
    }
    kinds.hold(&[
        "pixels met",
        "pixels met nothing",
        "pixels walked apart",
        "pixels past the reach",
        "voxels",
        "samples gone on",
        "voxels' boxes past the reach",
        "tiles stood on",
        "casts that stood nothing",
    ]);
    tally.end("light_mark_walked_cs, record_draw_cs");
}

/// Which of the receivers' passes stands the tiles (`hand_light_marks_answer_as_the_rust`).
#[derive(Clone, Copy, PartialEq)]
enum Marks {
    Walked,
    Voxels,
}

fn run_marks(
    gpu: &Gpu,
    pipes: &[wgpu::ComputePipeline; 2],
    w: World,
    c: &Look,
    by: Marks,
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
    let reach = l.reach(SIZE);
    let n = light::tiles(SIZE);
    let grow = if c.footprint {
        light::FOOTPRINT_TILES
    } else {
        0
    };
    let mut q = Asking {
        w,
        cuts: [Cut::NONE; 2],
    };
    let detail = Detail {
        range,
        spacing: Spacing {
            angle: 0.002,
            across: 0.0,
        },
    };
    let mut eye_words = vec![0u32; eye::WORDS];
    for k in 0..3 {
        eye_words[eye::FORWARD + k] = forward[k].to_bits();
        eye_words[eye::RIGHT + k] = right[k].to_bits();
        eye_words[eye::UP + k] = up[k].to_bits();
        eye_words[eye::ORIGIN + k] = c.origin[k] as u32;
    }
    eye_words[eye::TAN] = c.tan[0].to_bits();
    eye_words[eye::TAN + 1] = c.tan[1].to_bits();
    eye_words[eye::JITTER] = c.jitter[0].to_bits();
    eye_words[eye::JITTER + 1] = c.jitter[1].to_bits();
    eye_words[eye::RANGE] = range[0];
    eye_words[eye::RANGE + 1] = range[1];
    eye_words[eye::FAR] = c.far.to_bits();
    eye_words[eye::LIGHT..eye::LIGHT + eye::LIGHT_WORDS].copy_from_slice(&l.words());
    let grid = beam::Grid::of(c.tan, 1);
    eye_words[eye::GRID..eye::GRID + beam::GRID_WORDS].copy_from_slice(&grid.words());
    let walked_at = grid.walked_at() as usize;
    let mut beams = vec![0u32; walked_at + ray::holes_words([wd, ht]) as usize];

    // Each pixel's ray walked over the world: its first word and its t, as the cast leaves them;
    // some meeting nothing, some walked apart and listed.
    let mut words: Vec<u32> = (0..ray::WORDS * wd * ht).map(noise).collect();
    let tan_of = |p: [u32; 2]| {
        let at = [0, 1].map(|k| (p[k] as f32 + 0.5 + c.jitter[k]) / c.size[k] as f32 * 2.0);
        [(at[0] - 1.0) * c.tan[0], (1.0 - at[1]) * c.tan[1]]
    };
    let mut want = Stood::default();
    // The tiles about `t`, the footprint where the head asks it, kept to the box.
    let about = |t: [i32; 2]| {
        let m = n as i32 - 1;
        [
            [(t[0] - grow).max(0), (t[1] - grow).max(0)],
            [(t[0] + grow).min(m), (t[1] + grow).min(m)],
        ]
    };
    // The raster's draw args as the draw begins, room for the boxes, and the node cache, every
    // slot free but those the samples' nodes hold.
    let mut raster_args = vec![0u32; raster::ARGS_WORDS as usize];
    raster_args[raster::ARGS_VERTICES as usize] = raster::VERTICES;
    let boxes = vec![0u32; (record::BOXES * raster::BOX_WORDS) as usize];
    let mut rec = vec![0u32; node_cache::WORDS as usize];
    match by {
        Marks::Walked => {
            let mut walked = Vec::new();
            for y in 0..ht {
                for x in 0..wd {
                    let k = (y * wd + x) as usize;
                    let t = tan_of([x, y]);
                    let dir = cast::through(forward, right, up, t[0], t[1]);
                    let r = Ray::new(dir, c.far);
                    let m = cast::first(&mut q, c.origin, &r, detail, Until::Opaque);
                    let h = noise(k as u32 ^ 0x4242);
                    let (first, at) = if m.ends == End::Hit && !h.is_multiple_of(17) {
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
                    if !h.is_multiple_of(3) {
                        continue;
                    }
                    walked.push((y << 16) | x);
                    if first == 0 {
                        continue;
                    }
                    let v = dir.map(|a| a * m.hit.t);
                    let d = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
                    kinds.add("pixels met", true);
                    kinds.add("pixels past the reach", d > reach);
                    let (sure, maybe) = within_by(d, reach);
                    if !maybe {
                        continue;
                    }
                    let own = tiles_about(&l, v);
                    for &t in &own {
                        want.put_range(about(t), sure && own.len() == 1);
                    }
                }
            }
            kinds.count("pixels walked apart", walked.len() as u64);
            for &p in &walked {
                let (w, bit) = ray::hole_bit([p & 0xffff, p >> 16], [wd, ht]);
                beams[walked_at + w as usize] |= 1 << bit;
            }
        }
        Marks::Voxels => {
            // A held voxel of the record a stopped sample's node, a slot each, as the samples
            // write it (sample_box.wgsl); the draw writes each in view as a box and stands it.
            let mut slot = 0u32;
            for j in 0..grid.samples[1] {
                for i in 0..grid.samples[0] {
                    let t = grid.sample([i, j]);
                    let dir = cast::through(forward, right, up, t[0], t[1]);
                    let r = Ray::new(dir, c.far);
                    let m = cast::first(&mut q, c.origin, &r, detail, Until::Opaque);
                    if m.ends == End::Miss {
                        kinds.add("samples gone on", true);
                        continue;
                    }
                    // A face an eye's ray entered and asked about (`record::ASKED`).
                    let face = 1 << m.hit.axis;
                    let kind = if m.ends == End::Lacks {
                        Kind::Lacks
                    } else {
                        Kind::Cell
                    };
                    let state = face | face << record::ASKED_SHIFT;
                    lay_entry(
                        &mut rec,
                        slot,
                        c.origin,
                        &m.hit.node,
                        (kind, m.hit.asset),
                        state,
                    );
                    slot += 1;
                    kinds.add("voxels", true);
                    // Drawn where in view or by the eye; a box a hair smaller holds the tiles it
                    // must stand on, a hair larger those it may, as a GPU's rounding may move a
                    // tile's edge or the view's.
                    let (lo, hi) = cast::asset_box(c.origin, &m.hit.node, m.hit.asset[0]);
                    let hair = 1e-3;
                    let shrunk = (lo.map(|v| v + hair), hi.map(|v| v - hair));
                    let seen = raster::is_eyed(shrunk.0, shrunk.1, c.tan)
                        || record::in_view(shrunk, [right, up, forward], c.tan, c.far * 0.999);
                    let inner = l
                        .box_tiles(SIZE, shrunk.0, shrunk.1, c.footprint)
                        .filter(|_| seen && Light::within(lo, hi, reach * (1.0 - 1e-4)));
                    let outer = l.box_tiles(
                        SIZE,
                        lo.map(|v| v - hair),
                        hi.map(|v| v + hair),
                        c.footprint,
                    );
                    kinds.add("voxels' boxes past the reach", outer.is_none());
                    for (range, sure) in [(outer, false), (inner, true)] {
                        if let Some(r) = range {
                            want.put_range(r, sure);
                        }
                    }
                }
            }
        }
    }
    let inside = |t: &[i32; 2]| t[0] >= 0 && t[1] >= 0 && t[0] < n as i32 && t[1] < n as i32;
    want.must.retain(inside);
    want.may.retain(inside);
    kinds.count("tiles stood on", want.must.len() as u64);
    kinds.add("casts that stood nothing", want.may.is_empty());

    let mut head = vec![0u32; light::HEAD_WORDS];
    head[light::HEAD_CAST] = CAST;
    head[light::HEAD_WORLD] = CAST;
    head[light::HEAD_FOOTPRINT] = u32::from(c.footprint);
    head[light::HEAD_SLIDE_OWED] = CAST - 2;
    head[light::HEAD_STANDS_WORLD] = 1;
    for (k, v) in light::head_layout(SIZE) {
        head[k] = v;
    }
    // The list noise but the slide's words: the slide last ran before the last cast that owed
    // one. The marks some other cast's, none the world's this one's.
    let mut list: Vec<u32> = (0..light::list_words(SIZE))
        .map(|k| noise(k ^ 0x99))
        .collect();
    let slide = light::slide_at(SIZE) as usize;
    list[slide + light::SLIDE_RAN as usize] = CAST - 3;
    list[slide + light::READS_FROM as usize] = 0;
    let marks_at = (light::ARGS_WORDS + light::listed_marks_at(SIZE, light::LISTED_STOOD)) as usize;
    let mut args: Vec<u32> = (0..light::args_words(SIZE)).map(noise).collect();
    for t in 0..(n * n) as usize {
        args[marks_at + t] = match noise(t as u32) % 3 {
            0 => 0,
            1 => CAST + 1,
            _ => CAST - 1,
        };
    }
    let mut stood = vec![0u32; light::STOOD_WORDS as usize];
    stood[..ray::LEFT_EMPTY.len()].copy_from_slice(&ray::LEFT_EMPTY);

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
    let args_buf = gpu.buffer(&args, wgpu::BufferUsages::STORAGE);
    let stood_buf = gpu.buffer(&stood, wgpu::BufferUsages::STORAGE);
    let beams_buf = gpu.buffer(&beams, wgpu::BufferUsages::STORAGE);
    let raster_buf = gpu.buffer(&raster_args, wgpu::BufferUsages::STORAGE);
    let boxes_buf = gpu.buffer(&boxes, wgpu::BufferUsages::STORAGE);
    let record_buf = gpu.buffer(&rec, wgpu::BufferUsages::STORAGE);
    let mut binds: Vec<(u32, wgpu::BindingResource)> = vec![
        (0, eye_buf.as_entire_binding()),
        (11, head_buf.as_entire_binding()),
        (12, list_buf.as_entire_binding()),
        (21, args_buf.as_entire_binding()),
        (22, stood_buf.as_entire_binding()),
    ];
    let (pipe, groups) = match by {
        Marks::Walked => {
            binds.push((3, wgpu::BindingResource::TextureView(&view)));
            binds.push((23, beams_buf.as_entire_binding()));
            let words = ray::hole_words([wd, ht]);
            let per = ray::MISS_LANES * ray::MISS_ACROSS;
            (&pipes[0], [ray::MISS_ACROSS, words.div_ceil(per).max(1), 1])
        }
        Marks::Voxels => {
            binds.push((14, boxes_buf.as_entire_binding()));
            binds.push((16, raster_buf.as_entire_binding()));
            binds.push((9, record_buf.as_entire_binding()));
            let per = record::DRAW_LANES;
            (&pipes[1], [node_cache::ENTRIES / per, 1, 1])
        }
    };
    gpu.dispatch(pipe, &binds, groups);
    let out = gpu.read(&list_buf);
    let out_stood = gpu.read(&stood_buf);
    let cnt = out_stood[light::STOOD_CNT as usize] as usize;
    let at = light::listed_at(SIZE, light::LISTED_STOOD) as usize;
    let listed: Vec<[i32; 2]> = out[at + 1..at + 1 + cnt.min((n * n) as usize)]
        .iter()
        .map(|&w| [(w & 0xffff) as i32, (w >> 16) as i32])
        .collect();
    let got: BTreeSet<[i32; 2]> = listed.iter().copied().collect();
    let name = c.name;
    let case = |what: &str| {
        format!(
            "{name}, {}: {what}",
            ["the walked", "the voxels"][by as usize]
        )
    };
    // Each tile listed once, every one it must stand on listed, none it may not.
    tally.compare(
        &case("each listed once"),
        &[listed.len() as u32],
        &[got.len() as u32],
        |_| "listed".into(),
    );
    let must: Vec<[i32; 2]> = want.must.iter().copied().collect();
    tally.compare(
        &case("the tiles it must stand on"),
        &vec![1; must.len()],
        &must
            .iter()
            .map(|t| u32::from(got.contains(t)))
            .collect::<Vec<_>>(),
        |k| format!("tile {:?}", must[k]),
    );
    let extra: Vec<[i32; 2]> = got.difference(&want.may).copied().collect();
    tally.compare(
        &case("tiles past what it may stand on"),
        &[0],
        &[extra.len() as u32],
        |_| format!("{extra:?}"),
    );
    // The slide opened and the reads' epoch begun anew by the first stood tile, else neither.
    let n_slide = n.div_ceil(SQUARE_SIDE);
    let any = cnt != 0;
    tally.compare(
        &case("the slide opened, the reads' epoch begun anew"),
        &[if any { n_slide } else { 0 }, if any { CAST } else { 0 }],
        &[
            out_stood[light::STOOD_SLIDE as usize],
            out[slide + light::READS_FROM as usize],
        ],
        |k| ["the slide's workgroups across", "the reads' epoch"][k].into(),
    );
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

/// Every pass that reads a tile's band (light_aim.wgsl `tile_band`) gives each tile the band
/// pxl-engine's `Light::band` gives it, so no two passes split over one: a slot's against its
/// samples' run's and the holes' queue's, a cell's against its beam rays', a slide's against the cast it
/// keeps. A probe appended to each pass's module reads every tile of a box, and of the beam
/// rays' window past it, over boxes of every side, their texels' width at random and where a
/// tile's distance squared lands on a split distance's.
#[test]
fn every_pass_bands_a_tile_alike() {
    use pxl_engine::grid::INV_SPLIT2;
    use pxl_engine::FINEST_ZOOM;
    use pxl_render::frame::shader::record::{
        compute, COMPUTE_LIGHT, COMPUTE_LIGHT_CELL, COMPUTE_LIGHT_SLIDE, COMPUTE_QUEUE,
        COMPUTE_QUEUE_LAST, COMPUTE_QUEUE_LAST_LIGHT_CORNERS, COMPUTE_QUEUE_LAST_LIGHT_SAMPLES,
        COMPUTE_QUEUE_LIGHT_CORNERS, COMPUTE_QUEUE_LIGHT_SAMPLES,
    };
    const PROBE: &str = r"
@group(0) @binding(40) var<storage, read_write> probe: array<u32>;

@compute @workgroup_size(8, 8, 1)
fn probe_cs(@builtin(global_invocation_id) id: vec3<u32>) {
  let across = head_word(LIGHT_HEAD_SIZE) / LIGHT_TILE + 2u * PROBE_LEAD;
  if (id.x >= across || id.y >= across) {
    return;
  }
  probe[id.y * across + id.x] = tile_band(vec2<i32>(id.xy) - vec2<i32>(i32(PROBE_LEAD)));
}
";
    /// Tiles probed before the box's first, as far as the beam rays' window reaches.
    const LEAD: u32 = light::BEAM_WINDOW + 1;
    let Some(gpu) = Gpu::new() else {
        eprintln!("no adapter: skipped");
        return;
    };
    let passes = [
        COMPUTE_LIGHT,
        COMPUTE_QUEUE_LIGHT_SAMPLES,
        COMPUTE_QUEUE_LAST_LIGHT_SAMPLES,
        COMPUTE_QUEUE_LAST,
        COMPUTE_QUEUE,
        COMPUTE_QUEUE_LIGHT_CORNERS,
        COMPUTE_QUEUE_LAST_LIGHT_CORNERS,
        COMPUTE_LIGHT_CELL,
        COMPUTE_LIGHT_SLIDE,
    ]
    .map(|id| {
        let (shader, entry) = compute(id);
        let module = format!(
            "{}\nconst PROBE_LEAD: u32 = {LEAD}u;\n{PROBE}",
            gpu::source(src(shader), entry)
        );
        (entry, gpu.pipeline(&module, "probe_cs"))
    });
    // The texel widths: at random over four octaves either side of one, and where a tile
    // `g` texels off the middle on each axis lies a split distance off, a last place either
    // side.
    let mut widths: Vec<f32> = (0..24)
        .map(|k| f32::from_bits(0x3e00_0000 + noise(k) % 0x0200_0000))
        .collect();
    for (k, g) in [[24u32, 32], [40, 96], [64, 120], [8, 8], [16, 56], [48, 48]]
        .into_iter()
        .enumerate()
    {
        let split2 = f64::from(1u32 << (2 * (k % 3))) / f64::from(INV_SPLIT2);
        let w = (split2 / f64::from(g[0] * g[0] + g[1] * g[1])).sqrt() as f32;
        widths.extend([w.next_down(), w, w.next_up()]);
    }
    let sides = [64u32, 256, 1024, 4096];
    let ranges: [[Zoom; 2]; 2] = [[0, FINEST_ZOOM], [12, 26]];
    let (mut tiles, mut off, mut split) = (0u64, 0u64, 0u64);
    for (b, &width) in widths.iter().enumerate() {
        let size = sides[b % sides.len()];
        let range = ranges[b % ranges.len()];
        let l = Light {
            first: [0.0; 3],
            t_max: 100.0,
            across: [width, 0.0, 0.0],
            down: [0.0, width, 0.0],
            dir: [0.0, 0.0, -1.0],
            bias: [0.0; 3],
            corner: [0, 0],
            reach_ndc: 1.0,
        };
        let mut eye_words = vec![0u32; eye::WORDS];
        eye_words[eye::LIGHT..eye::LIGHT + eye::LIGHT_WORDS].copy_from_slice(&l.words());
        eye_words[eye::RANGE] = range[0];
        eye_words[eye::RANGE + 1] = range[1];
        let mut head = vec![0u32; light::HEAD_WORDS];
        head[light::HEAD_SIZE] = size;
        let eye_buf = gpu.buffer(&eye_words, wgpu::BufferUsages::UNIFORM);
        let head_buf = gpu.buffer(&head, wgpu::BufferUsages::UNIFORM);
        let across = light::tiles(size) + 2 * LEAD;
        let want: Vec<u32> = (0..across * across)
            .map(|k| {
                let t = [
                    (k % across) as i32 - LEAD as i32,
                    (k / across) as i32 - LEAD as i32,
                ];
                l.band(size, range, t)
            })
            .collect();
        let got: Vec<Vec<u32>> = passes
            .iter()
            .map(|(_, p)| {
                let out = gpu.buffer(&vec![u32::MAX; want.len()], wgpu::BufferUsages::STORAGE);
                gpu.dispatch(
                    p,
                    &[
                        (0, eye_buf.as_entire_binding()),
                        (11, head_buf.as_entire_binding()),
                        (40, out.as_entire_binding()),
                    ],
                    [across.div_ceil(8), across.div_ceil(8), 1],
                );
                gpu.read(&out)
            })
            .collect();
        for k in 0..want.len() {
            tiles += 1;
            off += u64::from(got.iter().any(|g| g[k] != want[k]));
            split += u64::from(got.iter().any(|g| g[k] != got[0][k]));
        }
    }
    let names: Vec<&str> = passes.iter().map(|(e, _)| *e).collect();
    eprintln!(
        "{tiles} tiles over {} boxes in {names:?}: {off} off pxl-engine's band, {split} split \
         between passes",
        widths.len()
    );
    assert_eq!(
        (off, split),
        (0, 0),
        "tiles off the engine's band, split between passes"
    );
}
