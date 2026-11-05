//! What the tests of a walk that writes the node cache share (tests/light_miss.rs,
//! tests/forget.rs, tests/sample.rs): `Writing`'s rule over a table of words, what the world
//! answers of a node, what a table is laid with, and the device.

#![allow(dead_code, reason = "each test reads its own of it")]

use std::collections::{BTreeMap, BTreeSet};

use pxl_engine::answer::{self, In, Kind};
use pxl_engine::cast::Query;
use pxl_engine::overwrite::Cut;
use pxl_engine::node_cache::{self, BUCKET, CLAIMED, NONE};
use pxl_engine::way::tree;
use pxl_engine::world::{self, Inheritance};
use pxl_engine::{value, Zoom, FRAC_W};

use crate::real_world::World;

/// How many visits each node had, by its zoom, its index and the zooms it was asked at.
pub type Met = BTreeMap<(Zoom, [i32; 3], [Zoom; 2]), u32>;

/// Why each visit was answered as it was: off a slot, or of the world because no slot may
/// keep it, none holds it, one holds it at other zooms, or this pass keeps it.
#[derive(Default, Clone, Copy)]
pub struct Asks {
    pub visits: u64,
    pub kept: u64,
    pub past_roots: u64,
    pub absent: u64,
    pub other_zooms: u64,
    pub own_pass: u64,
}

impl Asks {
    pub fn of_world(&self) -> u64 {
        self.past_roots + self.absent + self.other_zooms + self.own_pass
    }
}

pub fn is_node(t: &[u32], w: usize, zoom: Zoom, c: [i32; 3]) -> bool {
    let n = w + node_cache::W_NODE as usize;
    t[n] == zoom && t[n + 1] == c[0] as u32 && t[n + 2] == c[1] as u32 && t[n + 3] == c[2] as u32
}

/// `Writing` over a table of words, one walk at a time.
pub struct Keeping<'a> {
    pub w: World<'a>,
    pub cuts: [Cut; 2],
    pub table: &'a mut [u32],
    pub tag: u32,
    pub asks: Asks,
    /// Every node visited at the zoom asked, how many times, where it is wanted.
    pub met: Option<&'a mut Met>,
    /// Whether what it asks is kept: none, for a walk over a table as it stands.
    pub writes: bool,
}

impl Query for Keeping<'_> {
    type At = Inheritance;

    fn root(&mut self, node: [i32; 3]) -> Inheritance {
        Inheritance::of(&self.w, (0, node))
    }

    fn child(&mut self, at: &Inheritance, zoom: Zoom, node: [i32; 3]) -> Inheritance {
        at.child(&self.w, &tree::Read(self.w.pool), (zoom, node))
    }

    fn node(
        &mut self,
        n: &mut Inheritance,
        zoom: Zoom,
        c: [i32; 3],
        zooms: [Zoom; 2],
        _: f32,
    ) -> In<[u32; 2]> {
        self.asks.visits += 1;
        if let Some(met) = &mut self.met {
            *met.entry((zoom, c, zooms)).or_default() += 1;
        }
        let t = &mut *self.table;
        let (mut to, mut key) = (NONE, CLAIMED);
        let mut other_zooms = false;
        let mut own_pass = false;
        let mut unstamped = false;
        let keeps = node_cache::keeps(zoom, c);
        if keeps {
            let first = node_cache::entry_of(zoom, c);
            let mut free = NONE;
            let mut k = 0;
            while k < BUCKET {
                let w = node_cache::entry_at(first + k) as usize;
                let held = t[w];
                if held != CLAIMED && held != 0 && is_node(t, w, zoom, c) {
                    if node_cache::holds(held, zooms) {
                        if node_cache::tag_of(held) == self.tag {
                            // Kept by a walk of this pass: asked of the world, as its words
                            // may be half written, and left to that walk.
                            own_pass = true;
                            to = NONE;
                            free = NONE;
                            break;
                        }
                        let a = w + node_cache::W_INHERITANCE as usize;
                        let handed = [t[a], t[a + 1], t[a + 2], t[a + 3]];
                        let at = w + node_cache::W_ASSET as usize;
                        let asset = [t[at], t[at + 1]];
                        let word = t[w + node_cache::W_KIND as usize];
                        if node_cache::stamp_holds(word, held, asset, handed, zoom, c) {
                            *n = Inheritance::of_words(handed);
                            self.asks.kept += 1;
                            return In {
                                kind: node_cache::kind_of(word),
                                asset,
                            };
                        }
                        // Words of two writes: asked of the world, and left to the walk that
                        // writes.
                        unstamped = true;
                        to = NONE;
                        free = NONE;
                        break;
                    }
                    other_zooms = true;
                    if node_cache::tag_of(held) / node_cache::PASSES < self.tag / node_cache::PASSES {
                        (to, key) = (first + k, held);
                    }
                } else if held == 0 && free == NONE {
                    free = first + k;
                }
                k += 1;
            }
            if to == NONE && free != NONE {
                (to, key) = (free, 0);
            } else if to == NONE && k == BUCKET {
                let v = first + node_cache::victim(zoom, c);
                (to, key) = (v, t[node_cache::entry_at(v) as usize]);
            }
        }
        let which = if !keeps {
            &mut self.asks.past_roots
        } else if unstamped {
            &mut self.asks.other_zooms
        } else if own_pass {
            &mut self.asks.own_pass
        } else if other_zooms {
            &mut self.asks.other_zooms
        } else {
            &mut self.asks.absent
        };
        *which += 1;
        let pool = tree::Read(self.w.pool);
        let mut held = self.w;
        let answer = world::node(
            &mut held,
            &self.w,
            &self.w,
            &pool,
            n,
            (zoom, c),
            zooms,
            self.cuts,
        );
        if self.writes && to != NONE && key != CLAIMED {
            put_entry(
                t,
                to,
                zoom,
                c,
                node_cache::key(self.tag, zooms),
                &answer,
                n.words(),
            );
        }
        answer
    }
}

pub fn put_entry(
    t: &mut [u32],
    slot: u32,
    zoom: Zoom,
    c: [i32; 3],
    key: u32,
    a: &In<[u32; 2]>,
    words: [u32; 4],
) {
    let w = node_cache::entry_at(slot) as usize;
    t[w + node_cache::W_KEY as usize] = key;
    t[w + node_cache::W_KIND as usize] = node_cache::stamped(a.kind, key, a.asset, words, zoom, c);
    t[w + node_cache::W_ASSET as usize] = a.asset[0];
    t[w + node_cache::W_ASSET as usize + 1] = a.asset[1];
    let h = w + node_cache::W_INHERITANCE as usize;
    t[h..h + 4].copy_from_slice(&words);
    let n = w + node_cache::W_NODE as usize;
    t[n] = zoom;
    t[n + 1] = c[0] as u32;
    t[n + 2] = c[1] as u32;
    t[n + 3] = c[2] as u32;
}

/// What the world answers of a node with nothing handed down, what it hands on included:
/// what a stackless walk asks.
pub fn truth(
    w: World,
    cuts: [Cut; 2],
    zoom: Zoom,
    c: [i32; 3],
    zooms: [Zoom; 2],
) -> (In<[u32; 2]>, [u32; 4]) {
    let pool = tree::Read(w.pool);
    let mut held = w;
    let mut at = Inheritance::of(&w, (zoom, c));
    let a = world::node(&mut held, &w, &w, &pool, &mut at, (zoom, c), zooms, cuts);
    (a, at.words())
}

pub fn q8(units: [i32; 3]) -> [i32; 3] {
    [
        units[0].wrapping_shl(FRAC_W) | 5,
        units[1].wrapping_shl(FRAC_W) | 77,
        units[2].wrapping_shl(FRAC_W) | 13,
    ]
}

pub fn bytes(words: &[u32]) -> &[u8] {
    // SAFETY: every u32's bytes are u8s, in the slice's own lifetime.
    unsafe { std::slice::from_raw_parts(words.as_ptr().cast(), words.len() * 4) }
}

pub struct Gpu {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
}

impl Gpu {
    pub fn buffer(&self, words: &[u32], usage: wgpu::BufferUsages) -> wgpu::Buffer {
        use wgpu::util::DeviceExt as _;
        self.device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: None,
                contents: bytes(words),
                usage: usage | wgpu::BufferUsages::COPY_SRC,
            })
    }

    pub fn read(&self, from: &wgpu::Buffer) -> Vec<u32> {
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
        self.device
            .poll(wgpu::PollType::wait_indefinitely())
            .unwrap();
        let v = staging.slice(..).get_mapped_range().unwrap();
        v.as_chunks::<4>()
            .0
            .iter()
            .map(|b| u32::from_le_bytes(*b))
            .collect()
    }

    /// The pipeline of `entry` in `source`, or in the module the file `MUT_WGSL` names where
    /// that holds the entry: a mutant sweep's.
    pub fn pipeline(&self, source: &str, entry: &str) -> wgpu::ComputePipeline {
        let mutant = std::env::var_os("MUT_WGSL")
            .map(|p| std::fs::read_to_string(p).expect("the mutant's module"))
            .filter(|t| t.contains(&format!("fn {entry}(")));
        let text = mutant.unwrap_or_else(|| source.to_string());
        let module = self
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some(entry),
                source: wgpu::ShaderSource::Wgsl(text.into()),
            });
        self.device
            .create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some(entry),
                layout: None,
                module: &module,
                entry_point: Some(entry),
                compilation_options: Default::default(),
                cache: None,
            })
    }

    pub fn dispatch(
        &self,
        p: &wgpu::ComputePipeline,
        binds: &[(u32, wgpu::BindingResource)],
        groups: [u32; 2],
    ) {
        let entries: Vec<_> = binds
            .iter()
            .map(|(b, resource)| wgpu::BindGroupEntry {
                binding: *b,
                resource: resource.clone(),
            })
            .collect();
        let group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &p.get_bind_group_layout(0),
            entries: &entries,
        });
        // A word the same command buffer clears after the pass: one Metal aborts mid-way runs
        // nothing after, and wgpu counts it done all the same.
        let ran = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: 4,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: true,
        });
        ran.slice(..)
            .get_mapped_range_mut()
            .unwrap()
            .copy_from_slice(&[0xff; 4]);
        ran.unmap();
        let mut e = self.device.create_command_encoder(&Default::default());
        {
            let mut pass = e.begin_compute_pass(&Default::default());
            pass.set_pipeline(p);
            pass.set_bind_group(0, &group, &[]);
            pass.dispatch_workgroups(groups[0], groups[1], 1);
        }
        e.clear_buffer(&ran, 0, None);
        self.queue.submit([e.finish()]);
        ran.slice(..).map_async(wgpu::MapMode::Read, |r| r.unwrap());
        self.device
            .poll(wgpu::PollType::wait_indefinitely())
            .unwrap();
        let cleared = ran
            .slice(..)
            .get_mapped_range()
            .unwrap()
            .iter()
            .all(|&b| b == 0);
        assert!(
            cleared,
            "a dispatch of {groups:?} workgroups: the command buffer did not run to its end"
        );
    }

    pub fn map(&self, size: u32) -> wgpu::Texture {
        self.device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d {
                width: size,
                height: size,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R32Uint,
            usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        })
    }

    pub fn read_map(&self, map: &wgpu::Texture) -> Vec<u32> {
        let size = map.width();
        let staging = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: u64::from(size * size * 4),
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut e = self.device.create_command_encoder(&Default::default());
        e.copy_texture_to_buffer(
            map.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &staging,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(size * 4),
                    rows_per_image: None,
                },
            },
            map.size(),
        );
        self.queue.submit([e.finish()]);
        staging
            .slice(..)
            .map_async(wgpu::MapMode::Read, |r| r.unwrap());
        self.device
            .poll(wgpu::PollType::wait_indefinitely())
            .unwrap();
        let v = staging.slice(..).get_mapped_range().unwrap();
        v.as_chunks::<4>()
            .0
            .iter()
            .map(|b| u32::from_le_bytes(*b))
            .collect()
    }
}

/// What a slot's kept before the walks, and what it must be after.
#[derive(Clone, Copy, PartialEq, Eq, Debug, PartialOrd, Ord)]
pub enum Trap {
    /// Held at other zooms by an earlier frame: written over.
    OtherFrame,
    /// Held at other zooms by this frame's samples: stands, the node kept beside it.
    Beside,
    /// Kept by this pass, a false answer: not read, and not written.
    OwnPass,
    /// At the zooms asked, one word its stamp covers over, as a walk writing leaves them:
    /// asked of the world, and not written.
    TornWords,
    /// Kept by an earlier frame, the world's answer: read, and stands.
    Kept,
    /// Kept by this frame's samples, the world's answer: read, and stands.
    KeptThisFrame,
    /// Kept by this frame's samples, a false answer: what the walk takes, and stands.
    KeptFalse,
    /// A split a moved brick lay under: at no zooms, written over.
    Forgotten,
    /// A walk's claim: no key to read, and it stands; the node kept in the next free slot.
    Claimed,
    /// Its bucket full of other nodes: the one its hash names written over, the rest stand.
    Victim,
    /// Its bucket all claimed: none written.
    AllClaimed,
    /// Its first slot another node's: the next free one taken.
    NextFree,
    /// Its bucket full of nodes a word apart at the zooms asked, false: none read, the one
    /// its hash names written over.
    NotIt,
}

/// The node `trap` stands on: its zoom and index, the zooms asked, the first slot of its bucket.
#[derive(Clone, Copy)]
pub struct TrapAt {
    pub trap: Trap,
    pub zoom: Zoom,
    pub c: [i32; 3],
    pub zooms: [Zoom; 2],
    pub first: u32,
}

/// Some of the nodes `nodes` the walks visit set up, one a kind a few in turn, each alone in
/// its bucket, so no other node's slot goes with it; a false answer only on a node asked at
/// one pair of zooms, as nothing then takes its slot, in whichever order the walks run. `tags`:
/// an earlier frame's walks', this frame's another pass's, and the walks' own.
pub fn lay_traps(
    table: &mut [u32],
    w: World,
    cuts: [Cut; 2],
    nodes: &Met,
    [earlier, this_samples, own]: [u32; 3],
) -> Vec<TrapAt> {
    const KINDS: [Trap; 13] = [
        Trap::OtherFrame,
        Trap::Beside,
        Trap::OwnPass,
        Trap::TornWords,
        Trap::Kept,
        Trap::KeptThisFrame,
        Trap::KeptFalse,
        Trap::Forgotten,
        Trap::Claimed,
        Trap::Victim,
        Trap::AllClaimed,
        Trap::NextFree,
        Trap::NotIt,
    ];
    let mut bands: BTreeMap<(Zoom, [i32; 3]), u32> = BTreeMap::new();
    for &(z, c, _) in nodes.keys() {
        *bands.entry((z, c)).or_default() += 1;
    }
    let mut in_bucket: BTreeMap<u32, u32> = BTreeMap::new();
    for &(z, c, _) in nodes.keys() {
        if node_cache::keeps(z, c) {
            *in_bucket.entry(node_cache::entry_of(z, c)).or_default() += 1;
        }
    }
    let mut laid = Vec::new();
    let mut buckets = BTreeSet::new();
    let mut turn = 0usize;
    let mut torn = 0u32;
    for (n, &(zoom, c, zooms)) in nodes.keys().enumerate() {
        // Below the top, whose nodes every ray meets, one in so many.
        if zoom < 12 || !node_cache::keeps(zoom, c) || n % 13 != 0 {
            continue;
        }
        let first = node_cache::entry_of(zoom, c);
        if in_bucket[&first] != 1 || !buckets.insert(first) {
            continue;
        }
        let trap = KINDS[turn % KINDS.len()];
        turn += 1;
        if matches!(
            trap,
            Trap::KeptFalse | Trap::OwnPass | Trap::TornWords | Trap::NotIt
        ) && bands[&(zoom, c)] != 1
        {
            continue;
        }
        let (a, words) = truth(w, cuts, zoom, c, zooms);
        let other = [zooms[0] - 1, zooms[1] - 1];
        let (at_other, words_other) = truth(w, cuts, zoom, c, other);
        let wrong = In {
            kind: Kind::Cell,
            asset: if a.kind == Kind::Cell && !answer::transparent(a.asset[0]) {
                answer::AIR
            } else {
                [value::OBJECTS[0], 0]
            },
        };
        // What walks before these kept in its bucket gone.
        let bucket = node_cache::entry_at(first) as usize;
        table[bucket..bucket + (BUCKET * node_cache::ENTRY_WORDS) as usize].fill(0);
        // Another node of the bucket, at a zoom no walk here asks.
        let stranger = |k: i32| [c[0] ^ (k + 1), c[1], c[2]];
        match trap {
            Trap::OtherFrame => {
                put_entry(
                    table,
                    first + 1,
                    zoom,
                    c,
                    node_cache::key(earlier, other),
                    &at_other,
                    words_other,
                );
            }
            Trap::Beside => {
                put_entry(
                    table,
                    first,
                    zoom,
                    c,
                    node_cache::key(this_samples, other),
                    &at_other,
                    words_other,
                );
            }
            Trap::OwnPass => {
                put_entry(
                    table,
                    first,
                    zoom,
                    c,
                    node_cache::key(own, zooms),
                    &wrong,
                    words,
                );
            }
            Trap::TornWords => {
                // A false answer and its stamp, then one word its stamp covers over: taken if
                // that word goes unread.
                let key = node_cache::key(this_samples, zooms);
                put_entry(table, first, zoom, c, key, &wrong, words);
                let e = node_cache::entry_at(first) as usize;
                let kind = e + node_cache::W_KIND as usize;
                let asset = e + node_cache::W_ASSET as usize;
                let handed = e + node_cache::W_INHERITANCE as usize;
                let stamp = |key, zoom, c| {
                    node_cache::stamped(wrong.kind, key, wrong.asset, words, zoom, c)
                };
                torn += 1;
                match torn % 13 {
                    0 => {
                        table[kind] = stamp(
                            node_cache::key(this_samples, [zooms[0] - 1, zooms[1]]),
                            zoom,
                            c,
                        );
                    }
                    1 => {
                        table[kind] = stamp(
                            node_cache::key(this_samples, [zooms[0], zooms[1] - 1]),
                            zoom,
                            c,
                        );
                    }
                    2 => table[kind] ^= 1,
                    3 => table[asset] ^= 1,
                    4 => table[asset + 1] ^= 1,
                    5 => table[handed] ^= 1,
                    6 => table[handed + 1] ^= 1,
                    7 => table[handed + 2] ^= 1,
                    8 => table[handed + 3] ^= 1,
                    9 => table[kind] = stamp(key, zoom + 1, c),
                    10 => table[kind] = stamp(key, zoom, [c[0] ^ 1, c[1], c[2]]),
                    11 => table[kind] = stamp(key, zoom, [c[0], c[1] ^ 1, c[2]]),
                    _ => table[kind] = stamp(key, zoom, [c[0], c[1], c[2] ^ 1]),
                }
            }
            Trap::Kept => {
                put_entry(
                    table,
                    first + 1,
                    zoom,
                    c,
                    node_cache::key(earlier, zooms),
                    &a,
                    words,
                );
            }
            Trap::KeptThisFrame => {
                put_entry(
                    table,
                    first,
                    zoom,
                    c,
                    node_cache::key(this_samples, zooms),
                    &a,
                    words,
                );
            }
            Trap::KeptFalse => {
                put_entry(
                    table,
                    first,
                    zoom,
                    c,
                    node_cache::key(this_samples, zooms),
                    &wrong,
                    words,
                );
            }
            Trap::Forgotten => {
                let key = node_cache::forgotten_split(node_cache::key(earlier, zooms));
                put_entry(table, first, zoom, c, key, &a, words);
            }
            Trap::Claimed => {
                put_entry(table, first, zoom, c, CLAIMED, &a, words);
            }
            Trap::Victim => {
                for k in 0..BUCKET {
                    let key = node_cache::key(earlier, zooms);
                    put_entry(table, first + k, 40, stranger(k as i32), key, &a, words);
                }
            }
            Trap::AllClaimed => {
                for k in 0..BUCKET {
                    put_entry(table, first + k, zoom, c, CLAIMED, &a, words);
                }
            }
            Trap::NextFree => {
                let key = node_cache::key(earlier, zooms);
                put_entry(table, first, 40, stranger(0), key, &a, words);
            }
            Trap::NotIt => {
                // False but for the one a walk writes over, which a thread may read as it does.
                let key = node_cache::key(earlier, zooms);
                let v = node_cache::victim(zoom, c);
                let apart = [
                    (zoom + 1, c),
                    (zoom, [c[0] ^ 1, c[1], c[2]]),
                    (zoom, [c[0], c[1] ^ 1, c[2]]),
                    (zoom, [c[0], c[1], c[2] ^ 1]),
                ];
                for (k, (z, at)) in apart.into_iter().enumerate() {
                    let said = if k as u32 == v { &a } else { &wrong };
                    put_entry(table, first + k as u32, z, at, key, said, words);
                }
            }
        }
        laid.push(TrapAt {
            trap,
            zoom,
            c,
            zooms,
            first,
        });
    }
    laid
}

/// `table` with the false answers `traps` laid gone, for walks other than the frame's they
/// were laid for: another's may write over them in any order.
pub fn strike_traps(table: &mut [u32], traps: &[TrapAt], tag: u32) {
    for l in traps {
        if matches!(
            l.trap,
            Trap::OwnPass | Trap::TornWords | Trap::KeptFalse | Trap::NotIt
        ) {
            for k in 0..BUCKET {
                let w = node_cache::entry_at(l.first + k) as usize;
                if table[w] != CLAIMED
                    && (l.trap == Trap::OwnPass || node_cache::tag_of(table[w]) != tag)
                {
                    table[w] = 0;
                }
            }
        }
    }
}

/// The slots of `table` holding the node at its zooms.
pub fn slots_of(table: &[u32], l: &TrapAt, tag: u32) -> Vec<u32> {
    (0..BUCKET)
        .filter(|&k| {
            let w = node_cache::entry_at(l.first + k) as usize;
            let key = table[w];
            key != CLAIMED
                && node_cache::tag_of(key) == tag
                && node_cache::holds(key, l.zooms)
                && is_node(table, w, l.zoom, l.c)
        })
        .collect()
}

/// Whether `table` holds to what `l` was laid for, no other node asked in its bucket. Two
/// walks may keep a node twice, one finding the other's slot claimed, as `Writing`'s do.
pub fn trap_holds(table: &[u32], before: &[u32], l: &TrapAt, tag: u32) -> bool {
    let same = |k: u32| {
        let w = node_cache::entry_at(l.first + k) as usize;
        let n = node_cache::ENTRY_WORDS as usize;
        table[w..w + n] == before[w..w + n]
    };
    let new = slots_of(table, l, tag);
    match l.trap {
        Trap::OtherFrame => new.contains(&1),
        Trap::Forgotten => new.contains(&0),
        Trap::Beside | Trap::Claimed | Trap::NextFree => same(0) && new.contains(&1),
        Trap::OwnPass
        | Trap::TornWords
        | Trap::Kept
        | Trap::KeptThisFrame
        | Trap::KeptFalse
        | Trap::AllClaimed => (0..BUCKET).all(same),
        Trap::Victim | Trap::NotIt => {
            let v = node_cache::victim(l.zoom, l.c);
            (0..BUCKET).all(|k| k == v || same(k)) && new == [v]
        }
    }
}

/// The world's buffers on the device.
pub struct WorldBufs {
    pub clip: wgpu::Buffer,
    pub ground: wgpu::Buffer,
    pub palette: wgpu::Buffer,
    pub shards: wgpu::Buffer,
    pub placed: wgpu::Buffer,
    pub pool: wgpu::Buffer,
}

pub fn store_of() -> wgpu::BufferUsages {
    wgpu::BufferUsages::STORAGE
}

/// The zooms a key holds, far then near.
pub fn key_zooms(key: u32) -> [Zoom; 2] {
    [
        (key >> node_cache::TAG_BITS) & ((1 << pxl_engine::ZOOM_BITS) - 1),
        key >> (node_cache::TAG_BITS + pxl_engine::ZOOM_BITS),
    ]
}

/// The slots a pass left claimed, which one that ran to its end never does: a claim writes
/// every word, then the key. Each slot's number and words.
pub fn left_claimed(table: &[u32]) -> Vec<(u32, Vec<u32>)> {
    (0..node_cache::ENTRIES)
        .filter_map(|e| {
            let w = node_cache::entry_at(e) as usize;
            (table[w] == CLAIMED).then(|| (e, table[w..w + node_cache::ENTRY_WORDS as usize].to_vec()))
        })
        .collect()
}
