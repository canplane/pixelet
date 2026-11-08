// pxl-render/frame/shader/access.rs

//! The build's check of the frame's calls over the buffers they share. A frame lists its calls
//! in the order the host encodes them (sequence.rs `FRAME`), and `check` refuses, as the crate
//! is built, an order in which a call could read what no call has written yet, or what a call
//! after it overwrites.
//!
//! A pass's access to a binding (read, write, atomic) is its shader's, which build.rs reads
//! off the shader (record.rs `ACCESS`). A buffer's role is what the host does with it (`ARGS`,
//! `LIST`, `STATE`, `INPUT`), and a binding's role in a call follows from that role and from
//! the call (`role`). Where several passes share a buffer, the words each pass touches are
//! declared here (`TOUCHES`). Without a declaration, a pass touches an args buffer through its
//! whole view, and a list or a state not at all.
//!
//! A frame declares more facts about its ranges. `appends` are counts that several passes add
//! to: one fact with several writers. `seeds` are the copies that open such a count before any
//! pass adds to it, so the count starts from the copied value. `bounds` are counts a pass reads
//! as a bound while other passes still add to them; the reader takes what has been added so
//! far, and each bound names how far the count may run past it. `persistent` is what the frame
//! keeps over frames: each kept range with the one step or the one rule that owns it, and what
//! makes it stale.
//!
//! `check` refuses, as the crate is built:
//! 1. a call that reads its count from a buffer that a group set for it binds writable (a
//!    dispatch's group, or any group of a render pass), which WebGPU refuses within one usage
//!    scope;
//! 2. two calls of one stage that write the same word, unless one runs in place of the other
//!    (`Step::one_of`), both add to a count through atomics, or the first is a copy declared
//!    as the seed of a count the second adds to;
//! 3. a call that reads a word no earlier call of the frame wrote, unless the word is kept over
//!    frames; or a call that reads a word a later call of its own stage writes, unless the
//!    reader writes that word too, the later call is the kept word's owner writing it for the
//!    frames after, or the word is a bound;
//! 4. a kept range written by a call other than its owner or the host, an owner that does not
//!    write it, or, where a rule owns the range, a call that applies the rule but not through
//!    atomics.
//!
//! The frame names each hazard it holds today in its known list, and `check` also refuses a
//! known hazard the frame no longer holds.

use crate::frame::shader::record::{compute, lays_out_writable, ACCESS, NONE, STAGES, STORAGE_RW};
use crate::frame::shader::word::SHADOW_SIZE;
use pxl_engine::{light, node_cache, raster, ray};

/// A pass's access to a binding, as its shader uses it: the shader reads it, writes it, or
/// touches it through atomics. The bits combine.
pub const READ: u32 = 1;
pub const WRITE: u32 = 2;
pub const ATOMIC: u32 = 4;

/// A buffer's role. An `ARGS` buffer holds the counts an indirect call reads and the words a
/// pass counts them in. A `LIST` is one a pass appends to and another reads. A `STATE` is what
/// passes keep over frames. An `INPUT` is what the host writes and passes only read.
pub const ARGS: u32 = 1;
pub const LIST: u32 = 2;
pub const STATE: u32 = 3;
pub const INPUT: u32 = 4;
/// The roles a binding takes in a call beyond its buffer's role: `ARGS_INDIRECT` when the call
/// reads its count from the binding's buffer, and `ARGS_WRITTEN` when the call writes args
/// there for a later call.
pub const ARGS_INDIRECT: u32 = 5;
pub const ARGS_WRITTEN: u32 = 6;

/// A call's kind: a compute dispatch, a draw in a render pass, a copy between buffers, or a
/// write by the host. A write by the host lands before the frame's first call.
pub const DISPATCH: u32 = 0;
pub const DRAW: u32 = 1;
pub const COPY: u32 = 2;
pub const WRITE_BY_RUNTIME: u32 = 3;

/// A word count that reaches to the buffer's end.
pub const END: u32 = u32::MAX;
pub const NO_BUFFER: u32 = u32::MAX;

/// A buffer the frame's calls share, by its name and its role.
#[derive(Clone, Copy)]
pub struct Buffer {
    pub name: &'static str,
    pub role: u32,
}

/// Words `at` to `at + words` of buffer `buf` (an index into the frame's `buffers`).
#[derive(Clone, Copy)]
pub struct Range {
    pub buf: u32,
    pub at: u32,
    pub words: u32,
}

pub const NO_RANGE: Range = Range {
    buf: NO_BUFFER,
    at: 0,
    words: 0,
};

/// A group's binding, which views `words` words of buffer `buf` from its word `at`. `words` is
/// `END` for a view to the buffer's end.
#[derive(Clone, Copy)]
pub struct Bind {
    pub binding: u32,
    pub buf: u32,
    pub at: u32,
    pub words: u32,
}

/// A bind group as the host makes it: its layout as (binding, stages, kind) words, and the
/// frame's buffers its bindings view. A binding of the layout that `binds` leaves out views a
/// buffer outside the frame's, which only the host names.
#[derive(Clone, Copy)]
pub struct Group {
    pub layout: &'static [u32],
    pub binds: &'static [Bind],
}

/// The words compute pass `compute` touches through `binding`, relative to the binding's view:
/// those it writes and those it reads, each as (first word, words).
#[derive(Clone, Copy)]
pub struct Touch {
    pub compute: u32,
    pub binding: u32,
    pub writes: &'static [[u32; 2]],
    pub reads: &'static [[u32; 2]],
}

/// A call the host encodes. `stage` is an index into the frame's `stages`. `pipeline` is a
/// compute or a render pipeline. `groups` are the groups set for the call: a dispatch's group 0,
/// or every group its render pass sets. `indirect` is the words the call reads its count from.
/// `from` is a copy's source, and `to` is a copy's or a host write's destination. `one_of` is a
/// tag the call shares with the other calls of its stage of which only one runs in a frame, or
/// 0 for none. `pass` is the render pipelines a draw's render pass draws (`in_pass`).
#[derive(Clone, Copy)]
pub struct Step {
    pub name: &'static str,
    pub stage: u32,
    pub kind: u32,
    pub pipeline: u32,
    pub groups: &'static [&'static Group],
    pub indirect: Range,
    pub from: Range,
    pub to: Range,
    pub one_of: u32,
    pub pass: &'static [u32],
}

impl Step {
    const fn new(name: &'static str, stage: u32, kind: u32) -> Step {
        Step {
            name,
            stage,
            kind,
            pipeline: 0,
            groups: &[],
            indirect: NO_RANGE,
            from: NO_RANGE,
            to: NO_RANGE,
            one_of: 0,
            pass: &[],
        }
    }

    /// A direct dispatch of compute pipeline `compute` over `group`.
    pub const fn dispatch(
        name: &'static str,
        stage: u32,
        compute: u32,
        group: &'static [&'static Group],
    ) -> Step {
        Step {
            pipeline: compute,
            groups: group,
            ..Step::new(name, stage, DISPATCH)
        }
    }

    /// An indirect dispatch of compute pipeline `compute` over `group`, which reads its
    /// workgroup counts from `buf` at word `at`.
    pub const fn dispatch_off(
        name: &'static str,
        stage: u32,
        compute: u32,
        group: &'static [&'static Group],
        (buf, at): (u32, u32),
    ) -> Step {
        Step {
            indirect: Range { buf, at, words: 3 },
            ..Step::dispatch(name, stage, compute, group)
        }
    }

    /// An indirect draw of render pipeline `render` in a pass that sets `groups`, which reads
    /// its count from `buf` at word `at`.
    pub const fn draw_off(
        name: &'static str,
        stage: u32,
        render: u32,
        groups: &'static [&'static Group],
        (buf, at): (u32, u32),
    ) -> Step {
        Step {
            pipeline: render,
            groups,
            indirect: Range { buf, at, words: 4 },
            ..Step::new(name, stage, DRAW)
        }
    }

    /// Indirect draws of render pipeline `render` in a pass that sets `groups`, one draw a
    /// region. Each region's count lies in `buf` where the host lays it out (`Step::off`).
    pub const fn draw_regions(
        name: &'static str,
        stage: u32,
        render: u32,
        groups: &'static [&'static Group],
        buf: u32,
    ) -> Step {
        Step {
            pipeline: render,
            groups,
            indirect: Range {
                buf,
                at: 0,
                words: END,
            },
            ..Step::new(name, stage, DRAW)
        }
    }

    /// A copy of `from` to `to`.
    pub const fn copy(name: &'static str, stage: u32, from: Range, to: Range) -> Step {
        Step {
            from,
            to,
            ..Step::new(name, stage, COPY)
        }
    }

    /// A write of `to` by the host.
    pub const fn write(name: &'static str, to: Range) -> Step {
        Step {
            to,
            ..Step::new(name, 0, WRITE_BY_RUNTIME)
        }
    }

    /// This call, run in place of the other calls of its stage tagged `tag`.
    pub const fn one_of(self, tag: u32) -> Step {
        Step {
            one_of: tag,
            ..self
        }
    }

    /// This draw, in a render pass that draws `pipelines`, its own among them. Every group the
    /// pass sets belongs to one of those pipelines.
    pub const fn in_pass(self, pipelines: &'static [u32]) -> Step {
        Step {
            pass: pipelines,
            ..self
        }
    }

    /// The byte offset of the call's count, as the host's indirect call takes it.
    pub const fn at(&self) -> u32 {
        self.indirect.at * 4
    }

    /// The byte offset `bytes` past the count's, where the host picks one of the step's
    /// regions.
    pub const fn off(&self, bytes: u32) -> u32 {
        self.at() + bytes
    }

    /// The byte offsets of a copy's source, and of a copy's or a host write's destination.
    pub const fn from_at(&self) -> u32 {
        self.from.at * 4
    }
    pub const fn to_at(&self) -> u32 {
        self.to.at * 4
    }
}

/// A frame: its calls in the order the host encodes them, its stages' names, the buffers the
/// calls share, and the facts it declares about its ranges (see the module's comment).
pub struct Frame {
    pub stages: &'static [&'static str],
    pub buffers: &'static [Buffer],
    pub steps: &'static [Step],
    pub appends: &'static [Range],
    pub seeds: &'static [&'static str],
    pub bounds: &'static [Bound],
    pub persistent: &'static [Persistent],
}

/// A count a pass reads as a bound while other passes still add to it. `overrun` is the most
/// the count may run past the bound a reader took, once the adds after the read land; the
/// count's owner keeps it within that.
#[derive(Clone, Copy)]
pub struct Bound {
    pub range: Range,
    pub overrun: u32,
}

/// Whether `r` lies within one of `bounds`.
const fn within_bound(r: Range, bounds: &[Bound]) -> bool {
    let mut k = 0;
    while k < bounds.len() {
        if within(r, &[bounds[k].range]) {
            return true;
        }
        k += 1;
    }
    false
}

/// A range the frame keeps over frames. `owner` is the one step that writes it, together with
/// the steps run in place of that step (`Step::one_of`). Where `by` names steps, `owner` names
/// instead the one rule those steps apply: each applies it through atomics and sets the same
/// value wherever it runs, and the range is an append range. `invalidation` says what makes
/// the range's contents stale.
#[derive(Clone, Copy)]
pub struct Persistent {
    pub range: Range,
    pub owner: &'static str,
    pub by: &'static [&'static str],
    pub invalidation: &'static str,
}

/// A hazard the frame holds today: its rule, 1 to 4, the call it is held at, and what it is
/// held with: the other call, the buffer, or the kept range's owner (`with`).
#[derive(Clone, Copy)]
pub struct KnownHazard {
    pub rule: u32,
    pub step: &'static str,
    pub with: &'static str,
}

/// The first word and the word count of list `which`'s marks in the light's args (pxl-engine
/// `light::list_marks_at`). Each tile has one word, the number of the cast that last listed it.
pub const fn light_marks(which: u32) -> [u32; 2] {
    [
        light::ARGS_WORDS + light::list_marks_at(SHADOW_SIZE, which),
        light::list_cap(SHADOW_SIZE, which),
    ]
}

/// The words each pass touches in a buffer it shares with others, relative to the binding's
/// view (`Bind::at`), as its shader's text indexes them.
pub const TOUCHES: &[Touch] = {
    use crate::frame::shader::record::*;
    /// Words of the light's args (pxl-engine `light::ARGS_WORDS`). `opened` is the x of a
    /// pass's dispatch, which the claim raises as it lists (light_enlist.wgsl `opened`).
    /// `count` is a list's count, and `marks` are a list's marks, which lie past the counts.
    const fn opened(w: u32) -> [u32; 2] {
        [w + 1, 1]
    }
    const fn count(which: u32) -> [u32; 2] {
        [light::ARGS_COUNTS + which, 1]
    }
    const fn marks(which: u32) -> [u32; 2] {
        light_marks(which)
    }
    /// The stood list is the list of the tiles the receivers stand on. These are the words of
    /// its args that a receivers' pass writes as it lists a tile: its dispatch's x, its count
    /// and the slide's x (light_receiver_tile.wgsl `receiver_tiles_put`,
    /// `receiver_tiles_first`).
    const RECEIVER_TILES: [u32; 2] = [light::RECEIVER_TILES_LIST, light::RECEIVER_TILES_SLIDE + 1];
    /// The args of a raster's draw, and the faces' asks a pass lays out at `at`
    /// (record_put.wgsl).
    const DRAW_ARGS: [u32; 2] = [0, raster::ARGS_WORDS];
    const fn asks(at: u32) -> [u32; 2] {
        [at, raster::ARGS_WORDS]
    }
    const QUEUE_HITS: [u32; 2] = [ray::QUEUE_HITS, 4];
    /// The count of the rays a run's walks parked (queue.wgsl `park`).
    const QUEUE_PARKED: [u32; 2] = [ray::QUEUE_PARKED, 1];
    /// The count of the frame's asks so far, in the node cache's head.
    const ASKED: [u32; 2] = [node_cache::ASKED, 1];
    &[
        // The eye's samples' stops lay their faces' asks (sample_box.eye.wgsl); the light's lay
        // none (sample_box.light.wgsl).
        Touch {
            compute: COMPUTE_SAMPLE_BOX,
            binding: 16,
            writes: &[asks(raster::ASKED_AT)],
            reads: &[asks(raster::ASKED_AT)],
        },
        Touch {
            compute: COMPUTE_LIGHT_SAMPLE_BOX,
            binding: 16,
            writes: &[],
            reads: &[],
        },
        // The record's draw counts the drawable entries' boxes into its draw's args, and stands
        // each box's tiles as a receiver of the world's (record_draw.wgsl,
        // light_receiver_tile.wgsl `stand_box`).
        Touch {
            compute: COMPUTE_RECORD_DRAW,
            binding: 16,
            writes: &[DRAW_ARGS],
            reads: &[DRAW_ARGS],
        },
        Touch {
            compute: COMPUTE_LIGHT_RECORD_DRAW,
            binding: 16,
            writes: &[DRAW_ARGS],
            reads: &[DRAW_ARGS],
        },
        Touch {
            compute: COMPUTE_RECORD_DRAW,
            binding: 21,
            writes: &[marks(light::LIST_RECEIVERS)],
            reads: &[marks(light::LIST_RECEIVERS)],
        },
        // The eye's holes' stops lay their faces' asks at a place of their own
        // (record_hits.wgsl, record_hits.eye.wgsl); the light's lay none
        // (record_hits.light.wgsl).
        Touch {
            compute: COMPUTE_RECORD_HITS,
            binding: 16,
            writes: &[asks(raster::HIT_ASKED_AT)],
            reads: &[asks(raster::HIT_ASKED_AT)],
        },
        Touch {
            compute: COMPUTE_LIGHT_RECORD_HITS,
            binding: 16,
            writes: &[],
            reads: &[],
        },
        // The faces' pass reads the asks its group views (face.wgsl).
        Touch {
            compute: COMPUTE_FACE,
            binding: 16,
            writes: &[],
            reads: &[DRAW_ARGS],
        },
        // The receivers' passes mark the tiles they stand on, the world's receivers and the
        // drawn ones alike (light_receiver_tile.wgsl `stand`).
        Touch {
            compute: COMPUTE_LIGHT_MARK_WALKED,
            binding: 21,
            writes: &[marks(light::LIST_RECEIVERS)],
            reads: &[marks(light::LIST_RECEIVERS)],
        },
        Touch {
            compute: COMPUTE_LIGHT_MARK_DRAWS,
            binding: 21,
            writes: &[marks(light::LIST_RECEIVERS)],
            reads: &[marks(light::LIST_RECEIVERS)],
        },
        // The propagation lists the tiles the receivers read (light_propagate.wgsl
        // `light_propagate_cs`).
        Touch {
            compute: COMPUTE_LIGHT_PROPAGATE,
            binding: 21,
            writes: &[count(light::LIST_READS), marks(light::LIST_READS)],
            reads: &[count(light::LIST_READS), marks(light::LIST_READS)],
        },
        // The claim claims each tile read that is due to be cast again, lists what feeds it,
        // and opens the passes that cast those. It also sets the lay's tiles to those read
        // (light_propagate.wgsl `light_damage_cs`, light_enlist.wgsl).
        Touch {
            compute: COMPUTE_LIGHT_DAMAGE,
            binding: 21,
            writes: &[
                [light::ARGS_LAY + 1, 1],
                opened(light::ARGS_CELLS),
                opened(light::ARGS_SAMPLE_BOX),
                opened(light::ARGS_SLOT),
                count(light::LIST_CORNERS),
                count(light::LIST_SOURCES),
                count(light::LIST_DAMAGE),
                marks(light::LIST_CORNERS),
                marks(light::LIST_SOURCES),
                marks(light::LIST_DAMAGE),
            ],
            reads: &[
                opened(light::ARGS_CELLS),
                opened(light::ARGS_SAMPLE_BOX),
                opened(light::ARGS_SLOT),
                count(light::LIST_READS),
                count(light::LIST_CORNERS),
                count(light::LIST_SOURCES),
                count(light::LIST_DAMAGE),
                marks(light::LIST_CORNERS),
                marks(light::LIST_SOURCES),
                marks(light::LIST_DAMAGE),
            ],
        },
        // The receivers' passes and the record's draw write the stood list's args as they list
        // a tile.
        Touch {
            compute: COMPUTE_LIGHT_MARK_WALKED,
            binding: 22,
            writes: &[RECEIVER_TILES],
            reads: &[RECEIVER_TILES],
        },
        Touch {
            compute: COMPUTE_LIGHT_MARK_DRAWS,
            binding: 22,
            writes: &[RECEIVER_TILES],
            reads: &[RECEIVER_TILES],
        },
        Touch {
            compute: COMPUTE_RECORD_DRAW,
            binding: 22,
            writes: &[RECEIVER_TILES],
            reads: &[RECEIVER_TILES],
        },
        // In the node cache's head, the last walk adds to the stops' dispatch and count, and to
        // the frame's asks (queue_last.wgsl).
        Touch {
            compute: COMPUTE_QUEUE_LAST,
            binding: 9,
            writes: &[[node_cache::HITS_ARGS, 4], ASKED],
            reads: &[[node_cache::HITS_ARGS, 4], ASKED],
        },
        // In the holes' queue's words, the walks over the node cache raise the stops' count and
        // dispatch, and the count of the rays they parked (queue_walk.wgsl). The holes' asks
        // add to the frame's asks in the node cache's head (queue_ask.wgsl).
        Touch {
            compute: COMPUTE_QUEUE,
            binding: 10,
            writes: &[QUEUE_HITS, QUEUE_PARKED],
            reads: &[QUEUE_HITS, QUEUE_PARKED],
        },
        Touch {
            compute: COMPUTE_QUEUE_PARKED,
            binding: 10,
            writes: &[QUEUE_HITS],
            reads: &[QUEUE_HITS, QUEUE_PARKED],
        },
        Touch {
            compute: COMPUTE_QUEUE_ASK,
            binding: 9,
            writes: &[ASKED],
            reads: &[ASKED],
        },
        Touch {
            compute: COMPUTE_QUEUE_ASK_PARKED,
            binding: 9,
            writes: &[ASKED],
            reads: &[ASKED],
        },
        // The asks and the last walks of the samples' and the beam rays' runs, the eye's and the
        // light's, count the frame's asks as the holes' do (queue_ask.wgsl, queue_last.wgsl).
        Touch {
            compute: COMPUTE_QUEUE_ASK_SAMPLES,
            binding: 9,
            writes: &[ASKED],
            reads: &[ASKED],
        },
        Touch {
            compute: COMPUTE_QUEUE_ASK_SAMPLES_PARKED,
            binding: 9,
            writes: &[ASKED],
            reads: &[ASKED],
        },
        Touch {
            compute: COMPUTE_QUEUE_ASK_LIGHT_SAMPLES,
            binding: 9,
            writes: &[ASKED],
            reads: &[ASKED],
        },
        Touch {
            compute: COMPUTE_QUEUE_ASK_LIGHT_SAMPLES_PARKED,
            binding: 9,
            writes: &[ASKED],
            reads: &[ASKED],
        },
        Touch {
            compute: COMPUTE_QUEUE_LAST_SAMPLES,
            binding: 9,
            writes: &[ASKED],
            reads: &[ASKED],
        },
        Touch {
            compute: COMPUTE_QUEUE_LAST_LIGHT_SAMPLES,
            binding: 9,
            writes: &[ASKED],
            reads: &[ASKED],
        },
        Touch {
            compute: COMPUTE_QUEUE_ASK_CORNERS,
            binding: 9,
            writes: &[ASKED],
            reads: &[ASKED],
        },
        Touch {
            compute: COMPUTE_QUEUE_ASK_CORNERS_PARKED,
            binding: 9,
            writes: &[ASKED],
            reads: &[ASKED],
        },
        Touch {
            compute: COMPUTE_QUEUE_ASK_LIGHT_CORNERS,
            binding: 9,
            writes: &[ASKED],
            reads: &[ASKED],
        },
        Touch {
            compute: COMPUTE_QUEUE_ASK_LIGHT_CORNERS_PARKED,
            binding: 9,
            writes: &[ASKED],
            reads: &[ASKED],
        },
        Touch {
            compute: COMPUTE_QUEUE_LAST_CORNERS,
            binding: 9,
            writes: &[ASKED],
            reads: &[ASKED],
        },
        Touch {
            compute: COMPUTE_QUEUE_LAST_LIGHT_CORNERS,
            binding: 9,
            writes: &[ASKED],
            reads: &[ASKED],
        },
    ]
};

/// Pass `id`'s access to its binding `binding` of group 0, as its shader uses it (`ACCESS`).
pub const fn access(id: u32, binding: u32) -> u32 {
    let (shader, entry) = compute(id);
    let entries = ACCESS[shader as usize];
    let mut k = 0;
    while k < entries.len() {
        if same(entries[k].0, entry) {
            let by = entries[k].1;
            let mut b = 0;
            while b < by.len() {
                if by[b][0] == 0 && by[b][1] == binding {
                    return by[b][2];
                }
                b += 1;
            }
            return 0;
        }
        k += 1;
    }
    panic!("{}", entry)
}

/// Whether two names are equal, byte for byte. A const fn cannot compare two `str`s with `==`.
pub const fn same(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    if a.len() != b.len() {
        return false;
    }
    let mut k = 0;
    while k < a.len() {
        if a[k] != b[k] {
            return false;
        }
        k += 1;
    }
    true
}

/// The kind `layout` gives `binding`, or None when the layout does not hold it.
const fn kind_in(layout: &[u32], binding: u32) -> Option<u32> {
    let mut e = 0;
    while e + 2 < layout.len() {
        if layout[e] == binding {
            return Some(layout[e + 2]);
        }
        e += 3;
    }
    None
}

/// Pass `compute`'s touch of `binding`, where one is declared.
const fn touch_of(compute: u32, binding: u32) -> Option<&'static Touch> {
    let mut k = 0;
    while k < TOUCHES.len() {
        if TOUCHES[k].compute == compute && TOUCHES[k].binding == binding {
            return Some(&TOUCHES[k]);
        }
        k += 1;
    }
    None
}

const fn end(r: Range) -> u32 {
    if r.words == END {
        END
    } else {
        r.at + r.words
    }
}

/// Whether two ranges share a word.
pub const fn overlap(a: Range, b: Range) -> bool {
    a.buf != NO_BUFFER
        && a.buf == b.buf
        && a.at < end(b)
        && b.at < end(a)
        && a.words != 0
        && b.words != 0
}

/// The `n`th range step `s` writes, when `writing`, or reads, and `NO_RANGE` past the last. A
/// dispatch reads its count, and touches its group's bindings as `TOUCHES` declares, or an args
/// buffer through the binding's whole view. A draw reads its count. A copy reads its source and
/// writes its destination, and a host's write writes its destination.
pub const fn nth(f: &Frame, s: &Step, writing: bool, n: usize) -> Range {
    let mut k = 0;
    match s.kind {
        COPY | WRITE_BY_RUNTIME => {
            let r = if writing { s.to } else { s.from };
            if n == 0 && r.buf != NO_BUFFER {
                return r;
            }
            NO_RANGE
        }
        DRAW => {
            if !writing && n == 0 {
                return s.indirect;
            }
            NO_RANGE
        }
        _ => {
            if !writing && s.indirect.buf != NO_BUFFER {
                if n == 0 {
                    return s.indirect;
                }
                k = 1;
            }
            let binds = s.groups[0].binds;
            let mut b = 0;
            while b < binds.len() {
                let bind = binds[b];
                let a = access(s.pipeline, bind.binding);
                if a & if writing { WRITE } else { READ } != 0 {
                    match touch_of(s.pipeline, bind.binding) {
                        Some(t) => {
                            let words = if writing { t.writes } else { t.reads };
                            let mut w = 0;
                            while w < words.len() {
                                if k == n {
                                    return Range {
                                        buf: bind.buf,
                                        at: bind.at + words[w][0],
                                        words: words[w][1],
                                    };
                                }
                                k += 1;
                                w += 1;
                            }
                        }
                        None if f.buffers[bind.buf as usize].role == ARGS => {
                            if k == n {
                                return Range {
                                    buf: bind.buf,
                                    at: bind.at,
                                    words: END,
                                };
                            }
                            k += 1;
                        }
                        None => {}
                    }
                }
                b += 1;
            }
            NO_RANGE
        }
    }
}

/// Whether step `s` writes buffer `buf` through atomics alone. Only a dispatch can, through the
/// bindings its shader uses atomically; a copy and a host's write are plain writes.
pub const fn writes_atomic(s: &Step, buf: u32) -> bool {
    if s.kind != DISPATCH {
        return false;
    }
    let binds = s.groups[0].binds;
    let mut b = 0;
    while b < binds.len() {
        if binds[b].buf == buf {
            let a = access(s.pipeline, binds[b].binding);
            if a & WRITE != 0 && a & ATOMIC == 0 {
                return false;
            }
        }
        b += 1;
    }
    true
}

/// Whether `r` lies within one of `ranges`.
const fn within(r: Range, ranges: &[Range]) -> bool {
    let mut k = 0;
    while k < ranges.len() {
        let o = ranges[k];
        if o.buf == r.buf && o.at <= r.at && end(r) <= end(o) {
            return true;
        }
        k += 1;
    }
    false
}

/// The kept range `r` lies within, if one does.
const fn kept_of(f: &Frame, r: Range) -> Option<usize> {
    let mut k = 0;
    while k < f.persistent.len() {
        if within(r, &[f.persistent[k].range]) {
            return Some(k);
        }
        k += 1;
    }
    None
}

/// Binding `binding`'s role in step `s`: `ARGS_INDIRECT` when the call reads its count from the
/// binding's buffer, `ARGS_WRITTEN` when a dispatch writes an args buffer there, and otherwise
/// the buffer's own role. It is `NONE` when the call's groups bind none of the frame's buffers
/// there.
pub const fn role(f: &Frame, s: &Step, binding: u32) -> u32 {
    let mut g = 0;
    while g < s.groups.len() {
        let binds = s.groups[g].binds;
        let mut b = 0;
        while b < binds.len() {
            if binds[b].binding == binding {
                let buf = binds[b].buf;
                let role = f.buffers[buf as usize].role;
                if buf == s.indirect.buf {
                    return ARGS_INDIRECT;
                }
                if role == ARGS && s.kind == DISPATCH && access(s.pipeline, binding) & WRITE != 0 {
                    return ARGS_WRITTEN;
                }
                return role;
            }
            b += 1;
        }
        g += 1;
    }
    NONE
}

/// A hazard found: its rule, the step it is held at, the other step, and the buffer. For rule
/// 1 `other` is the group's place, for rule 4 it is the kept range's place, and for rule 3 it
/// is `usize::MAX` when no step wrote the word.
#[derive(Clone, Copy)]
pub struct Hazard {
    pub rule: u32,
    pub step: usize,
    pub other: usize,
    pub buf: u32,
}

/// Refuses a declaration that does not match the shaders or the frame's other declarations: a
/// touch, a kept range, a bound, a seed, a group, a dispatch's layout, a draw's pass or a
/// call's stage. The panic names the pass, the step or the owner at fault.
pub const fn declared(f: &Frame) {
    let mut t = 0;
    while t < TOUCHES.len() {
        let touch = TOUCHES[t];
        let a = access(touch.compute, touch.binding);
        if (a & WRITE == 0 && !touch.writes.is_empty())
            || (a & READ == 0 && !touch.reads.is_empty())
        {
            panic!("{}", compute(touch.compute).1);
        }
        t += 1;
    }
    let mut k = 0;
    while k < f.persistent.len() {
        let kept = &f.persistent[k];
        let mut by = 0;
        while by < kept.by.len() {
            if !named(f, kept.by[by]) {
                panic!("{}", kept.by[by]);
            }
            by += 1;
        }
        if kept.invalidation.is_empty() || (kept.by.is_empty() && !named(f, kept.owner)) {
            panic!("{}", kept.owner);
        }
        k += 1;
    }
    // A bound is a count the frame appends to.
    let mut k = 0;
    while k < f.bounds.len() {
        if !within(f.bounds[k].range, f.appends) {
            panic!("{}", f.buffers[f.bounds[k].range.buf as usize].name);
        }
        k += 1;
    }
    // A seed is a copy of the frame whose destination lies in a count it appends to.
    let mut k = 0;
    while k < f.seeds.len() {
        let mut found = false;
        let mut s = 0;
        while s < f.steps.len() {
            let step = &f.steps[s];
            found |= same(step.name, f.seeds[k]) && step.kind == COPY && within(step.to, f.appends);
            s += 1;
        }
        if !found {
            panic!("{}", f.seeds[k]);
        }
        k += 1;
    }
    let mut k = 0;
    while k < f.steps.len() {
        let s = &f.steps[k];
        let mut g = 0;
        while g < s.groups.len() {
            let group = s.groups[g];
            let mut b = 0;
            while b < group.binds.len() {
                if kind_in(group.layout, group.binds[b].binding).is_none()
                    || group.binds[b].buf as usize >= f.buffers.len()
                {
                    panic!("{}", s.name);
                }
                b += 1;
            }
            g += 1;
        }
        if s.kind == DISPATCH {
            if s.groups.len() != 1 {
                panic!("{}", s.name);
            }
            let read = STAGES[compute(s.pipeline).0 as usize];
            let mut e = 0;
            while e < read.len() {
                if read[e][2] != NONE && kind_in(s.groups[0].layout, read[e][1]).is_none() {
                    panic!("{}", s.name);
                }
                e += 1;
            }
        }
        if s.kind == DRAW && !holds(s.pass, s.pipeline) {
            panic!("{}", s.name);
        }
        if (s.kind == DISPATCH || s.kind == DRAW) && s.stage as usize >= f.stages.len() {
            panic!("{}", s.name);
        }
        k += 1;
    }
}

/// Whether a step of the frame is named `name`.
const fn named(f: &Frame, name: &str) -> bool {
    let mut k = 0;
    while k < f.steps.len() {
        if same(f.steps[k].name, name) {
            return true;
        }
        k += 1;
    }
    false
}

/// Whether `pipelines` holds `pipeline`.
const fn holds(pipelines: &[u32], pipeline: u32) -> bool {
    let mut k = 0;
    while k < pipelines.len() {
        if pipelines[k] == pipeline {
            return true;
        }
        k += 1;
    }
    false
}

/// Rule 1: the place among step `k`'s groups of the group that binds writable the buffer the
/// step reads its count from, if one does. For a draw, a pipeline of its pass that lays out
/// writable storage also counts, at a place past the groups.
const fn bound_writable(f: &Frame, k: usize) -> Option<usize> {
    let s = &f.steps[k];
    if s.indirect.buf == NO_BUFFER {
        return None;
    }
    let mut g = 0;
    while g < s.groups.len() {
        let group = s.groups[g];
        let mut b = 0;
        while b < group.binds.len() {
            let bind = group.binds[b];
            if bind.buf == s.indirect.buf {
                if let Some(STORAGE_RW) = kind_in(group.layout, bind.binding) {
                    return Some(g);
                }
            }
            b += 1;
        }
        g += 1;
    }
    // A whole render pass is one usage scope. A pipeline the pass draws that lays out writable
    // storage may have a group set that binds the count's buffer writable.
    let mut p = 0;
    while p < s.pass.len() {
        if lays_out_writable(s.pass[p]) {
            return Some(s.groups.len() + p);
        }
        p += 1;
    }
    None
}

/// The most steps a frame has, and the most ranges a step writes or reads.
pub const STEP_CAP: usize = 128;
pub const RANGE_CAP: usize = 24;

/// Each step's ranges, written and read (`nth`), worked out once.
struct Table {
    writes: [[Range; RANGE_CAP]; STEP_CAP],
    reads: [[Range; RANGE_CAP]; STEP_CAP],
    written: [usize; STEP_CAP],
    read: [usize; STEP_CAP],
}

const fn table(f: &Frame) -> Table {
    if f.steps.len() > STEP_CAP {
        panic!("more steps than `STEP_CAP`");
    }
    let mut t = Table {
        writes: [[NO_RANGE; RANGE_CAP]; STEP_CAP],
        reads: [[NO_RANGE; RANGE_CAP]; STEP_CAP],
        written: [0; STEP_CAP],
        read: [0; STEP_CAP],
    };
    let mut k = 0;
    while k < f.steps.len() {
        let mut n = 0;
        loop {
            let r = nth(f, &f.steps[k], true, n);
            if r.buf == NO_BUFFER {
                break;
            }
            if n == RANGE_CAP {
                panic!("{}", f.steps[k].name);
            }
            t.writes[k][n] = r;
            n += 1;
        }
        t.written[k] = n;
        n = 0;
        loop {
            let r = nth(f, &f.steps[k], false, n);
            if r.buf == NO_BUFFER {
                break;
            }
            if n == RANGE_CAP {
                panic!("{}", f.steps[k].name);
            }
            t.reads[k][n] = r;
            n += 1;
        }
        t.read[k] = n;
        k += 1;
    }
    t
}

/// Whether step `k` writes what `r` holds.
const fn writes_into(t: &Table, k: usize, r: Range) -> bool {
    let mut n = 0;
    while n < t.written[k] {
        if overlap(t.writes[k][n], r) {
            return true;
        }
        n += 1;
    }
    false
}

/// Whether the step named `name` is declared a seed (`Frame::seeds`).
const fn seeds(f: &Frame, name: &str) -> bool {
    let mut k = 0;
    while k < f.seeds.len() {
        if same(f.seeds[k], name) {
            return true;
        }
        k += 1;
    }
    false
}

/// Whether steps `a` and `b` are one writer: the same step, or run in place of each other.
const fn one_writer(f: &Frame, a: usize, b: usize) -> bool {
    let (x, y) = (&f.steps[a], &f.steps[b]);
    a == b || (x.one_of != 0 && x.one_of == y.one_of && x.stage == y.stage)
}

/// Rule 2: a later step of `a`'s stage that writes a word `a` writes, and the buffer. A count
/// both add to through atomics is allowed, and so is one that `a` opens as its seed, a copy
/// ahead of the steps that add to it (`Frame::appends`). A plain write after an atomic one, or
/// beside another plain one, is refused.
const fn second_writer(f: &Frame, t: &Table, a: usize) -> Option<(usize, u32)> {
    let s = &f.steps[a];
    if s.kind != DISPATCH && s.kind != COPY {
        return None;
    }
    let mut b = a + 1;
    while b < f.steps.len() {
        let other = &f.steps[b];
        if other.stage == s.stage
            && (other.kind == DISPATCH || other.kind == COPY)
            && !one_writer(f, a, b)
        {
            let mut n = 0;
            while n < t.written[a] {
                let w = t.writes[a][n];
                let opens = writes_atomic(s, w.buf) || (s.kind == COPY && seeds(f, s.name));
                let appended = within(w, f.appends) && opens && writes_atomic(other, w.buf);
                if writes_into(t, b, w) && !appended {
                    return Some((b, w.buf));
                }
                n += 1;
            }
        }
        b += 1;
    }
    None
}

/// Rule 3: for a word step `r` reads, the buffer with `usize::MAX` for the step when no earlier
/// step writes the word and the frame does not keep it (`Frame::persistent`); or a later step
/// of `r`'s stage that writes the word, when `r` does not write it itself. Two steps that both
/// write a word and read what they add to fall under rule 2.
const fn unwritten_read(f: &Frame, t: &Table, r: usize) -> Option<(usize, u32)> {
    let s = &f.steps[r];
    let mut n = 0;
    while n < t.read[r] {
        let read = t.reads[r][n];
        let mut before = false;
        let mut w = 0;
        while w < f.steps.len() {
            if w != r && writes_into(t, w, read) {
                if w < r {
                    before = true;
                } else if f.steps[w].stage == s.stage
                    && !writes_into(t, r, read)
                    && !rewrites_persistent(f, w, read)
                    && !within_bound(read, f.bounds)
                    && s.kind != WRITE_BY_RUNTIME
                    && f.steps[w].kind != WRITE_BY_RUNTIME
                    && !one_writer(f, r, w)
                {
                    return Some((w, read.buf));
                }
            }
            w += 1;
        }
        if !before && kept_of(f, read).is_none() {
            return Some((usize::MAX, read.buf));
        }
        n += 1;
    }
    None
}

/// Rule 4: the place of a kept range that step `w` writes without owning it, that `w` owns but
/// does not write, or whose rule `w` applies with a plain write.
const fn not_owner(f: &Frame, t: &Table, w: usize) -> Option<usize> {
    let s = &f.steps[w];
    let mut k = 0;
    while k < f.persistent.len() {
        let kept = f.persistent[k];
        let writes = writes_into(t, w, kept.range);
        let applies = applies(&kept, s.name);
        let owner = same(s.name, kept.owner) || applies;
        let atomic = within(kept.range, f.appends) && writes_atomic(s, kept.range.buf);
        if (writes && !owner && s.kind != WRITE_BY_RUNTIME && !owned_with(f, w, kept.owner))
            || (owner && !writes)
            || (applies && writes && !atomic)
        {
            return Some(k);
        }
        k += 1;
    }
    None
}

/// Whether step `name` is one that applies kept range `kept`'s rule.
const fn applies(kept: &Persistent, name: &str) -> bool {
    let mut k = 0;
    while k < kept.by.len() {
        if same(kept.by[k], name) {
            return true;
        }
        k += 1;
    }
    false
}

/// Whether step `w` runs in place of the step named `owner`.
const fn owned_with(f: &Frame, w: usize, owner: &str) -> bool {
    let mut k = 0;
    while k < f.steps.len() {
        if same(f.steps[k].name, owner) {
            return one_writer(f, w, k);
        }
        k += 1;
    }
    false
}

/// Whether step `w` owns the kept range `read` lies within. An owner writes the range for the
/// frames after, so its write after a read in the same frame is no hazard.
const fn rewrites_persistent(f: &Frame, w: usize, read: Range) -> bool {
    match kept_of(f, read) {
        Some(k) => {
            same(f.steps[w].name, f.persistent[k].owner)
                || owned_with(f, w, f.persistent[k].owner)
                || applies(&f.persistent[k], f.steps[w].name)
        }
        None => false,
    }
}

/// The most hazards a frame is held to.
pub const HAZARDS_CAP: usize = 64;

/// Every hazard the frame holds, step by step and rule by rule within a step, and how many
/// there are.
pub const fn hazards(f: &Frame) -> ([Hazard; HAZARDS_CAP], usize) {
    let none = Hazard {
        rule: 0,
        step: 0,
        other: 0,
        buf: NO_BUFFER,
    };
    let t = table(f);
    let mut out = [none; HAZARDS_CAP];
    let mut n = 0;
    let mut k = 0;
    while k < f.steps.len() {
        let mut each = [none; 4];
        if let Some(kept) = not_owner(f, &t, k) {
            each[3] = Hazard {
                rule: 4,
                step: k,
                other: kept,
                buf: f.persistent[kept].range.buf,
            };
        }
        if let Some(g) = bound_writable(f, k) {
            each[0] = Hazard {
                rule: 1,
                step: k,
                other: g,
                buf: f.steps[k].indirect.buf,
            };
        }
        if let Some((b, buf)) = second_writer(f, &t, k) {
            each[1] = Hazard {
                rule: 2,
                step: k,
                other: b,
                buf,
            };
        }
        if let Some((w, buf)) = unwritten_read(f, &t, k) {
            each[2] = Hazard {
                rule: 3,
                step: k,
                other: w,
                buf,
            };
        }
        let mut e = 0;
        while e < each.len() {
            if each[e].rule != 0 {
                if n == HAZARDS_CAP {
                    panic!("more hazards than `HAZARDS_CAP`");
                }
                out[n] = each[e];
                n += 1;
            }
            e += 1;
        }
        k += 1;
    }
    (out, n)
}

/// What a hazard is held with, as a known hazard names it: the kept range's owner for rule 4,
/// the other step for rule 2 and for rule 3 with a later writer, and otherwise the buffer.
pub const fn with(f: &Frame, found: &Hazard) -> &'static str {
    if found.rule == 4 {
        f.persistent[found.other].owner
    } else if found.rule == 2 || (found.rule == 3 && found.other != usize::MAX) {
        f.steps[found.other].name
    } else {
        f.buffers[found.buf as usize].name
    }
}

/// Whether `known` names `found`.
const fn names(f: &Frame, known: &KnownHazard, found: &Hazard) -> bool {
    known.rule == found.rule
        && same(known.step, f.steps[found.step].name)
        && same(known.with, with(f, found))
}

/// Refuses rule 1 at the step whose count's buffer a group set for it binds writable.
const fn refuse_args_bound_writable(name: &str) {
    panic!("{}", name)
}
/// Refuses rule 2 at the first of two steps of a stage that write one word.
const fn refuse_two_writers(name: &str) {
    panic!("{}", name)
}
/// Refuses rule 3 at the step that reads a word before it is written, and rule 4 at the step
/// that writes a kept range it does not own.
const fn refuse_read_before_written(name: &str) {
    panic!("{}", name)
}
/// A known hazard the frame no longer holds, refused by its step's name.
const fn refuse_known_gone(name: &str) {
    panic!("{}", name)
}

/// Refuses, as the crate is built, a frame whose declarations do not hold (`declared`), a
/// hazard the frame holds that `known` does not name, and a hazard `known` names that the frame
/// no longer holds. Each refusal panics with the step's name inside a function of its own
/// rule, so the build's error says which rule refused the frame.
pub const fn check(f: &Frame, known: &[KnownHazard]) {
    declared(f);
    let (found, n) = hazards(f);
    let mut k = 0;
    while k < n {
        let mut is_known = false;
        let mut m = 0;
        while m < known.len() {
            is_known |= names(f, &known[m], &found[k]);
            m += 1;
        }
        if !is_known {
            let name = f.steps[found[k].step].name;
            match found[k].rule {
                1 => refuse_args_bound_writable(name),
                2 => refuse_two_writers(name),
                _ => refuse_read_before_written(name),
            }
        }
        k += 1;
    }
    let mut m = 0;
    while m < known.len() {
        let mut held = false;
        let mut k = 0;
        while k < n {
            held |= names(f, &known[m], &found[k]);
            k += 1;
        }
        if !held {
            refuse_known_gone(known[m].step);
        }
        m += 1;
    }
}

/// Every hazard the frame holds, each as its rule, its step and what it is held with.
pub fn found(f: &Frame) -> Vec<(u32, &'static str, &'static str)> {
    let (found, n) = hazards(f);
    found[..n]
        .iter()
        .map(|h| (h.rule, f.steps[h.step].name, with(f, h)))
        .collect()
}
