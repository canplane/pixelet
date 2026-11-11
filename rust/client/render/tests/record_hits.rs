//! Checks that the record's passes over the holes' hits (frame/shader/record_hits.wgsl) answer
//! as pxl-engine's record does, over the real world of tests/real_world.
//!
//! `rec_hits_cs` writes each hit an observer's holes kept into the entry that draws the
//! hit's node (record_put.wgsl `observe_hit`), the eye's hits and the light's alike
//! (record_hits.eye.wgsl, record_hits.light.wgsl). The hits are those the holes' queue leaves
//! over the world: the eye's in frames that cast their samples first, whose hits set the
//! entries and whose faces' pass answered their asks, and in frames that leave the entries to
//! the holes' hits; and the light's over its box. Some entries the hits find are then laid
//! otherwise, as other frames and the other observer leave them (`lay_otherwise`). From those
//! same words, the test works out what pxl-engine `record` says the pass leaves
//! (`rec_want`) and compares every word of the node cache, of the raster's args and of the
//! observer's words. The asks the eye's hits open are compared as a set, because the device
//! lays them in the order its threads come.
//!
//! `rec_faces_cs` must then put, at each pixel that still waits for its hit, the word that
//! the hit's entry holds for the hit's face, once the faces' pass has answered the asks
//! (`faces_want`). Every word of the rays, of the node cache and of the eye's words is
//! compared.
//!
//! The tests count each kind of case they meet and fail where they meet none of one. They are
//! skipped where no adapter answers.

mod gpu;
mod real_world;

use pxl_spec::share::EIGHTHS;
use std::collections::{BTreeMap, BTreeSet};

use gpu::{
    array_view, cross, holds_node, mix, read_texture, texture, unit, Gpu, KindCount, LightBox,
    Tally,
};
use pxl_engine::answer::{self, Kind};
use pxl_engine::beam::{self, Grid};
use pxl_engine::cast::Node;
use pxl_engine::light::{self, Light, StartGrid};
use pxl_engine::object::Cut;
use pxl_engine::{eye, face, node_cache, raster, ray, record, WORKGROUPS_CAP};
use pxl_render::frame::shader::record::{
    compute, src, COMPUTE_FACE, COMPUTE_LIGHT_REC_HITS, COMPUTE_REC_FACES, COMPUTE_REC_HITS,
    SHADER_CELL, SHADER_LIGHT_CELL, SHADER_SAMPLE_HIT,
};
use pxl_render::frame::shader::word::SQUARE_SIDE;
use pxl_spec::space::coord::zoom::{Zoom, LEAF_CELL_ZOOM};

use real_world::{q8, store_of, Built, WorldBuffer, FINEST};

/// The leaf cell the world is built about, 300 cells west of X's seam.
const EYE: [i32; 2] = [(1 << 23) - 300, (40 << 16) + 30];
const FRAME: u32 = 9;
const WORKGROUP: u32 = beam::SAMPLE_SIDE;
/// The light's map's side, in texels.
const SIZE: u32 = 256;
/// The light's cast the head names. Its epoch begins with it, so every tile is read.
const CAST: u32 = 7;
/// A first ray word that no walk writes, which a hole's pixel holds until its walk ends it.
const UNWALKED: u32 = u32::MAX;

fn device() -> Option<Gpu> {
    // A loop in a hand-written shader can hang the wait on the device, so the run ends itself.
    std::thread::spawn(|| {
        std::thread::sleep(std::time::Duration::from_secs(1200));
        eprintln!("gave up after 20 minutes");
        std::process::exit(124);
    });
    let gpu = Gpu::new();
    if gpu.is_none() {
        eprintln!("no adapter: skipped");
    }
    gpu
}

fn world_bufs(gpu: &Gpu, built: &Built) -> WorldBuffer {
    let flat_u = |rows: &[[u32; 4]]| rows.iter().flatten().copied().collect::<Vec<u32>>();
    let clip: Vec<u32> = built.clipmap.iter().flatten().map(|&v| v as u32).collect();
    WorldBuffer {
        clip: gpu.buf(&clip, store_of()),
        ground: gpu.buf(&built.ground, store_of()),
        folds: gpu.buf(&built.folds, store_of()),
        shards: gpu.buf(&flat_u(&built.shards), store_of()),
        placed: gpu.buf(&built.objects, store_of()),
        pool: gpu.buf(&built.pool, store_of()),
    }
}

fn world_binds<'a>(
    b: &'a WorldBuffer,
    eye: &'a wgpu::Buffer,
    words: &'a wgpu::Buffer,
    node_cache: &'a wgpu::Buffer,
) -> Vec<(u32, wgpu::BindingResource<'a>)> {
    vec![
        (0, eye.as_entire_binding()),
        (1, b.clip.as_entire_binding()),
        (2, b.ground.as_entire_binding()),
        (4, b.folds.as_entire_binding()),
        (5, b.shards.as_entire_binding()),
        (6, b.placed.as_entire_binding()),
        (7, b.pool.as_entire_binding()),
        (8, words.as_entire_binding()),
        (9, node_cache.as_entire_binding()),
    ]
}

/// What the faces' pass binds, with its args at word `at` of the raster's args: the samples'
/// asks at `raster::ASKED_AT`, the holes' hits' at `raster::HIT_ASKED_AT`.
fn face_binds<'a>(
    b: &'a WorldBuffer,
    run: &'a Run,
    at: u32,
) -> Vec<(u32, wgpu::BindingResource<'a>)> {
    vec![
        (0, run.eye_buf.as_entire_binding()),
        (1, b.clip.as_entire_binding()),
        (2, b.ground.as_entire_binding()),
        (5, b.shards.as_entire_binding()),
        (6, b.placed.as_entire_binding()),
        (7, b.pool.as_entire_binding()),
        (8, run.words_buf.as_entire_binding()),
        (9, run.node_cache_buf.as_entire_binding()),
        (
            16,
            wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                buffer: &run.args_buf,
                offset: u64::from(at) * 4,
                size: std::num::NonZeroU64::new(u64::from(raster::ARGS_WORDS) * 4),
            }),
        ),
    ]
}

/// The pipeline the renderer builds for the compute pass `id`.
fn pipeline(gpu: &Gpu, id: u32) -> wgpu::ComputePipeline {
    let (shader, entry) = compute(id);
    gpu.pipeline(src(shader), entry)
}

/// An eye over a built world: where it stands, where it looks, and the zooms it asks.
struct EyeCase {
    name: &'static str,
    origin: [i32; 3],
    look: [f32; 3],
    tan: [f32; 2],
    far: f32,
    range: [Zoom; 2],
}

/// The screen an eye's holes lie on: its size, the jitter of its pixels' rays, and how many of
/// its pixels the cast marks as holes, one in `marked_in`. Where `samples`, the frame casts its
/// samples first, and their hits set the entries before the holes' hits do.
struct Screen {
    name: &'static str,
    size: [u32; 2],
    jitter: [f32; 2],
    marked_in: u32,
    samples: bool,
}

/// A light's box over a built world (`LightBox`).
struct LightCase {
    name: &'static str,
    origin: [i32; 3],
    dir: [f32; 3],
    side: f32,
    back: f32,
    t_max: f32,
    range: [Zoom; 2],
    corner: [i32; 2],
}

/// The eyes and screens the tests stand at: over the ground, over the seam, by a way, and over
/// the ground with more holes than a frame's hits hold (`ray::HITS_A_FRAME`). Two of the frames
/// cast their samples first, and the rest leave the entries to the holes' hits.
fn eye_cases(built: &Built) -> Vec<(EyeCase, Screen)> {
    let over = |g: [i32; 2], up: i32| q8([g[0], g[1], built.top_at(g) + up]);
    let way = built
        .way_points
        .iter()
        .find(|p| p.0 == LEAF_CELL_ZOOM && [0, 3].contains(&p.1) && p.2[0] > 0)
        .expect("a way")
        .2;
    let ground = |name| EyeCase {
        name,
        origin: over([EYE[0] + 90, EYE[1] + 90], 60),
        look: [0.5, 0.4, -0.77],
        tan: [0.3, 0.2],
        far: 2000.0,
        range: [8, FINEST],
    };
    let every = Screen {
        name: "every pixel marked, its samples cast first",
        size: [96, 61],
        jitter: [0.0, 0.0],
        marked_in: 1,
        samples: true,
    };
    vec![
        (ground("the eye, over the ground"), every),
        (
            ground("the eye, over the ground"),
            Screen {
                name: "one in three marked, jittered",
                size: [131, 77],
                jitter: [0.25, -0.375],
                marked_in: 3,
                samples: false,
            },
        ),
        (
            EyeCase {
                name: "the eye, over the seam",
                origin: over([EYE[0] + 280, EYE[1] + 90], 40),
                look: [0.8, 0.2, -0.55],
                tan: [0.4, 0.3],
                far: 2000.0,
                range: [8, FINEST],
            },
            Screen {
                name: "every pixel marked, jittered",
                size: [131, 77],
                jitter: [0.25, -0.375],
                marked_in: 1,
                samples: false,
            },
        ),
        (
            EyeCase {
                name: "the eye, by a way",
                origin: q8([way[0], way[1], built.top_at([way[0], way[1]]) + 12]),
                look: [0.5, -0.3, -0.8],
                tan: [0.3, 0.3],
                far: 600.0,
                range: [LEAF_CELL_ZOOM, FINEST],
            },
            Screen {
                name: "every pixel marked, its samples cast first",
                size: [96, 61],
                jitter: [0.0, 0.0],
                marked_in: 1,
                samples: true,
            },
        ),
        (
            EyeCase {
                name: "the eye, over the ground, near",
                origin: over([EYE[0] + 90, EYE[1] + 90], 30),
                look: [0.5, 0.4, -0.77],
                tan: [0.3, 0.2],
                far: 600.0,
                range: [8, FINEST],
            },
            Screen {
                name: "more pixels marked than a frame's hits",
                size: [768, 432],
                jitter: [0.25, -0.375],
                marked_in: 1,
                samples: false,
            },
        ),
    ]
}

/// The light's boxes the test casts: about the eye, and wide over the clip levels, where many
/// cells fill short of their cube.
fn light_cases(built: &Built) -> Vec<LightCase> {
    let over = |g: [i32; 2], up: i32| q8([g[0], g[1], built.top_at(g) + up]);
    vec![
        LightCase {
            name: "the light, about the eye",
            origin: over([EYE[0] + 90, EYE[1] + 90], 40),
            dir: [0.3, 0.2, -0.93],
            side: 1.0,
            back: 60.0,
            t_max: 400.0,
            range: [8, FINEST],
            corner: [1300, -900],
        },
        LightCase {
            name: "the light, wide over the clip levels",
            origin: over([EYE[0] + 3000, EYE[1] + 3000], 900),
            dir: [0.2, -0.3, -0.93],
            side: 32.0,
            back: 200.0,
            t_max: 4000.0,
            range: [8, FINEST],
            corner: [77, 5],
        },
    ]
}

/// A hit as the record reads it (`beam::HIT_WORDS`). Its node is held as a walk from `origin`
/// holds it. `face` is the face its ray entered (`ray::face`), and `whole` says the ray stopped
/// on the node coarser than it asked: for the eye `record::ray_whole`, for the light
/// `record::light_whole` at its slot's band.
struct Hit {
    node: Node,
    origin: [i32; 3],
    face: u32,
    lacks: bool,
    asset: [u32; 2],
    line: u32,
    px: [u32; 2],
    whole: bool,
}

/// The hits the words `words` hold from word `at` on, `cnt` of them as the node cache's head
/// counts them (`node_cache::HITS_CNT`), and no more than a frame holds. `of` gives a hit's
/// origin and whether it is whole, from its pixel or slot and its node.
fn hits_of(
    words: &[u32],
    at: u32,
    cnt: u32,
    of: impl Fn([u32; 2], &Node) -> ([i32; 3], bool),
) -> Vec<Hit> {
    (0..cnt.min(ray::HITS_A_FRAME))
        .map(|r| {
            let k = at + r * beam::HIT_WORDS;
            let w = |a: u32| words[(k + a) as usize];
            let n = beam::HIT_NODE;
            let node = Node {
                zoom: w(n + beam::NODE_ZOOM),
                corner: [0, 1, 2].map(|a| w(n + beam::NODE_CORNER + a)),
                roots: [0, 1, 2].map(|a| w(n + beam::NODE_ROOTS + a) as i32),
            };
            let px = [w(beam::HIT_PX) & 0xffff, w(beam::HIT_PX) >> 16];
            let (origin, whole) = of(px, &node);
            Hit {
                node,
                origin,
                face: w(beam::HIT_FACE) & !beam::HIT_LACKS,
                lacks: w(beam::HIT_FACE) & beam::HIT_LACKS != 0,
                asset: [w(beam::HIT_ASSET), w(beam::HIT_ASSET + 1)],
                line: w(beam::HIT_LINE),
                px,
                whole,
            }
        })
        .collect()
}

/// The slots of `cache`, in bucket order, whose entries hold the node of hit `h` as the kind
/// of entry the hit takes: a lack's for a lack, and a cell's otherwise. There are none where
/// no slot keeps the node (`node_cache::keeps`).
fn entries_of(cache: &[u32], h: &Hit) -> Vec<u32> {
    let (zoom, idx) = (h.node.zoom, h.node.idx(h.origin));
    if !node_cache::keeps(zoom, idx) {
        return Vec::new();
    }
    let kind = if h.lacks { Kind::Lacks } else { Kind::Cell } as u32;
    let first = node_cache::bucket_of(zoom, idx);
    (first..first + node_cache::BUCKET)
        .filter(|&slot| {
            let e = node_cache::entry_at(slot) as usize;
            let w = e + node_cache::W_NODE as usize;
            holds_node(cache, slot)
                && cache[e + node_cache::W_KIND as usize] & node_cache::KIND_MASK == kind
                && cache[w] == zoom
                && [1, 2, 3].map(|k| cache[w + k] as i32) == idx
        })
        .collect()
}

/// The entries of `entries_of` that hold the node with the hit's answer: every one for a lack,
/// and a cell's with the hit's asset otherwise.
fn holders(cache: &[u32], h: &Hit) -> Vec<u32> {
    let answers = |s: &u32| {
        let a = (node_cache::entry_at(*s) + node_cache::W_ASSET) as usize;
        h.lacks || cache[a..a + 2] == h.asset
    };
    entries_of(cache, h).into_iter().filter(answers).collect()
}

/// The slot of the entry that draws the node of hit `h` (pxl-engine `record`): among its
/// holders, the one that already draws, with a face set, or else the first.
fn drawer(cache: &[u32], h: &Hit) -> Option<u32> {
    let slots = holders(cache, h);
    let draws = |s: &u32| cache[node_cache::state_at(*s) as usize] & record::FACES != 0;
    slots.iter().copied().find(draws).or(slots.first().copied())
}

/// Lays some of the entries the hits find otherwise, as other frames and the other observer
/// leave them, so that the record meets every case its rules part. Each entry a hit finds is
/// chosen once, by its slot. One in eight has the face of the first hit on it entered by the
/// other observer: by the light, which asks no word, where `reads`, and by the eye, which asks,
/// where not. One in sixteen has its node held twice, the entry moved later in the bucket and
/// an entry before it that draws nothing (`twice`), and half of those have that face entered
/// by the other observer first. One in thirty-two is forgotten, as forget.wgsl forgets a slot:
/// its key is cleared. One in thirty-two has its node held twice, and the entry before it
/// holds the node with another answer, which no hit takes.
fn lay_otherwise(cache: &mut [u32], hits: &[Hit], reads: bool) {
    let mut chosen = BTreeSet::new();
    for h in hits {
        let Some(slot) = drawer(cache, h) else {
            continue;
        };
        if !chosen.insert(slot) {
            continue;
        }
        let bit = 1 << h.face;
        let other = if reads {
            bit
        } else {
            bit | bit << record::ASKED_SHIFT
        };
        let state = node_cache::state_at(slot) as usize;
        match mix(slot, 0x5eed) % 32 {
            0..=3 => cache[state] |= other,
            4 => chosen.extend(twice(cache, slot)),
            5 => {
                cache[state] |= other;
                chosen.extend(twice(cache, slot));
            }
            6 => cache[node_cache::key_at(slot) as usize] = 0,
            7 => {
                if let Some(to) = twice(cache, slot) {
                    chosen.insert(to);
                    cache[(node_cache::entry_at(slot) + node_cache::W_ASSET + 1) as usize] ^= 1;
                }
            }
            _ => {}
        }
    }
}

/// Moves the entry of `slot`, with its state and its record, to the first free slot after it
/// in its bucket, and leaves at `slot` an entry that holds the same node and answer and draws
/// nothing. Returns the slot it moved to, or `None` where no slot after it is free and nothing
/// changes.
fn twice(cache: &mut [u32], slot: u32) -> Option<u32> {
    let last = slot | (node_cache::BUCKET - 1);
    let to = (slot + 1..=last).find(|&s| cache[node_cache::key_at(s) as usize] == 0)?;
    cache[node_cache::key_at(to) as usize] = cache[node_cache::key_at(slot) as usize];
    let (from, into) = (node_cache::entry_at(slot), node_cache::entry_at(to));
    cache.copy_within(
        from as usize..(from + node_cache::ENTRY_WORDS) as usize,
        into as usize,
    );
    let (state, moved) = (
        node_cache::state_at(slot) as usize,
        node_cache::state_at(to) as usize,
    );
    cache[moved] = std::mem::take(&mut cache[state]);
    let rec = |s: u32| node_cache::RECS_AT + s * node_cache::R_WORDS;
    let (from, into) = (rec(slot), rec(to));
    for w in 0..node_cache::R_WORDS {
        cache[(into + w) as usize] = std::mem::take(&mut cache[(from + w) as usize]);
    }
    Some(to)
}

/// An ask the eye's hits open (`beam::ASK_WORDS`), with the lines it may be asked from. The
/// first hit to set the asked bit of a face of an entry opens the face's ask, and which of the
/// hits that entered that face comes first is the device's order, so any of their lines may be
/// the ask's.
struct Ask {
    words: [u32; beam::ASK_WORDS as usize],
    lines: BTreeSet<u32>,
}

/// What `rec_hits_cs` leaves where the hits `hits` are written into the node cache `cache`
/// and the raster's args `args`, by pxl-engine `record`; `reads` says the observer reads its
/// faces' words, as the eye does. Each hit finds the entry that draws its node (`drawer`), and
/// as no hit writes a key, every hit on a node finds the entry the pass began with. A hit on
/// an entry with no face set before the pass marks it drawn in the frame's number, so no
/// eviction takes it before a draw (`record::drawn`). A hit that stopped whole sets
/// `record::WHOLE`. Each hit sets its face's bit (`record::FACES`) and, where `reads`, its
/// asked bit (`record::ASKED`). Where `reads`, the first hit to ask a face settles its word: none
/// for a lack, `face::BOX` for a cell short of its cube, and otherwise none until an ask it opens
/// answers, whose target is the entry's word (`record::TO_REC`), counted in the args at
/// `raster::HIT_ASKED_AT` as a dispatch of `raster::FACES_LANES` asks a workgroup. Returns the
/// asks by their targets.
fn rec_want(
    cache: &mut [u32],
    args: &mut [u32],
    hits: &[Hit],
    reads: bool,
    kind_cnt: &mut KindCount,
) -> BTreeMap<u32, Ask> {
    let frame = cache[node_cache::FRAME as usize];
    let found: Vec<Option<u32>> = hits.iter().map(|h| drawer(cache, h)).collect();
    let before: BTreeMap<u32, u32> = found
        .iter()
        .flatten()
        .map(|&s| (s, cache[node_cache::state_at(s) as usize]))
        .collect();
    let at = raster::HIT_ASKED_AT as usize;
    let cap = ray::HITS_A_FRAME;
    let mut asks = BTreeMap::new();
    for (h, slot) in hits.iter().zip(&found) {
        kind_cnt.add("hits", true);
        kind_cnt.add("hits, lacks", h.lacks);
        kind_cnt.add("hits, whole", h.whole);
        kind_cnt.add(
            "hits, short of its cube",
            !h.lacks && answer::fill_eighths(h.asset[0]) < EIGHTHS,
        );
        let Some(slot) = *slot else {
            kind_cnt.add("hits, no slot keeps the node", true);
            continue;
        };
        let held = holders(cache, h);
        kind_cnt.add("hits, the node held twice", held.len() > 1);
        kind_cnt.add(
            "hits, the node held with another answer",
            entries_of(cache, h).len() > held.len(),
        );
        kind_cnt.add(
            "hits, the node held twice, the later draws",
            held[0] != slot,
        );
        let state = node_cache::state_at(slot) as usize;
        let drawn = before[&slot] & (record::FACES | record::ASKED) == 0;
        kind_cnt.add("hits, an entry drawn from now on", drawn);
        if drawn {
            cache[node_cache::drawn_at(slot) as usize] = frame;
        }
        if h.whole {
            cache[state] |= record::WHOLE;
        }
        let bit = 1 << h.face;
        let asked = bit << record::ASKED_SHIFT;
        let mark = if reads { bit | asked } else { bit };
        let was = cache[state];
        kind_cnt.add("hits, a face set already", was & mark == mark);
        kind_cnt.add(
            "hits, a face entered and not asked",
            reads && was & (bit | asked) == bit,
        );
        if was & mark == mark {
            continue;
        }
        cache[state] |= mark;
        if !reads || was & asked != 0 {
            continue;
        }
        let to = node_cache::face_at(slot, h.face);
        let short = !h.lacks && answer::fill_eighths(h.asset[0]) < EIGHTHS;
        cache[to as usize] = if short { face::BOX } else { 0 };
        if h.lacks || short {
            continue;
        }
        let idx = args[at + raster::ARGS_CNT as usize];
        args[at + raster::ARGS_CNT as usize] += 1;
        if idx >= cap {
            continue;
        }
        let instances = &mut args[at + raster::ARGS_INSTANCES as usize];
        *instances = (*instances).max((idx + 1).min(cap));
        if idx.is_multiple_of(raster::FACES_LANES) {
            let groups = (idx + 1)
                .min(cap)
                .div_ceil(raster::FACES_LANES)
                .min(WORKGROUPS_CAP);
            let faces = &mut args[at + raster::ARGS_FACES as usize];
            *faces = (*faces).max(groups);
        }
        let mut words = [0; beam::ASK_WORDS as usize];
        words[beam::ASK_NODE as usize] = node_cache::entry_at(slot);
        words[beam::ASK_FACE as usize] = h.face;
        words[beam::ASK_LINE as usize] = h.line;
        words[beam::ASK_TO as usize] = record::TO_REC | to;
        asks.insert(
            to,
            Ask {
                words,
                lines: BTreeSet::new(),
            },
        );
    }
    for (h, slot) in hits.iter().zip(&found) {
        if let Some(ask) = slot.and_then(|s| asks.get_mut(&node_cache::face_at(s, h.face))) {
            ask.lines.insert(h.line);
        }
    }
    kind_cnt.count("asks", asks.len() as u64);
    kind_cnt.count(
        "asks, from either of two lines",
        asks.values().filter(|a| a.lines.len() > 1).count() as u64,
    );
    asks
}

/// What `rec_faces_cs` leaves in the rays `rays`, laid out as `read_texture` reads them,
/// `size` pixels a layer. `hits` are the eye's hits, and `cache` is the node cache once the
/// faces' pass has answered their asks, whose targets are `asked`. A pixel whose last ray word
/// (`ray::FACES`) holds `beam::HIT_WAITS` and the number of its hit takes the word its hit's
/// entry holds for the hit's face (`node_cache::face_at`), or 0 where no slot keeps the node.
/// No other word changes.
fn faces_want(
    rays: &mut [u32],
    size: [u32; 2],
    cache: &[u32],
    (hits, asked): (&[Hit], &BTreeSet<u32>),
    kind_cnt: &mut KindCount,
) {
    let texels = (size[0] * size[1]) as usize;
    for (r, h) in hits.iter().enumerate() {
        let k = ray::FACES as usize * texels + (h.px[1] * size[0] + h.px[0]) as usize;
        let waits = rays[k] == beam::HIT_WAITS | r as u32;
        kind_cnt.add("hits, their pixel waits", waits);
        kind_cnt.add("hits, their pixel waits no longer", !waits);
        if !waits {
            continue;
        }
        let Some(slot) = drawer(cache, h) else {
            kind_cnt.add("pixels, no slot keeps the node", true);
            rays[k] = 0;
            continue;
        };
        let to = node_cache::face_at(slot, h.face);
        rays[k] = cache[to as usize];
        kind_cnt.add("pixels, the word asked this frame", asked.contains(&to));
        kind_cnt.add("pixels, the word asked before", !asked.contains(&to));
    }
}

/// A word of the node cache, named for a message.
fn cache_word(k: usize) -> String {
    use node_cache::{
        ENTRIES_AT, ENTRY_WORDS, KEYS_AT, RECS_AT, R_DRAWN, R_FACES, R_WORDS, STATES_AT,
    };
    let k = k as u32;
    if k < KEYS_AT {
        format!("head word {k}")
    } else if k < STATES_AT {
        format!("slot {}'s key", k - KEYS_AT)
    } else if k < ENTRIES_AT {
        format!("slot {}'s state", k - STATES_AT)
    } else if k < RECS_AT {
        let w = k - ENTRIES_AT;
        format!("slot {}'s entry word {}", w / ENTRY_WORDS, w % ENTRY_WORDS)
    } else {
        let (slot, w) = ((k - RECS_AT) / R_WORDS, (k - RECS_AT) % R_WORDS);
        if w == R_DRAWN {
            format!("slot {slot}'s drawn frame")
        } else {
            format!("slot {slot}'s face {} word", w - R_FACES)
        }
    }
}

/// An observer's frame up to its record of its holes' hits: the hits, and the words the
/// record's passes run over, each kept as laid and bound on the device. `words` are the eye's
/// beam rays' words or the light's list, and `asks_at` the first word of the asks its hits
/// open. `rays` are the eye's rays or the light's map, `size` texels a layer. Only the light has
/// a head (`header_buf`).
struct Run {
    name: String,
    reads: bool,
    hits: Vec<Hit>,
    cache: Vec<u32>,
    words: Vec<u32>,
    args: Vec<u32>,
    asks_at: u32,
    size: [u32; 2],
    eye_buf: wgpu::Buffer,
    header_buf: Option<wgpu::Buffer>,
    words_buf: wgpu::Buffer,
    node_cache_buf: wgpu::Buffer,
    args_buf: wgpu::Buffer,
    rays: wgpu::Texture,
}

impl Run {
    /// The bindings of `rec_hits_cs`: the eye at 0, the observer's words at 8, the node cache
    /// at 9 and the raster's args at 16, and the light's head at 11 for the light.
    fn rec_binds(&self) -> Vec<(u32, wgpu::BindingResource<'_>)> {
        let mut binds = vec![
            (0, self.eye_buf.as_entire_binding()),
            (8, self.words_buf.as_entire_binding()),
            (9, self.node_cache_buf.as_entire_binding()),
            (16, self.args_buf.as_entire_binding()),
        ];
        if let Some(header) = &self.header_buf {
            binds.push((11, header.as_entire_binding()));
        }
        binds
    }
}

/// The eye's words for `c`, laid out as pxl-engine eye.rs places them, and its grid.
fn eye_of(c: &EyeCase) -> (Vec<u32>, Grid) {
    let forward = unit(c.look);
    let right = unit(cross(forward, [0.0, 0.0, 1.0]));
    let up = cross(right, forward);
    let grid = Grid::of(c.tan);
    let mut words = vec![0u32; eye::WORDS];
    for k in 0..3 {
        words[eye::ORIGIN + k] = c.origin[k] as u32;
        words[eye::FORWARD + k] = forward[k].to_bits();
        words[eye::RIGHT + k] = right[k].to_bits();
        words[eye::UP + k] = up[k].to_bits();
    }
    words[eye::FAR] = c.far.to_bits();
    words[eye::TAN] = c.tan[0].to_bits();
    words[eye::TAN + 1] = c.tan[1].to_bits();
    words[eye::RANGE] = c.range[0];
    words[eye::RANGE + 1] = c.range[1];
    words[eye::GRID..eye::GRID + eye::GRID_WORDS].copy_from_slice(&grid.words());
    for at in [eye::CARVE, eye::HIDE] {
        words[at..at + 3].copy_from_slice(&Cut::NONE.at.map(|v| v as u32));
        words[at + 3] = Cut::NONE.zoom;
    }
    (words, grid)
}

/// The raster's args as the renderer lays them in a frame that casts its samples
/// (ray/raster.rs `prepare`): the draw's, then the faces' asks the samples open at
/// `raster::ASKED_AT` and those the holes' hits open at `raster::HIT_ASKED_AT`, none opened
/// yet.
fn args_of(grid: &Grid) -> Vec<u32> {
    let mut args = vec![0u32; raster::WORDS as usize];
    args[raster::ARGS_VERTICES as usize] = raster::VERTICES;
    for (at, asks) in [
        (raster::ASKED_AT, grid.asks_at),
        (raster::HIT_ASKED_AT, grid.hit_asks_at),
    ] {
        let at = at as usize;
        args[at + raster::ARGS_FACES as usize + 1] = 1;
        args[at + raster::ARGS_FACES as usize + 2] = 1;
        args[at + raster::ARGS_ASKS as usize] = asks;
    }
    args
}

/// Runs the eye's frame of case `c` on screen `s` up to the record of its holes' hits, in the
/// order the renderer runs it (ray/cast.rs): the beam rays and the cells' starts; where
/// `s.samples`, the samples, their hits' record and the faces' pass over their asks; and then
/// the holes' queue over the pixels the cast marked, as tests/sample.rs marks them. The
/// entries the hits find are then laid otherwise (`lay_otherwise`).
fn eye_run(gpu: &Gpu, b: &WorldBuffer, (c, s): (&EyeCase, &Screen)) -> Run {
    let (mut eye, grid) = eye_of(c);
    for k in 0..2 {
        eye[eye::JITTER + k] = s.jitter[k].to_bits();
    }
    let eye_buf = gpu.buf(&eye, wgpu::BufferUsages::UNIFORM);
    let mut node_cache = vec![0u32; node_cache::WORDS as usize];
    node_cache[node_cache::FRAME as usize] = FRAME;
    let node_cache_buf = gpu.buf(&node_cache, store_of());
    let args_buf = gpu.buf(&args_of(&grid), store_of());
    let beams = gpu.buf(&vec![0u32; grid.size() as usize], store_of());
    gpu.corners(
        false,
        &world_binds(b, &eye_buf, &beams, &node_cache_buf),
        &node_cache_buf,
    );
    gpu.dispatch(
        &gpu.pipeline(src(SHADER_CELL), "cell_cs"),
        &[
            (0, eye_buf.as_entire_binding()),
            (8, beams.as_entire_binding()),
        ],
        [
            grid.span[0].div_ceil(WORKGROUP),
            grid.span[1].div_ceil(WORKGROUP),
            1,
        ],
    );
    if s.samples {
        gpu.samples(
            false,
            &world_binds(b, &eye_buf, &beams, &node_cache_buf),
            &node_cache_buf,
        );
        gpu.dispatch(
            &gpu.pipeline(src(SHADER_SAMPLE_HIT), "sample_hit_cs"),
            &[
                (0, eye_buf.as_entire_binding()),
                (8, beams.as_entire_binding()),
                (9, node_cache_buf.as_entire_binding()),
                (16, args_buf.as_entire_binding()),
            ],
            [
                grid.samples[0].div_ceil(SQUARE_SIDE),
                grid.samples[1].div_ceil(SQUARE_SIDE),
                1,
            ],
        );
    }

    // The eye's holes lie past the grid's words, and the pixels the cast marked are listed
    // there. A marked pixel's first ray word is one no walk writes.
    let mut words = gpu.read(&beams);
    let walked = grid.walked_at() as usize;
    words.resize(walked + ray::holes_words(s.size) as usize, 0);
    let [wd, ht] = s.size;
    let texels = (wd * ht) as usize;
    let marked: Vec<u32> = (0..wd * ht)
        .filter(|&p| (p.wrapping_mul(0x9e37_79b9) >> 7).is_multiple_of(s.marked_in))
        .collect();
    let holes = walked + ray::holes_at(s.size) as usize;
    words[holes] = marked.len() as u32;
    for (k, &p) in marked.iter().enumerate() {
        words[holes + 1 + k] = (p % wd) | ((p / wd) << 16);
    }
    // No word laid sets `beam::HIT_WAITS`, so a pixel waits for a hit only where the queue
    // says so.
    let mut ray_words: Vec<u32> = (0..ray::WORDS as usize * texels)
        .map(|k| (k as u32).wrapping_mul(0x2c1b_3c6d) >> 2)
        .collect();
    for &p in &marked {
        ray_words[p as usize] = UNWALKED;
    }
    let rays = texture(
        &gpu.device,
        &gpu.queue,
        s.size,
        ray::WORDS,
        wgpu::TextureUsages::STORAGE_BINDING,
        &ray_words,
    );
    let words_buf = gpu.buf(&words, store_of());
    let run = Run {
        name: format!("{}, {}", c.name, s.name),
        reads: true,
        hits: Vec::new(),
        cache: Vec::new(),
        words: Vec::new(),
        args: Vec::new(),
        asks_at: grid.hit_asks_at,
        size: s.size,
        header_buf: None,
        eye_buf,
        words_buf,
        node_cache_buf,
        args_buf,
        rays,
    };
    // The faces' pass answers the samples' asks before the holes are walked. Where the
    // samples are not cast, their args count no ask.
    let laid = gpu.read(&run.args_buf);
    gpu.dispatch(
        &pipeline(gpu, COMPUTE_FACE),
        &face_binds(b, &run, raster::ASKED_AT),
        [laid[(raster::ASKED_AT + raster::ARGS_FACES) as usize], 1, 1],
    );
    let view = array_view(&run.rays);
    let mut binds = world_binds(b, &run.eye_buf, &run.words_buf, &run.node_cache_buf);
    binds.push((3, wgpu::BindingResource::TextureView(&view)));
    gpu.holes(false, &binds, &run.node_cache_buf);
    drop(binds);

    let origin = c.origin;
    let range = c.range;
    let words = gpu.read(&run.words_buf);
    let mut cache = gpu.read(&run.node_cache_buf);
    let hits = hits_of(
        &words,
        grid.words()[beam::GRID_HITS_AT],
        cache[node_cache::HITS_CNT as usize],
        |_, n| (origin, record::ray_whole(origin, n, range)),
    );
    lay_otherwise(&mut cache, &hits, true);
    gpu.queue
        .write_buffer(&run.node_cache_buf, 0, gpu::bytes(&cache));
    Run {
        hits,
        cache,
        words,
        args: gpu.read(&run.args_buf),
        ..run
    }
}

/// Lays the light's beam lines and each cell's start in its list `list`, as the cast lays them
/// (beam.light.wgsl, light_cell.wgsl). Every tile is read, and the beam runs over a node cache
/// of its own.
fn lay_cells(gpu: &Gpu, b: &WorldBuffer, [eye, header]: [&wgpu::Buffer; 2], list: &mut [u32]) {
    let tiles = light::tiles(SIZE) as usize;
    let reads = light::reads_at(SIZE) as usize;
    list[reads..reads + tiles * tiles].fill(CAST);
    let mut node_cache = vec![0u32; node_cache::WORDS as usize];
    node_cache[node_cache::FRAME as usize] = FRAME;
    let node_cache = gpu.buf(&node_cache, store_of());
    let buf = gpu.buf(list, store_of());
    let mut binds = world_binds(b, eye, &buf, &node_cache);
    binds.push((11, header.as_entire_binding()));
    gpu.light_beam(&binds, &buf, &node_cache, SIZE);
    let cells = light::cells_across(SIZE);
    gpu.dispatch(
        &gpu.pipeline(src(SHADER_LIGHT_CELL), "light_cell_cs"),
        &[
            (0, eye.as_entire_binding()),
            (8, buf.as_entire_binding()),
            (11, header.as_entire_binding()),
        ],
        [cells.div_ceil(WORKGROUP), cells.div_ceil(WORKGROUP), 1],
    );
    // The beam's lines and hands and the cells' starts all lie before the holes' list
    // (`light::holes_at`), so the copy leaves the listed slots as they were.
    let laid = light::holes_at(SIZE) as usize;
    list[..laid].copy_from_slice(&gpu.read(&buf)[..laid]);
}

/// Runs the light's frame of case `c` up to the record of its holes' hits: its beam and its
/// cells' starts, then the holes' queue over every slot of its map, over a node cache that
/// holds nothing yet, as tests/light_miss.rs runs them. The entries the hits find are then
/// laid otherwise (`lay_otherwise`).
fn light_run(gpu: &Gpu, b: &WorldBuffer, c: &LightCase) -> Run {
    let (l, starts): (Light, StartGrid) = LightBox {
        size: SIZE,
        origin: c.origin,
        dir: c.dir,
        side: c.side,
        back: c.back,
        t_max: c.t_max,
        corner: c.corner,
        reach_ndc: 1.0,
    }
    .lay();
    let mut eye = vec![0u32; eye::WORDS];
    eye[eye::LIGHT..eye::LIGHT + eye::LIGHT_WORDS].copy_from_slice(&l.words());
    eye[eye::RANGE] = c.range[0];
    eye[eye::RANGE + 1] = c.range[1];
    for at in [eye::CARVE, eye::HIDE] {
        eye[at..at + 3].copy_from_slice(&Cut::NONE.at.map(|v| v as u32));
        eye[at + 3] = Cut::NONE.zoom;
    }
    let mut header = vec![0u32; light::HEADER_WORDS];
    header[light::HEADER_CAST] = CAST;
    header[light::HEADER_EPOCH] = CAST;
    for (k, v) in light::header_layout(SIZE) {
        header[k] = v;
    }
    header[light::STARTS_AT * 4..(light::STARTS_AT + light::STARTS_VEC4S) * 4]
        .copy_from_slice(&starts.words());
    let eye_buf = gpu.buf(&eye, wgpu::BufferUsages::UNIFORM);
    let header_buf = gpu.buf(&header, wgpu::BufferUsages::UNIFORM);

    // Every slot of the map is listed, row after row, in the holes' list the queue takes.
    let mut list = vec![0u32; light::buf_words(SIZE) as usize];
    let holes = light::holes_at(SIZE) as usize;
    list[holes] = SIZE * SIZE;
    for k in 0..SIZE * SIZE {
        list[holes + 1 + k as usize] = (k % SIZE) | ((k / SIZE) << 16);
    }
    lay_cells(gpu, b, [&eye_buf, &header_buf], &mut list);
    let mut node_cache = vec![0u32; node_cache::WORDS as usize];
    node_cache[node_cache::FRAME as usize] = FRAME;
    // The light's raster's args, as the renderer lays them (ray/raster.rs `light_prepare`).
    let mut args = vec![0u32; raster::ARGS_WORDS as usize];
    args[raster::ARGS_VERTICES as usize] = raster::VERTICES;
    let run = Run {
        name: c.name.to_string(),
        reads: false,
        hits: Vec::new(),
        cache: Vec::new(),
        words: Vec::new(),
        args: args.clone(),
        asks_at: 0,
        size: [SIZE; 2],
        words_buf: gpu.buf(&list, store_of()),
        node_cache_buf: gpu.buf(&node_cache, store_of()),
        args_buf: gpu.buf(&args, store_of()),
        rays: texture(
            &gpu.device,
            &gpu.queue,
            [SIZE; 2],
            1,
            wgpu::TextureUsages::STORAGE_BINDING,
            &vec![0; (SIZE * SIZE) as usize],
        ),
        eye_buf,
        header_buf: Some(header_buf),
    };
    let view = array_view(&run.rays);
    let mut binds = world_binds(b, &run.eye_buf, &run.words_buf, &run.node_cache_buf);
    binds.push((3, wgpu::BindingResource::TextureView(&view)));
    binds.extend(run.header_buf.iter().map(|h| (11, h.as_entire_binding())));
    gpu.holes(true, &binds, &run.node_cache_buf);
    drop(binds);

    // A hit's node is held from where its slot's ray starts, and the hit is whole where the
    // node is coarser than its slot's band.
    let words = gpu.read(&run.words_buf);
    let mut cache = gpu.read(&run.node_cache_buf);
    let hits = hits_of(
        &words,
        light::hits_at(SIZE),
        cache[node_cache::HITS_CNT as usize],
        |slot, n| {
            let p = l.texel(SIZE, slot);
            let tile = [(p[0] / light::TILE) as i32, (p[1] / light::TILE) as i32];
            let band = l.band(SIZE, c.range, tile);
            (
                light::ray(&l, &starts, p).0,
                record::light_whole(n.zoom, band),
            )
        },
    );
    lay_otherwise(&mut cache, &hits, false);
    gpu.queue
        .write_buffer(&run.node_cache_buf, 0, gpu::bytes(&cache));
    Run {
        hits,
        cache,
        words,
        ..run
    }
}

/// Runs `rec_hits_cs` over the words of `run`, a thread for each hit a frame holds, and
/// compares every word it leaves in the node cache, the raster's args and the observer's words
/// with `rec_want`'s. The asks it laid are put in the order of their targets first, and an
/// ask's line may be that of any hit that may open it (`Ask`).
fn rec_case(
    gpu: &Gpu,
    p: &wgpu::ComputePipeline,
    run: &Run,
    tally: &mut Tally,
    kind_cnt: &mut KindCount,
) {
    let mut cache = run.cache.clone();
    let mut args = run.args.clone();
    let asks = rec_want(&mut cache, &mut args, &run.hits, run.reads, kind_cnt);
    gpu.dispatch(
        p,
        &run.rec_binds(),
        [ray::HITS_A_FRAME.div_ceil(beam::HIT_LANES), 1, 1],
    );
    let name = &run.name;
    eprintln!("{name}: {} hits, {} asks", run.hits.len(), asks.len());
    tally.compare(
        &format!("{name}, the node cache"),
        &cache,
        &gpu.read(&run.node_cache_buf),
        cache_word,
    );
    tally.compare(
        &format!("{name}, the args"),
        &args,
        &gpu.read(&run.args_buf),
        |k| format!("word {k}"),
    );
    let n = beam::ASK_WORDS as usize;
    let at = run.asks_at as usize;
    let mut want = run.words.clone();
    let mut lines = Vec::new();
    for (place, ask) in asks.values().enumerate() {
        want[at + place * n..at + (place + 1) * n].copy_from_slice(&ask.words);
        lines.push(&ask.lines);
    }
    let mut got = gpu.read(&run.words_buf);
    let laid = &mut got[at..at + asks.len() * n];
    let sorted = {
        let mut each: Vec<&[u32]> = laid.chunks(n).collect();
        each.sort_by_key(|a| a[beam::ASK_TO as usize]);
        each.concat()
    };
    laid.copy_from_slice(&sorted);
    let ask_of = |k: usize| {
        (at..at + lines.len() * n)
            .contains(&k)
            .then(|| (k - at) / n)
    };
    tally.compare_by(
        &format!("{name}, the observer's words"),
        &want,
        &got,
        |k| match ask_of(k) {
            Some(place) => format!("ask {place}'s word {}", (k - at) % n),
            None => format!("word {k}"),
        },
        |k, w, h| {
            w == h
                || ask_of(k).is_some_and(|place| {
                    (k - at) % n == beam::ASK_LINE as usize && lines[place].contains(&h)
                })
        },
    );
}

/// The eye's record of its holes' hits must leave every word as pxl-engine `record` says,
/// over the eye's frames of `eye_cases`.
#[test]
fn hand_eye_hits_answer_as_rec() {
    let Some(gpu) = device() else { return };
    let built = real_world::build(EYE);
    let b = world_bufs(&gpu, &built);
    let p = pipeline(&gpu, COMPUTE_REC_HITS);
    let mut tally = Tally::default();
    let mut kind_cnt = KindCount::new();
    for (c, s) in eye_cases(&built) {
        let run = eye_run(&gpu, &b, (&c, &s));
        rec_case(&gpu, &p, &run, &mut tally, &mut kind_cnt);
    }
    tally.end("the eye's hits");
    kind_cnt.hold(&[
        "hits",
        "hits, lacks",
        "hits, whole",
        "hits, short of its cube",
        "hits, no slot keeps the node",
        "hits, the node held twice",
        "hits, the node held twice, the later draws",
        "hits, the node held with another answer",
        "hits, an entry drawn from now on",
        "hits, a face set already",
        "hits, a face entered and not asked",
        "asks",
        "asks, from either of two lines",
    ]);
}

/// The light's record of its holes' hits must leave every word as pxl-engine `record` says,
/// over the light's boxes of `light_cases`. It opens no ask and writes no args, since the
/// light's slots read no face's word.
#[test]
fn hand_light_hits_answer_as_rec() {
    let Some(gpu) = device() else { return };
    let built = real_world::build(EYE);
    let b = world_bufs(&gpu, &built);
    let p = pipeline(&gpu, COMPUTE_LIGHT_REC_HITS);
    let mut tally = Tally::default();
    let mut kind_cnt = KindCount::new();
    for c in light_cases(&built) {
        let run = light_run(&gpu, &b, &c);
        rec_case(&gpu, &p, &run, &mut tally, &mut kind_cnt);
    }
    tally.end("the light's hits");
    kind_cnt.hold(&[
        "hits",
        "hits, whole",
        "hits, short of its cube",
        "hits, no slot keeps the node",
        "hits, the node held twice",
        "hits, the node held twice, the later draws",
        "hits, the node held with another answer",
        "hits, an entry drawn from now on",
        "hits, a face set already",
    ]);
}

/// Once the eye's hits are recorded and the faces' pass has answered their asks, as the
/// renderer runs them (ray/raster.rs `rec_hits`), `rec_faces_cs` must leave every
/// word of the rays as `faces_want` says, and no word of the node cache or of the eye's words
/// changed.
#[test]
fn hand_hit_faces_answer_as_rec() {
    let Some(gpu) = device() else { return };
    let built = real_world::build(EYE);
    let b = world_bufs(&gpu, &built);
    let [hits_p, face_p, faces_p] =
        [COMPUTE_REC_HITS, COMPUTE_FACE, COMPUTE_REC_FACES].map(|id| pipeline(&gpu, id));
    let mut tally = Tally::default();
    let mut kind_cnt = KindCount::new();
    for (c, s) in eye_cases(&built) {
        let run = eye_run(&gpu, &b, (&c, &s));
        gpu.dispatch(
            &hits_p,
            &run.rec_binds(),
            [ray::HITS_A_FRAME.div_ceil(beam::HIT_LANES), 1, 1],
        );
        let args = gpu.read(&run.args_buf);
        let at = raster::HIT_ASKED_AT as usize;
        gpu.dispatch(
            &face_p,
            &face_binds(&b, &run, raster::HIT_ASKED_AT),
            [args[at + raster::ARGS_FACES as usize], 1, 1],
        );
        let cache = gpu.read(&run.node_cache_buf);
        let words = gpu.read(&run.words_buf);
        let asked: BTreeSet<u32> = (0..args[at + raster::ARGS_INSTANCES as usize])
            .map(|place| {
                let k = (run.asks_at + place * beam::ASK_WORDS + beam::ASK_TO) as usize;
                words[k] & !record::TO_REC
            })
            .collect();
        let mut want = read_texture(&gpu.device, &gpu.queue, &run.rays);
        faces_want(
            &mut want,
            run.size,
            &cache,
            (&run.hits, &asked),
            &mut kind_cnt,
        );
        let view = array_view(&run.rays);
        gpu.dispatch(
            &faces_p,
            &[
                (0, run.eye_buf.as_entire_binding()),
                (3, wgpu::BindingResource::TextureView(&view)),
                (8, run.words_buf.as_entire_binding()),
                (9, run.node_cache_buf.as_entire_binding()),
            ],
            [ray::HITS_A_FRAME.div_ceil(beam::HIT_LANES), 1, 1],
        );
        let name = &run.name;
        let texels = (run.size[0] * run.size[1]) as usize;
        tally.compare(
            &format!("{name}, the rays"),
            &want,
            &read_texture(&gpu.device, &gpu.queue, &run.rays),
            |k| {
                let p = k % texels;
                let px = [p % run.size[0] as usize, p / run.size[0] as usize];
                format!("pixel {px:?}'s word {}", k / texels)
            },
        );
        tally.compare(
            &format!("{name}, the node cache"),
            &cache,
            &gpu.read(&run.node_cache_buf),
            cache_word,
        );
        tally.compare(
            &format!("{name}, the eye's words"),
            &words,
            &gpu.read(&run.words_buf),
            |k| format!("word {k}"),
        );
    }
    tally.end("the hits' faces");
    kind_cnt.hold(&[
        "hits, their pixel waits",
        "pixels, no slot keeps the node",
        "pixels, the word asked this frame",
        "pixels, the word asked before",
    ]);
}
