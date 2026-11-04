//! The ways' fill by hand (frame/shader/way.wgsl, way_free.wgsl, way_tree.wgsl,
//! way_return.wgsl) against pxl-engine's: bricks of a real world handed as a frame hands
//! them (tests/real_world), at every eye clip level, the service area's edge and its notch
//! walling some, their spans word for word (`fill::spans`); their trees by shape and leaf
//! through each root against `fill::tree`, child groups being taken in whatever order the
//! workgroups pop them; their records laid as the CPU's mirror lays them
//! (`way::tree::lay`), every live tree read again there each frame; what each build gave
//! back; and the free stack's count and ids after each frame. Frames as the renderer runs
//! them: one pool and stack across them, slots handed again over their old trees, a build
//! that ran out of the pool or its scratch handed again once either has grown, the pool
//! grown as the host grows it; a brick past its own most and one whose ways span more of
//! its nodes along z than its root holds. Skipped where no adapter answers.

mod real_world;

use std::collections::{BTreeMap, BTreeSet};
use std::time::Duration;

use pxl_engine::area::Shard;
use pxl_engine::clipmap::{
    self as clip, ACROSS, BRICK, BRICK_CELLS, BRICK_SHIFT, BRICK_WORDS, ENTRIES, FILL_CAP,
    FILL_WORDS, FIRST_CELL, HANDED_WORDS, LEVELS, LINES_AT, LINES_ENTRIES_AT, LINES_SEGMENTS_AT,
    PALETTE_AT, ROOT_AT, SPANS_AT, SPANS_COLUMNS, SPANS_COLUMN_WORDS,
};
use pxl_engine::overwrite::{Columns, BRANCH, CHILDREN};
use pxl_engine::way::{self, tree, Segment};
use pxl_engine::{fill, ground, value, wrap_idx, Zoom, LEAF_CELL};
use pxl_render::frame::shader::record::{
    self, SHADER_WAY, SHADER_WAY_FREE, SHADER_WAY_RETURN, SHADER_WAY_TREE,
};
use real_world::{level_first, mix, World, EYE, NOTCH};

/// Slots of the test's ground: fewer than the bricks handed, so a slot is filled again
/// over the tree it holds.
const SLOTS: u32 = 48;

/// An eye at the notch's west edge, where leaf bricks are walled.
const EYE_NOTCH: [i32; 2] = [(NOTCH[0] << 16) - 40, (NOTCH[1] << 16) + 900];

/// The pool's and each brick's scratch's child groups at first: small, so builds run out of
/// both and they grow, as the renderer's do.
const POOL_FIRST: u32 = 1024;
const ROOM_FIRST: u32 = 48;
/// The pool at its most here, and the mirror's room.
const POOL_MOST: u32 = 64 * POOL_FIRST;

fn bytes(words: &[u32]) -> &[u8] {
    // SAFETY: every u32's bytes are u8s, in the slice's own lifetime.
    unsafe { std::slice::from_raw_parts(words.as_ptr().cast(), words.len() * 4) }
}

/// A brick a frame hands: its record and its cells' zoom, and what it is named by.
#[derive(Clone)]
struct Hand {
    fresh: Vec<u32>,
    cell: Zoom,
    shard: Shard,
    name: String,
}

/// Brick `q` of the clip level at `cell` about `eye` as the kernel hands it (pxl-kernel
/// domain/layer/terrain/clipmap.rs `fill`): its palette indices, samples and lines
/// (`real_world::handed`), some cells no ground's, its shard as the area knows the brick,
/// `more` segments after its lines, and each bin holding the segments that reach it.
fn hand(
    eye: [i32; 2],
    area: &World,
    (cell, q): (Zoom, [i32; 2]),
    lined: bool,
    more: &[[u32; way::SEGMENT_WORDS]],
) -> Hand {
    let mut fresh = vec![0u32; HANDED_WORDS as usize];
    real_world::handed(eye, &mut fresh, cell, q, lined, &mut Vec::new());
    let place = [(q[0] & 7) as u32, (q[1] & 7) as u32];
    let first = [wrap_idx(cell, q[0] << BRICK_SHIFT), q[1] << BRICK_SHIFT];
    let shard = Shard::UNKNOWN.at(area, (clip::bricks_zoom(cell), q));
    fresh[..FILL_WORDS as usize].copy_from_slice(&clip::fill_words(
        0,
        clip::place_word(place),
        cell,
        first,
        shard,
    ));
    // Some cells no ground: their four void.
    for i in 0..BRICK_CELLS {
        if mix(cell + 1000, [q[0] + i as i32, q[1]]).is_multiple_of(29) {
            fresh[(PALETTE_AT + i) as usize] = 0;
        }
    }
    // `handed` lays every segment in every bin.
    let n = fresh[(LINES_AT + way::HEADER_WORDS) as usize] >> 16;
    for (k, w) in more.iter().enumerate() {
        let at = (LINES_AT + LINES_SEGMENTS_AT) as usize + (n as usize + k) * way::SEGMENT_WORDS;
        fresh[at..at + way::SEGMENT_WORDS].copy_from_slice(w);
    }
    bins(&mut fresh, n + more.len() as u32, place);
    Hand {
        fresh,
        cell,
        shard,
        name: format!("cell {cell} brick {q:?}"),
    }
}

/// The record's `n` segments binned as a bake bins a brick's (`clipmap::LINES_ENTRIES_AT`):
/// each bin the segments whose footprint (`way::footprint`) reaches its cells or the ring's
/// about them, in their order.
fn bins(fresh: &mut [u32], n: u32, place: [u32; 2]) {
    let side = BRICK >> clip::WAYS_SHIFT;
    let cells = 1 << clip::WAYS_SHIFT;
    let mut entries = Vec::new();
    for b in 0..clip::WAYS_BINS {
        let [bx, by] = [b % side, b / side];
        let lo = [
            (place[0] * BRICK + bx * cells) as f32 - 1.0,
            (place[1] * BRICK + by * cells) as f32 - 1.0,
        ];
        let hi = [lo[0] + cells as f32 + 2.0, lo[1] + cells as f32 + 2.0];
        let first = entries.len() as u32;
        for k in 0..n {
            let s = Segment::of(segment(fresh, k as usize));
            let reach = way::footprint(s.width);
            let reaches = (0..2).all(|i| {
                s.a[i].min(s.b[i]) - reach <= hi[i] && s.a[i].max(s.b[i]) + reach >= lo[i]
            });
            if reaches {
                entries.push(k);
            }
        }
        let cnt = entries.len() as u32 - first;
        fresh[(LINES_AT + way::HEADER_WORDS + b) as usize] = first | cnt << 16;
    }
    assert!(entries.len() <= clip::WAYS_BRICK_SEGMENTS as usize);
    for (e, &k) in entries.iter().enumerate() {
        let at = (LINES_AT + LINES_ENTRIES_AT) as usize + e / 2;
        fresh[at] = fresh[at] & !(0xffff << (16 * (e % 2))) | k << (16 * (e % 2));
    }
}

/// The record's `k`-th segment.
fn segment(fresh: &[u32], k: usize) -> [u32; way::SEGMENT_WORDS] {
    let at = (LINES_AT + LINES_SEGMENTS_AT) as usize + k * way::SEGMENT_WORDS;
    fresh[at..at + way::SEGMENT_WORDS].try_into().unwrap()
}

/// A line of two segments over brick `q`, from `from` to `to` in cells from its first, a
/// joint midway: its form, its surface at its ends in leaf cells, its width in eighths, its
/// kind and its clear height.
#[expect(clippy::too_many_arguments, reason = "a line's words, each its own")]
fn line(
    q: [i32; 2],
    form: u32,
    from: [i32; 2],
    to: [i32; 2],
    tops: [i16; 2],
    width: u8,
    kind: u32,
    clear: u8,
) -> [[u32; way::SEGMENT_WORDS]; 2] {
    let first = [(q[0] & 7) * 64, (q[1] & 7) * 64];
    let at = |p: [i32; 2]| {
        [
            ((first[0] + p[0]) * 8) as i16,
            ((first[1] + p[1]) * 8) as i16,
        ]
    };
    let (a, b) = (at(from), at(to));
    let m = [
        ((a[0] as i32 + b[0] as i32) / 2) as i16,
        ((a[1] as i32 + b[1] as i32) / 2) as i16,
    ];
    let mid = ((tops[0] as i32 + tops[1] as i32) / 2) as i16;
    [
        Segment::words(
            [a, m],
            [tops[0], mid],
            [
                width,
                kind as u8,
                way::flags(form, true, false, false),
                clear,
            ],
        ),
        Segment::words(
            [m, b],
            [mid, tops[1]],
            [
                width,
                kind as u8,
                way::flags(form, false, true, true),
                clear,
            ],
        ),
    ]
}

/// The tile's ground at the middle of brick `q`, in leaf cells, as its fill draws it.
fn ground_at(h: &Hand, q: [i32; 2]) -> i16 {
    let samples =
        |i: usize| ((h.fresh[clip::ELE_AT as usize + i / 2] >> (16 * (i % 2))) << 16) as i32 >> 16;
    let place = [(q[0] & 7) * 64 + 32, (q[1] & 7) * 64 + 32];
    ground::height(samples, place) as i16
}

/// A handed brick as its fill reads it (`clipmap::Brick`): its record, its ground and its
/// shard.
struct Handed<'a> {
    fresh: &'a [u32],
    ground: &'a [u32],
    base: u32,
    shard: Shard,
}

impl Handed<'_> {
    fn sample(&self, i: usize) -> i32 {
        ((self.fresh[clip::ELE_AT as usize + i / 2] >> (16 * (i % 2))) << 16) as i32 >> 16
    }
}

impl clip::Brick for Handed<'_> {
    fn cell(&self) -> Zoom {
        self.fresh[clip::FILL_CELL as usize]
    }
    fn place(&self) -> [u32; 2] {
        let w = self.fresh[clip::FILL_PLACE as usize];
        [w & 0xffff, w >> 16]
    }
    fn first(&self) -> [i32; 2] {
        let at = clip::FILL_FIRST as usize;
        [self.fresh[at] as i32, self.fresh[at + 1] as i32]
    }
    fn shard(&self) -> Shard {
        self.shard
    }
    fn line(&self, k: u32) -> u32 {
        self.fresh[(LINES_AT + k) as usize]
    }
    fn laid(&self, i: u32) -> u32 {
        fill::laid_of(self.ground, self.base, i)
    }
    fn height(&self, c: [i32; 2]) -> i32 {
        if c[0] >= 0 && c[1] >= 0 && c[0] < BRICK as i32 && c[1] < BRICK as i32 {
            let i = clip::idx_of([c[0] as u32, c[1] as u32]);
            return clip::column_top(fill::column_of(self.ground, self.base, i));
        }
        let p = clip::Brick::place(self);
        let g = [(p[0] * BRICK) as i32 + c[0], (p[1] * BRICK) as i32 + c[1]];
        ground::height(|i| self.sample(i), g)
    }
}

/// `h` with its columns' middles `ulps` off along each axis, and a ring column's height
/// drawn at a point `ring` ulps off its middle: what a GPU's float an ulp off the CPU's
/// reads where the CPU's lies on a rounding's or a comparison's edge.
struct Nudged<'a> {
    h: &'a Handed<'a>,
    ulps: [i32; 2],
    ring: i32,
}

fn nudge(v: f32, ulps: i32) -> f32 {
    f32::from_bits((v.to_bits() as i32 + ulps) as u32)
}

impl way::Columns for Nudged<'_> {
    fn cell(&self) -> Zoom {
        way::Columns::cell(self.h)
    }
    fn per_m(&self) -> f32 {
        way::Columns::per_m(self.h)
    }
    fn cnt(&self, c: [i32; 2]) -> u32 {
        way::Columns::cnt(self.h, c)
    }
    fn segment(&self, c: [i32; 2], k: u32) -> Segment {
        way::Columns::segment(self.h, c, k)
    }
    fn center(&self, c: [i32; 2]) -> [f32; 2] {
        let p = way::Columns::center(self.h, c);
        [nudge(p[0], self.ulps[0]), nudge(p[1], self.ulps[1])]
    }
    fn stations(&self) -> way::Stations {
        way::Columns::stations(self.h)
    }
    fn palette_idx(&self, c: [i32; 2]) -> u32 {
        way::Columns::palette_idx(self.h, c)
    }
    fn is_walled(&self, c: [i32; 2]) -> bool {
        way::Columns::is_walled(self.h, c)
    }
    fn face(&self, c: [i32; 2]) -> i32 {
        let side = BRICK as i32;
        let own = c[0] >= 0 && c[1] >= 0 && c[0] < side && c[1] < side;
        let held = c.iter().all(|&v| (-1..=side).contains(&v));
        if own || !held || self.ring == 0 {
            return way::Columns::face(self.h, c);
        }
        let cell = way::Columns::cell(self.h);
        let p = clip::Brick::place(self.h);
        let t = |k: usize| nudge((p[k] * BRICK) as f32 + c[k] as f32 + 0.5, self.ring);
        let h = ground::bicubic(|i| self.h.sample(i), t(0), t(1));
        ground::face_height(cell, ground::cells(cell, h as f32))
    }
    fn joins(&self, c: [i32; 2], k: u32) -> bool {
        way::Columns::joins(self.h, c, k)
    }
}

/// Column `c`'s spans' words as its fill keeps them (fill.rs `keep`).
fn kept((s, fits): (way::Spans, bool)) -> Vec<u32> {
    let held = |v: i32| v.clamp(i16::MIN as i32, i16::MAX as i32);
    let face = if s.face == i32::MIN {
        i16::MIN as i32
    } else {
        held(s.face).max(i16::MIN as i32 + 1)
    };
    let mut w = vec![s.n | if fits { 0 } else { fill::SPANS_PAST } | (face as u32) << 16];
    for j in 0..s.n as usize {
        let run = [held(s.run[j][0]), held(s.run[j][1])];
        w.push((run[0] & 0xffff | run[1] << 16) as u32);
        w.push(s.val[j]);
    }
    w
}

/// Brick `h`'s spans from `mine` as the GPU kept them, `got`, against the CPU's, `want`: a
/// column's alike, or alike the CPU's an ulp off where its float lies on an edge (`Nudged`).
fn spans_alike(
    want: &[u32],
    got: &[u32],
    mine: usize,
    (h, slot, ground): (&Hand, u32, &[u32]),
    tally: &mut Tally,
) {
    let handed = Handed {
        fresh: &h.fresh,
        ground,
        base: slot * BRICK_WORDS,
        shard: h.shard,
    };
    for i in 0..SPANS_COLUMNS as usize {
        let at = mine + (SPANS_AT + i as u32 * SPANS_COLUMN_WORDS) as usize;
        let len = 1 + 2 * (want[at] & fill::SPANS_CNT) as usize;
        let got_len = 1 + 2 * (got[at] & fill::SPANS_CNT) as usize;
        let c = [
            (i as u32 % (BRICK + 2)) as i32 - 1,
            (i as u32 / (BRICK + 2)) as i32 - 1,
        ];
        if got[at..at + got_len] != want[at..at + len] {
            let near = (-1..=1).any(|ulps_x| {
                (-1..=1).any(|ulps_y| {
                    (-1..=1).any(|ring| {
                        let n = Nudged {
                            h: &handed,
                            ulps: [ulps_x, ulps_y],
                            ring,
                        };
                        kept(way::column(&n, c)) == got[at..at + got_len]
                    })
                })
            });
            assert!(
                near,
                "{}: column {c:?}'s spans {:?}, the CPU's {:?}",
                h.name,
                &got[at..at + got_len],
                &want[at..at + len]
            );
            tally.add("spans an ulp off");
        }
        let spans = want[at] & fill::SPANS_CNT;
        tally.add("a column");
        if spans != 0 {
            tally.add(format!("spans {}", spans.min(4)));
        }
        if want[at] & fill::SPANS_PAST != 0 {
            tally.add("spans past their most");
        }
        let edge = |v: i32| v == -1 || v == BRICK as i32;
        if want[at] >> 16 == 0x8000 && !(edge(c[0]) && edge(c[1])) {
            tally.add("a column of none");
        }
        for j in 0..spans as usize {
            let v = want[at + 2 + 2 * j];
            tally.add(if v == value::PIER {
                "a pier"
            } else if v & way::AIR_OF != 0 {
                "a bore's air"
            } else {
                "a deck or a bank"
            });
        }
    }
}

/// What a frame's fill hands: its records in order and the slots they go to.
struct Frame {
    hands: Vec<(Hand, u32)>,
}

/// How a build ends (`way_tree_cs`): built, no ways, or refused for the brick's most,
/// for its scratch or for the pool, with the child groups taken first, given back.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum End {
    Built,
    Bare,
    Past(u32),
    Cramped(u32),
    Starved(u32),
}

/// A brick's build as the CPU makes it: its nodes along z and each level's flagged
/// (`way::tree::flag` over a CPU pool, as `fill::tree` builds); its end with the pool
/// endless and its scratch holding `room`; its tree built from child group 1 into `pool`,
/// what it took, held and its root word.
struct Want {
    nodes: u32,
    flagged: Vec<u32>,
    taken_held_root: [u32; 3],
}

fn want(lane: &[u32], scratch: &[u32], cell: Zoom, pool: &mut Vec<u32>) -> Want {
    let (z0, nodes) = fill::root_span(lane, 0, cell);
    let mut flagged = Vec::new();
    if nodes != 0 && nodes <= tree::ROOT_NODES {
        let brick = fill::LoneBrick {
            ground: lane,
            base: 0,
        };
        let slots = pool.len() as u32 / 2;
        let mut p = tree::Column(pool);
        tree::root(&mut p, z0, CHILDREN);
        let (mut b, mut n, mut next) = (CHILDREN, CHILDREN, 2 * CHILDREN);
        for zoom in cell - BRICK_SHIFT..cell {
            let f = (b..b + n)
                .filter(|&s| tree::flag(&p, &brick, 0, cell, zoom, s))
                .count() as u32;
            flagged.push(f);
            if next + CHILDREN * f > slots {
                break;
            }
            let mut k = 0;
            for s in b..b + n {
                if tree::flag(&p, &brick, 0, cell, zoom, s) {
                    tree::branch(&mut p, s, next + CHILDREN * k);
                    k += 1;
                } else {
                    tree::empty(&mut p, s);
                }
            }
            (b, n, next) = (next, CHILDREN * f, next + CHILDREN * f);
        }
    }
    // Built again in a pool of its own size, kept with its slot.
    let [taken, ..] = fill::tree(lane, scratch, cell, pool, 1);
    if taken != u32::MAX {
        *pool = vec![0; ((taken + 2) * tree::CHILD_GROUP_WORDS) as usize];
    }
    let taken_held_root = fill::tree(lane, scratch, cell, pool, 1);
    Want {
        nodes,
        flagged,
        taken_held_root,
    }
}

impl Want {
    /// Its end with the pool endless and a scratch of `room`.
    fn end(&self, room: u32) -> End {
        if self.nodes == 0 {
            return End::Bare;
        }
        if self.nodes > tree::ROOT_NODES {
            return End::Past(0);
        }
        let mut taken = 1;
        for &f in &self.flagged {
            if tree::past(taken, f) {
                return End::Past(taken);
            }
            if tree::is_cramped(room, taken, f) {
                return End::Cramped(taken);
            }
            taken += f;
        }
        End::Built
    }

    /// The child groups taken before each level, the root's 1 first: what a build that ran
    /// out of the pool there gave back.
    fn taken_before(&self) -> Vec<u32> {
        let mut out = vec![0, 1];
        let mut taken = 1;
        for &f in &self.flagged {
            taken += f;
            out.push(taken);
        }
        out
    }
}

/// Whether the trees under child groups `g` of `a` and `c` of `b` are one: each slot a leaf
/// or a branch alike, its word the same, its branches' alike; `a`'s child groups collected.
fn same(a: &impl Columns, g: u32, b: &impl Columns, c: u32, ids: &mut BTreeSet<u32>) -> bool {
    ids.insert(g / CHILDREN);
    (0..CHILDREN).all(|k| {
        let (fa, fb) = (a.first_child(g + k), b.first_child(c + k));
        (fa == 0) == (fb == 0)
            && a.data(g + k) == b.data(c + k)
            && (fa == 0 || same(a, fa & !BRANCH, b, fb & !BRANCH, ids))
    })
}

/// The child groups the tree under child group `g` holds.
fn held(a: &impl Columns, g: u32, ids: &mut BTreeSet<u32>) {
    ids.insert(g / CHILDREN);
    for k in 0..CHILDREN {
        let f = a.first_child(g + k);
        if f != 0 {
            held(a, f & !BRANCH, ids);
        }
    }
}

struct Gpu {
    device: wgpu::Device,
    queue: wgpu::Queue,
    passes: [wgpu::ComputePipeline; 4],
}

/// The buffers a frame's fill binds, each of its words.
struct Bufs {
    fresh: wgpu::Buffer,
    ground: wgpu::Buffer,
    pool: wgpu::Buffer,
    stack: wgpu::Buffer,
    scratch: wgpu::Buffer,
    marker: wgpu::Buffer,
    pool_groups: u32,
    room: u32,
}

impl Gpu {
    fn buffer(&self, words: &[u32]) -> wgpu::Buffer {
        use wgpu::util::DeviceExt as _;
        self.device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: None,
                contents: bytes(words),
                usage: wgpu::BufferUsages::STORAGE
                    | wgpu::BufferUsages::COPY_SRC
                    | wgpu::BufferUsages::COPY_DST,
            })
    }

    fn wait(&self) {
        self.device
            .poll(wgpu::PollType::Wait {
                submission_index: None,
                timeout: Some(Duration::from_secs(60)),
            })
            .expect("the device within a minute");
    }

    fn read(&self, from: &wgpu::Buffer) -> Vec<u32> {
        let staging = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: from.size(),
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut e = self.device.create_command_encoder(&Default::default());
        e.copy_buffer_to_buffer(from, 0, &staging, 0, from.size());
        self.queue.submit([e.finish()]);
        staging
            .slice(..)
            .map_async(wgpu::MapMode::Read, |r| r.unwrap());
        self.wait();
        let v = staging.slice(..).get_mapped_range().unwrap();
        v.as_chunks::<4>()
            .0
            .iter()
            .map(|b| u32::from_le_bytes(*b))
            .collect()
    }

    /// The pool and its stack twice as large, copied as the renderer grows them (pxl-render
    /// gpu/ray/ground.rs `GroundBuffers::grow`).
    fn grow(&self, b: &mut Bufs) {
        let (old, new) = (b.pool_groups, 2 * b.pool_groups);
        let half = |groups: u32| u64::from(groups * tree::CHILD_GROUP_WORDS * 4 / 2);
        let pool = self.buffer(&vec![0; (new * tree::CHILD_GROUP_WORDS) as usize]);
        let stack = self.buffer(&vec![0; new as usize]);
        let mut e = self.device.create_command_encoder(&Default::default());
        e.copy_buffer_to_buffer(&b.pool, 0, &pool, 0, half(old));
        e.copy_buffer_to_buffer(&b.pool, half(old), &pool, half(new), half(old));
        e.copy_buffer_to_buffer(&b.stack, 0, &stack, 0, u64::from(old * 4));
        self.queue.submit([e.finish()]);
        self.wait();
        (b.pool, b.stack, b.pool_groups) = (pool, stack, new);
    }

    /// The fill's passes `which` of its four over `n` handed bricks, in the renderer's
    /// order (gpu/ray/ground.rs `grids`); then a word cleared in the same command buffer,
    /// which reads zero only where it ran to its end.
    fn fill(&self, b: &Bufs, n: u32, which: std::ops::Range<usize>) {
        let [free, spans, build, give_back] = &self.passes;
        let group = |p: &wgpu::ComputePipeline, bufs: &[(u32, &wgpu::Buffer)]| {
            let entries: Vec<_> = bufs
                .iter()
                .map(|(k, buf)| wgpu::BindGroupEntry {
                    binding: *k,
                    resource: buf.as_entire_binding(),
                })
                .collect();
            self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: None,
                layout: &p.get_bind_group_layout(0),
                entries: &entries,
            })
        };
        let groups = [
            group(
                free,
                &[(0, &b.fresh), (1, &b.ground), (2, &b.pool), (3, &b.stack)],
            ),
            group(spans, &[(0, &b.fresh), (1, &b.ground), (2, &b.scratch)]),
            group(
                build,
                &[
                    (0, &b.fresh),
                    (1, &b.ground),
                    (2, &b.pool),
                    (3, &b.stack),
                    (4, &b.scratch),
                ],
            ),
            group(give_back, &[(0, &b.scratch), (1, &b.stack)]),
        ];
        let sides = [
            [1, 1, n],
            [SPANS_COLUMNS.div_ceil(fill::SPANS_LANES), 1, n],
            [1, 1, n],
            [1, 1, n],
        ];
        let mut e = self.device.create_command_encoder(&Default::default());
        for ((p, g), [x, y, z]) in self
            .passes
            .iter()
            .zip(&groups)
            .zip(sides)
            .skip(which.start)
            .take(which.len())
        {
            let mut pass = e.begin_compute_pass(&Default::default());
            pass.set_pipeline(p);
            pass.set_bind_group(0, g, &[]);
            pass.dispatch_workgroups(x, y, z);
        }
        e.clear_buffer(&b.marker, 0, None);
        self.queue.submit([e.finish()]);
        self.wait();
        assert_eq!(
            self.read(&b.marker),
            [0; 4],
            "a fill that did not run to its end"
        );
        self.queue.write_buffer(&b.marker, 0, bytes(&[1; 4]));
    }
}

/// What the test saw, a count each.
#[derive(Default)]
struct Tally(BTreeMap<String, u32>);

impl Tally {
    fn add(&mut self, name: impl Into<String>) {
        *self.0.entry(name.into()).or_default() += 1;
    }
}

/// The run's state across frames: the CPU's ground, each slot's brick and its tree as the
/// CPU built it, and the mirror the records are laid in.
struct Run {
    ground: Vec<u32>,
    /// Each slot's brick, its tree's root on the GPU, and the CPU's pool its tree was built
    /// in from child group 1, with its root word there.
    held: BTreeMap<u32, (String, u32, Vec<u32>, u32)>,
    mirror: Vec<u32>,
    cpu_pool: Vec<u32>,
    /// Bricks to hand again: their build ran out of the pool or the scratch, and which ran
    /// out in the last frame.
    again: Vec<Hand>,
    ran_out: [bool; 2],
    tally: Tally,
}

fn run_frame(gpu: &Gpu, b: &mut Bufs, run: &mut Run, frame: &Frame) {
    let n = frame.hands.len() as u32;
    assert!(n <= FILL_CAP);
    let scratch_len = (FILL_CAP * tree::scratch_words(b.room)) as usize;
    // The records as handed, each its slot.
    let mut fresh = vec![0u32; (FILL_CAP * HANDED_WORDS) as usize];
    for (k, (h, slot)) in frame.hands.iter().enumerate() {
        let at = k * HANDED_WORDS as usize;
        fresh[at..at + HANDED_WORDS as usize].copy_from_slice(&h.fresh);
        fresh[at + clip::FILL_BRICK as usize] = *slot;
    }
    // The CPU's fill: tops, then spans; its levels over the GPU's spans, which the tree
    // reads, as `bound_cs` lays them between.
    let mut scratch = vec![0u32; scratch_len];
    for (k, (_, slot)) in frame.hands.iter().enumerate() {
        let base = (slot * BRICK_WORDS) as usize;
        let root = run.ground[base + ROOT_AT as usize];
        run.ground[base..base + BRICK_WORDS as usize].fill(0);
        run.ground[base + ROOT_AT as usize] = root;
        for y in 0..BRICK {
            for x in 0..BRICK {
                fill::heights(&mut run.ground, &fresh, k as u32, [x, y]);
            }
        }
        for i in 0..SPANS_COLUMNS {
            fill::spans(&run.ground, &fresh, &mut scratch, k as u32, i);
        }
    }
    let upload = |ground: &[u32]| {
        for (_, slot) in &frame.hands {
            let base = (slot * BRICK_WORDS) as usize;
            gpu.queue.write_buffer(
                &b.ground,
                u64::from(slot * BRICK_WORDS * 4),
                bytes(&ground[base..base + BRICK_WORDS as usize]),
            );
        }
    };
    gpu.queue.write_buffer(&b.fresh, 0, bytes(&fresh));
    upload(&run.ground);
    gpu.fill(b, n, 0..2);
    let got_scratch = gpu.read(&b.scratch);
    for (k, (h, slot)) in frame.hands.iter().enumerate() {
        let mine = fill::scratch_at(&scratch, k as u32) as usize;
        spans_alike(
            &scratch,
            &got_scratch,
            mine,
            (h, *slot, &run.ground),
            &mut run.tally,
        );
    }
    for (k, (h, slot)) in frame.hands.iter().enumerate() {
        let spans = fill::Scratch {
            words: &got_scratch,
            at: fill::scratch_at(&got_scratch, k as u32),
        };
        for j in 0..=LEVELS {
            let side = BRICK >> j;
            for e in 0..side * side {
                fill::level(
                    &mut run.ground,
                    slot * BRICK_WORDS,
                    &spans,
                    h.cell,
                    j,
                    [e % side, e / side],
                );
            }
        }
    }
    upload(&run.ground);
    gpu.fill(b, n, 2..4);
    let (got_scratch, got_ground, got_pool, got_stack) = (
        gpu.read(&b.scratch),
        gpu.read(&b.ground),
        gpu.read(&b.pool),
        gpu.read(&b.stack),
    );
    let pool = tree::Read(&got_pool);
    let mut ends = Vec::new();
    let records_at = tree::records_at(b.room);
    for (k, (h, slot)) in frame.hands.iter().enumerate() {
        let mine = fill::scratch_at(&scratch, k as u32) as usize;
        // The build.
        let base = (slot * BRICK_WORDS) as usize;
        let lane = &run.ground[base..base + BRICK_WORDS as usize];
        let mut cpu_pool = run.cpu_pool.clone();
        let w = want(lane, &got_scratch[mine..], h.cell, &mut cpu_pool);
        let root = got_ground[base + ROOT_AT as usize];
        let count = got_scratch[mine + records_at as usize];
        let freed = got_scratch[mine + tree::FREED_AT as usize];
        let end = match count {
            tree::NONE => End::Past(freed),
            tree::CRAMPED => End::Cramped(freed),
            tree::STARVED => End::Starved(freed),
            0 if root == 0 => End::Bare,
            _ => End::Built,
        };
        let expected = w.end(b.room);
        match (end, expected) {
            (End::Starved(f), End::Built | End::Past(_) | End::Cramped(_)) => {
                let before = w.taken_before();
                let last = match expected {
                    End::Past(t) | End::Cramped(t) => t,
                    _ => u32::MAX,
                };
                assert!(
                    before.contains(&f) && f <= last,
                    "{}: starved giving back {f}, taken before a level {before:?}",
                    h.name
                );
            }
            _ => assert_eq!(end, expected, "{}: how its build ended", h.name),
        }
        ends.push(end);
        run.tally.add(match end {
            End::Built => "built",
            End::Bare => "no ways",
            End::Past(0) => "past its root",
            End::Past(_) => "past its most",
            End::Cramped(_) => "cramped",
            End::Starved(0) => "starved at its root",
            End::Starved(_) => "starved",
        });
        if end != End::Built {
            assert_eq!(root, 0, "{}: a root where none is built", h.name);
            if let End::Cramped(_) | End::Starved(_) = end {
                run.again.push(h.clone());
                run.ran_out[matches!(end, End::Cramped(_)) as usize] = true;
            }
            run.held.remove(slot);
            continue;
        }
        let [taken, kept, want_root] = w.taken_held_root;
        assert_eq!(
            root >> tree::ROOT_ID_BITS,
            want_root >> tree::ROOT_ID_BITS,
            "{}: its root's Z",
            h.name
        );
        assert_eq!(freed, taken - kept, "{}: what its folds gave back", h.name);
        if freed != 0 {
            run.tally.add("a fold gave back");
        }
        let cpu = tree::Read(&cpu_pool);
        let mut ids = BTreeSet::new();
        assert!(
            same(
                &pool,
                tree::root_slot(root).0,
                &cpu,
                tree::root_slot(want_root).0,
                &mut ids
            ),
            "{}: its tree",
            h.name
        );
        assert_eq!(
            ids.len() as u32,
            kept,
            "{}: the child groups it holds",
            h.name
        );
        // Its records, as the CPU's mirror lays them.
        let records = &got_scratch[mine + records_at as usize..];
        assert_eq!(count, kept, "{}: its records", h.name);
        let laid: BTreeSet<u32> = (0..count)
            .map(|r| records[(1 + r * tree::RECORD_WORDS) as usize])
            .collect();
        assert_eq!(laid, ids, "{}: its records' child groups", h.name);
        tree::lay(&mut tree::Column(&mut run.mirror), records);
        run.held
            .insert(*slot, (h.name.clone(), root, cpu_pool, want_root));
    }
    for slot in 0..SLOTS {
        let at = (slot * BRICK_WORDS + ROOT_AT) as usize;
        run.ground[at] = got_ground[at];
    }
    // A pop that ran out of the pool beside one that fit: a failed pop leaves the count where
    // it found it, which the stack's count below holds, whichever order the two took.
    if ends.iter().any(|e| matches!(e, End::Starved(_))) && ends.contains(&End::Built) {
        run.tally.add("built beside one starved");
    }
    // Every slot's tree whole, in the GPU's pool and in the mirror, and none built over another.
    let mirror = tree::Read(&run.mirror);
    let mut live = BTreeSet::new();
    for (slot, (name, root, cpu_pool, want_root)) in &run.held {
        assert_eq!(
            got_ground[(slot * BRICK_WORDS + ROOT_AT) as usize],
            *root,
            "{name}: its root"
        );
        let cpu = tree::Read(cpu_pool);
        let (first, want_first) = (tree::root_slot(*root).0, tree::root_slot(*want_root).0);
        let mut ids = BTreeSet::new();
        assert!(
            same(&pool, first, &cpu, want_first, &mut ids),
            "{name}: its tree held"
        );
        assert!(
            same(&mirror, first, &cpu, want_first, &mut BTreeSet::new()),
            "{name}: its tree in the mirror"
        );
        let before = live.len();
        live.extend(ids.iter().copied());
        assert_eq!(
            live.len(),
            before + ids.len(),
            "{name}: a child group two trees hold"
        );
    }
    for slot in 0..SLOTS {
        if !run.held.contains_key(&slot) {
            assert_eq!(got_ground[(slot * BRICK_WORDS + ROOT_AT) as usize], 0);
        }
    }
    // The stack: as many taken as the trees hold, and past them every child group they do
    // not hold, each once. The taken slots' ids are stale where a push wrote past them.
    let len = got_stack.len() as u32;
    let id = |i: u32| got_stack[i as usize] ^ i;
    let taken = got_stack[tree::TAKEN_CNT as usize];
    assert_eq!(taken as usize, live.len(), "the stack's count");
    let free: Vec<u32> = (taken + 1..len).map(id).collect();
    let free_set: BTreeSet<u32> = free.iter().copied().collect();
    assert_eq!(free.len(), free_set.len(), "a free child group twice");
    let unheld: BTreeSet<u32> = (1..len).filter(|g| !live.contains(g)).collect();
    assert_eq!(free_set, unheld, "the stack's free");
    let mut held_groups = BTreeSet::new();
    for (_, root, _, _) in run.held.values() {
        held(&pool, tree::root_slot(*root).0, &mut held_groups);
    }
    assert_eq!(held_groups, live);
}

#[test]
fn hand_ways_fill_as_the_rust() {
    let instance =
        wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    let Ok(adapter) = pollster::block_on(instance.request_adapter(&Default::default())) else {
        eprintln!("no adapter: skipped");
        return;
    };
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        required_limits: adapter.limits(),
        ..Default::default()
    }))
    .unwrap();
    let pipeline = |kind: u32| {
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some(record::compute_entry(kind)),
            source: wgpu::ShaderSource::Wgsl(record::src(kind).into()),
        });
        device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some(record::compute_entry(kind)),
            layout: None,
            module: &module,
            entry_point: Some(record::compute_entry(kind)),
            compilation_options: Default::default(),
            cache: None,
        })
    };
    let passes = [
        pipeline(SHADER_WAY_FREE),
        pipeline(SHADER_WAY),
        pipeline(SHADER_WAY_TREE),
        pipeline(SHADER_WAY_RETURN),
    ];
    let gpu = Gpu {
        device,
        queue,
        passes,
    };
    let shards = real_world::shards();
    let area = World {
        clipmap: &[],
        ground: &[],
        palette: &[],
        shards: &shards,
        overwrites: &[],
        pool: &[],
    };

    // The bricks: every eye clip level's about the seam and the area's south edge, and the
    // leaf levels' about the notch; most lined, some not.
    let mut bricks: Vec<Hand> = Vec::new();
    for (eye, cells) in [
        (EYE, FIRST_CELL..=LEAF_CELL),
        (EYE_NOTCH, LEAF_CELL - 2..=LEAF_CELL),
    ] {
        for cell in cells {
            let first = level_first(eye, cell);
            for k in 0..ENTRIES {
                let q = [
                    wrap_idx(clip::bricks_zoom(cell), first[0] + (k % ACROSS) as i32),
                    first[1] + (k / ACROSS) as i32,
                ];
                let h = mix(cell, q);
                if !h.is_multiple_of(3) && cell < LEAF_CELL - 2 {
                    continue;
                }
                bricks.push(hand(
                    eye,
                    &area,
                    (cell, q),
                    !(h >> 8).is_multiple_of(5),
                    &[],
                ));
            }
        }
    }
    // A low deck under each bridge of a leaf brick's, so the high one's piers stand on it or
    // not; a brick whose ways span more of its nodes along z than its root holds; and one of
    // bores deep and wide, which takes past a brick's most.
    let leaf = level_first(EYE, LEAF_CELL);
    for k in 0..6 {
        let q = [leaf[0] + k, leaf[1] + 3];
        let h = hand(EYE, &area, (LEAF_CELL, q), true, &[]);
        let mut low = Vec::new();
        for s in [0, 1, 6, 7] {
            let mut w = segment(&h.fresh, s);
            let tops = [
                (w[2] as i16 as i32 - 30) as i16,
                ((w[2] >> 16) as i16 as i32 - 30) as i16,
            ];
            w[2] = (tops[0] as u16 as u32) | (tops[1] as u16 as u32) << 16;
            low.push(w);
        }
        let mut h = hand(EYE, &area, (LEAF_CELL, q), true, &low);
        h.name += " under a deck";
        bricks.push(h);
    }
    let across = |dy: i32| ([-8, 32 + dy], [72, 32 + dy]);
    for k in 0..2 {
        let q = [leaf[0] + 2 + k, leaf[1] + 4];
        let h = hand(EYE, &area, (LEAF_CELL, q), false, &[]);
        let g = ground_at(&h, q) as i32;
        let kind = value::TRANSPORTS[0] + 3;
        let mut more = Vec::new();
        let (a, b) = across(-10);
        more.extend(line(
            q,
            way::BRIDGE,
            a,
            b,
            [(g + 400) as i16; 2],
            40,
            kind,
            0,
        ));
        let (a, b) = across(10);
        more.extend(line(
            q,
            way::TUNNEL,
            a,
            b,
            [(g - 300) as i16; 2],
            40,
            kind,
            20,
        ));
        let mut h = hand(EYE, &area, (LEAF_CELL, q), false, &more);
        h.name += " past its root";
        bricks.push(h);
        let mut more = Vec::new();
        for dy in [-26, 0, 26] {
            let (a, b) = across(dy);
            more.extend(line(
                q,
                way::TUNNEL,
                a,
                b,
                [(g - 240) as i16; 2],
                255,
                kind,
                250,
            ));
        }
        let mut h = hand(EYE, &area, (LEAF_CELL, q), false, &more);
        h.name += " past its most";
        bricks.push(h);
    }
    // Lines that end inside a brick, run along y, and on the very diagonal, at the leaves and
    // a zoom over, where piers stand: each form, narrow and wide, high and low.
    for (k, cell) in [
        (0, LEAF_CELL),
        (1, LEAF_CELL),
        (2, LEAF_CELL - 1),
        (3, LEAF_CELL - 1),
    ] {
        let first = level_first(EYE, cell);
        let q = [first[0] + 2 + k, first[1] + 5];
        let h = hand(EYE, &area, (cell, q), false, &[]);
        let g = ground_at(&h, q) as i32;
        let lift = |d: i32| [(g + d) as i16, (g + d + 3) as i16];
        let mut more = Vec::new();
        for (form, from, to, tops, width) in [
            (way::BRIDGE, [12, 20], [44, 20], lift(30), 40),
            (way::BRIDGE, [30, -6], [30, 70], lift(40), 90),
            (way::BRIDGE, [-4, -4], [68, 68], lift(25), 24),
            (way::BRIDGE, [50, 60], [8, 18], lift(35), 120),
            (way::TUNNEL, [20, 40], [20, 10], lift(-20), 30),
            (way::APPROACH, [40, 5], [60, 35], lift(2), 50),
            (way::APPROACH, [5, 50], [26, 50], lift(-3), 50),
        ] {
            let kind = value::TRANSPORTS[0] + more.len() as u32 % 19;
            more.extend(line(q, form, from, to, tops, width, kind, 6));
        }
        let mut h = hand(EYE, &area, (cell, q), false, &more);
        h.name += " ending inside";
        bricks.push(h);
    }
    // Bores stacked over the same columns, more than a column's spans hold.
    {
        let q = [leaf[0] + 1, leaf[1] + 5];
        let h = hand(EYE, &area, (LEAF_CELL, q), false, &[]);
        let g = ground_at(&h, q) as i32;
        let mut more = Vec::new();
        for k in 0..12 {
            let kind = value::TRANSPORTS[0] + k as u32;
            let (a, b) = across(0);
            more.extend(line(
                q,
                way::TUNNEL,
                a,
                b,
                [(g - 12 * k - 10) as i16; 2],
                40,
                kind,
                6,
            ));
        }
        let mut h = hand(EYE, &area, (LEAF_CELL, q), false, &more);
        h.name += " past its spans";
        bricks.push(h);
    }
    eprintln!("{} bricks", bricks.len());

    let mut b = Bufs {
        fresh: gpu.buffer(&vec![0; (FILL_CAP * HANDED_WORDS) as usize]),
        ground: gpu.buffer(&vec![0; (SLOTS * BRICK_WORDS) as usize]),
        pool: gpu.buffer(&vec![0; (POOL_FIRST * tree::CHILD_GROUP_WORDS) as usize]),
        stack: gpu.buffer(&vec![0; POOL_FIRST as usize]),
        scratch: gpu.buffer(&vec![
            0;
            (FILL_CAP * tree::scratch_words(ROOM_FIRST)) as usize
        ]),
        marker: gpu.buffer(&[1; 4]),
        pool_groups: POOL_FIRST,
        room: ROOM_FIRST,
    };
    let mut run = Run {
        ground: vec![0; (SLOTS * BRICK_WORDS) as usize],
        held: BTreeMap::new(),
        mirror: vec![0; (POOL_MOST * tree::CHILD_GROUP_WORDS) as usize],
        cpu_pool: vec![0; ((tree::BRICK_CHILD_GROUPS + 2) * tree::CHILD_GROUP_WORDS) as usize],
        again: Vec::new(),
        ran_out: [false; 2],
        tally: Tally::default(),
    };
    let (mut next_slot, mut frames) = (0u32, 0u32);
    let mut queue = bricks.into_iter();
    loop {
        // A frame's bricks: those to build again first, then the next handed, each the next
        // slot round the ground.
        let mut hands: Vec<Hand> = std::mem::take(&mut run.again);
        hands.extend(
            queue
                .by_ref()
                .take((FILL_CAP as usize).saturating_sub(hands.len())),
        );
        if hands.is_empty() {
            break;
        }
        // The pool grows where a build ran out of it, the scratch where one outgrew it, as
        // the renderer grows them ahead of the next turn's bricks.
        let [pool_out, scratch_out] = std::mem::take(&mut run.ran_out);
        if pool_out && b.pool_groups < POOL_MOST {
            gpu.grow(&mut b);
            run.tally.add("the pool grown");
        }
        if scratch_out && b.room < tree::BRICK_CHILD_GROUPS {
            b.room = (2 * b.room).min(tree::BRICK_CHILD_GROUPS);
            b.scratch = gpu.buffer(&vec![0; (FILL_CAP * tree::scratch_words(b.room)) as usize]);
            run.tally.add("the scratch grown");
        }
        let frame = Frame {
            hands: hands
                .into_iter()
                .map(|h| {
                    let slot = next_slot;
                    next_slot = (next_slot + 1) % SLOTS;
                    if run.held.contains_key(&slot) {
                        run.tally.add("filled over a tree");
                    }
                    (h, slot)
                })
                .collect(),
        };
        run_frame(&gpu, &mut b, &mut run, &frame);
        frames += 1;
    }
    eprintln!(
        "{frames} frames, pool {} child groups, scratch {}",
        b.pool_groups, b.room
    );
    for (name, n) in &run.tally.0 {
        eprintln!("  {name}: {n}");
    }
    let off = run.tally.0.get("spans an ulp off").copied().unwrap_or(0);
    assert!(
        off * 2000 < run.tally.0["a column"],
        "{off} columns' spans an ulp off"
    );
    for name in [
        "built",
        "no ways",
        "past its root",
        "past its most",
        "starved",
        "built beside one starved",
        "a fold gave back",
        "filled over a tree",
        "the pool grown",
        "the scratch grown",
        "a column of none",
        "spans past their most",
        "a pier",
        "a bore's air",
        "a deck or a bank",
        "cramped",
        "spans 1",
        "spans 2",
        "spans 4",
    ] {
        assert!(run.tally.0.contains_key(name), "no {name}");
    }
}
