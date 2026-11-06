// pxl-render/frame/shader/access.rs

//! What each compute pass does with each buffer it binds, and the hazards the build refuses
//! over the frame's calls in the order the host encodes them (gpu/ray/sequence.rs `FRAME`).
//!
//! A pass's access to a binding (read, write, atomic) is its shader's, read off it by build.rs
//! (record.rs `ACCESS`). A buffer's role is what the host does with it (`ARGS`, `LIST`,
//! `STATE`, `INPUT`); a binding's role in a call follows from it and from the call
//! (`role`). The words a pass touches in a buffer shared by several are declared here
//! (`TOUCHES`); where none are, an args buffer is touched through its whole view and a list
//! or a state not at all.
//!
//! `check` refuses, as the crate is built:
//! 1. a call that reads its count from a buffer a group set for it binds writable (a
//!    dispatch's groups, every group of a render pass), which WebGPU refuses in one usage
//!    scope;
//! 2. two calls of one stage that write one word, one of them not run in place of the other
//!    (`Step::one_of`);
//! 3. a call that reads a word no call before it in the frame wrote, or that a later call of
//!    its own stage writes, a word it does not write itself.
//!
//! Each hazard the frame holds today is named in its known list, and one named there that it
//! no longer holds is refused as well.

use crate::frame::shader::record::{compute, ACCESS, NONE, STAGES, STORAGE_RW};
use pxl_engine::{count, light, node_cache, raster, ray};

/// A pass's access to a binding, as its shader uses it: read, written, and through atomics.
pub const READ: u32 = 1;
pub const WRITE: u32 = 2;
pub const ATOMIC: u32 = 4;

/// A buffer's role: the counts an indirect call reads and the words a pass counts them in; a
/// list a pass appends to and another reads; a state passes keep over frames; what the host
/// writes and passes only read.
pub const ARGS: u32 = 1;
pub const LIST: u32 = 2;
pub const STATE: u32 = 3;
pub const INPUT: u32 = 4;
/// A binding's role in a call past its buffer's: the args the call reads its count from, and
/// args it writes for a later call.
pub const ARGS_INDIRECT: u32 = 5;
pub const ARGS_WRITTEN: u32 = 6;

/// A call's kind: a compute dispatch, a render pass's draw, a copy between buffers, and a
/// write by the host, which lands before the frame's first call.
pub const DISPATCH: u32 = 0;
pub const DRAW: u32 = 1;
pub const COPY: u32 = 2;
pub const WRITE_BY_HOST: u32 = 3;

/// To a buffer's end.
pub const END: u32 = u32::MAX;
/// No buffer.
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

/// A group's binding viewing buffer `buf` from its word `at`.
#[derive(Clone, Copy)]
pub struct Bind {
    pub binding: u32,
    pub buf: u32,
    pub at: u32,
}

/// A group as the host makes it: its layout's (binding, stages, kind) words and the frame's
/// buffers its bindings view; a binding to none of them is not named.
#[derive(Clone, Copy)]
pub struct Group {
    pub layout: &'static [u32],
    pub binds: &'static [Bind],
}

/// The words compute pass `compute` touches through `binding`, within its view, each
/// (from, words): those it writes and those it reads.
#[derive(Clone, Copy)]
pub struct Touch {
    pub compute: u32,
    pub binding: u32,
    pub writes: &'static [[u32; 2]],
    pub reads: &'static [[u32; 2]],
}

/// A call the host encodes: its name, its stage (an index into the frame's `stages`) and its
/// kind; its pipeline, compute or render; the groups set for it (a dispatch's group 0, every
/// group set in a draw's render pass); the words it reads its count from; a copy's source and
/// its and a host write's destination; and a tag it shares with the calls of its stage of which
/// one runs in a frame (0, none).
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
        }
    }

    /// Compute pipeline `compute` dispatched over `group`, its workgroups its own.
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

    /// The same, its workgroups read off `buf` at word `at`.
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

    /// Render pipeline `render` drawn in a pass setting `groups`, its count read off `buf` at
    /// word `at`.
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

    /// `from` copied to `to`.
    pub const fn copy(name: &'static str, stage: u32, from: Range, to: Range) -> Step {
        Step {
            from,
            to,
            ..Step::new(name, stage, COPY)
        }
    }

    /// `to` written by the host.
    pub const fn write(name: &'static str, to: Range) -> Step {
        Step {
            to,
            ..Step::new(name, 0, WRITE_BY_HOST)
        }
    }

    /// The same, run in place of the others of its stage tagged `tag`.
    pub const fn one_of(self, tag: u32) -> Step {
        Step {
            one_of: tag,
            ..self
        }
    }

    /// The byte its count is read from, as the host's indirect call takes it.
    pub const fn at(&self) -> u32 {
        self.indirect.at * 4
    }
}

/// The frame's calls in the order the host encodes them, its stages' names, and the buffers
/// they share.
pub struct Frame {
    pub stages: &'static [&'static str],
    pub buffers: &'static [Buffer],
    pub steps: &'static [Step],
}

/// A hazard the frame holds today: its rule (1, 2 or 3), the call and the other call or the
/// buffer it holds it with.
#[derive(Clone, Copy)]
pub struct Known {
    pub rule: u32,
    pub step: &'static str,
    pub with: &'static str,
}

/// The words each pass touches where it shares a buffer with others, within the binding's view
/// (the binding's group's `Bind::at`), as its shader's text indexes them.
pub const TOUCHES: &[Touch] = {
    use crate::frame::shader::record::*;
    use crate::gpu::shadow::SIZE;
    /// The light's args (pxl-engine `light::ARGS_WORDS`): a pass's dispatch's x, raised by
    /// the claim as it lists (light_enlist.wgsl `opened`); the claims' counts; each list's
    /// count; each list's marks past the counts, the drawn receivers' and the rings'.
    const fn opened(w: u32) -> [u32; 2] {
        [w + 1, 1]
    }
    const fn count(which: u32) -> [u32; 2] {
        [light::ARGS_COUNTS + which, 1]
    }
    const fn marks(which: u32) -> [u32; 2] {
        [
            light::ARGS_WORDS + light::listed_marks_at(SIZE, which),
            light::listed_room(SIZE, which),
        ]
    }
    const DRAWN_MARKS: [u32; 2] = [
        light::ARGS_WORDS + light::drawn_marks_at(SIZE),
        light::tiles(SIZE) * light::tiles(SIZE),
    ];
    const RINGS: [u32; 2] = [
        light::ARGS_WORDS + light::rings_at(SIZE),
        light::rings(SIZE),
    ];
    /// The stood list's args a receivers' pass lists a tile by: its dispatch's x, its count
    /// and the slide's x (light_stood.wgsl `stood_put`, `stood_first`).
    const STOOD: [u32; 2] = [light::STOOD_LIST, light::STOOD_SLIDE + 1];
    /// A raster's draw's args, and the faces' asks a pass lays at `at` (record_put.wgsl).
    const DRAW_ARGS: [u32; 2] = [0, raster::ARGS_WORDS];
    const fn asks(at: u32) -> [u32; 2] {
        [at, raster::ARGS_WORDS]
    }
    const DEFERRED_CNT: [u32; 2] = [raster::DEFERRED_CNT, raster::DEFERRED_WORDS];
    const QUEUE_HITS: [u32; 2] = [ray::QUEUE_HITS, 4];
    &[
        // The eye's samples into its record, their faces' asks (sample_box.eye.wgsl); the
        // light's lay none (sample_box.light.wgsl).
        Touch {
            compute: COMPUTE_SAMPLE_BOX,
            binding: 16,
            writes: &[asks(raster::ASKED_AT)],
            reads: &[asks(raster::ASKED_AT)],
        },
        Touch {
            compute: COMPUTE_SAMPLE_OBSERVE,
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
        Touch {
            compute: COMPUTE_LIGHT_SAMPLE_OBSERVE,
            binding: 16,
            writes: &[],
            reads: &[],
        },
        // The record's boxes counted into its draw's args, each box's tiles stood on by the
        // world's receivers (record_draw.wgsl, light_stood.wgsl `stand_world_box`).
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
            writes: &[marks(light::LISTED_STOOD)],
            reads: &[marks(light::LISTED_STOOD)],
        },
        // The holes' stops: those put off counted, their dispatch raised, and the eye's faces'
        // asks laid at their own place (record_stops.wgsl, record_stops.eye.wgsl); the light's
        // lay none (record_stops.light.wgsl).
        Touch {
            compute: COMPUTE_RECORD_STOPS,
            binding: 16,
            writes: &[DEFERRED_CNT, asks(raster::STOP_ASKED_AT)],
            reads: &[DEFERRED_CNT, asks(raster::STOP_ASKED_AT)],
        },
        Touch {
            compute: COMPUTE_LIGHT_RECORD_STOPS,
            binding: 16,
            writes: &[DEFERRED_CNT],
            reads: &[DEFERRED_CNT],
        },
        Touch {
            compute: COMPUTE_RECORD_DEFERRED,
            binding: 16,
            writes: &[asks(raster::STOP_ASKED_AT)],
            reads: &[DEFERRED_CNT, asks(raster::STOP_ASKED_AT)],
        },
        Touch {
            compute: COMPUTE_LIGHT_RECORD_DEFERRED,
            binding: 16,
            writes: &[],
            reads: &[DEFERRED_CNT],
        },
        // The faces' pass reads the asks its group views (face.wgsl).
        Touch {
            compute: COMPUTE_FACE,
            binding: 16,
            writes: &[],
            reads: &[DRAW_ARGS],
        },
        // The receivers' marks: the tiles stood on, the world's and the drawn's
        // (light_stood.wgsl).
        Touch {
            compute: COMPUTE_LIGHT_MARK_WALKED,
            binding: 21,
            writes: &[marks(light::LISTED_STOOD)],
            reads: &[marks(light::LISTED_STOOD)],
        },
        Touch {
            compute: COMPUTE_LIGHT_MARK_DRAWS,
            binding: 21,
            writes: &[DRAWN_MARKS],
            reads: &[marks(light::LISTED_STOOD), DRAWN_MARKS],
        },
        // The propagation: the tiles read and wanted listed, each wanted's ring counted, and
        // the lay's tiles raised (light_propagate.wgsl).
        Touch {
            compute: COMPUTE_LIGHT_PROPAGATE,
            binding: 21,
            writes: &[
                [light::ARGS_LAY + 1, 1],
                count(light::LISTED_READ),
                count(light::LISTED_WANTED),
                marks(light::LISTED_READ),
                RINGS,
            ],
            reads: &[
                [light::ARGS_LAY + 1, 1],
                count(light::LISTED_READ),
                count(light::LISTED_WANTED),
                marks(light::LISTED_READ),
                RINGS,
            ],
        },
        // The claim: the claims counted, the claimed, fed and cornered listed, and the passes
        // that cast them opened (light_claim.wgsl, light_enlist.wgsl).
        Touch {
            compute: COMPUTE_LIGHT_CLAIM,
            binding: 21,
            writes: &[
                opened(light::ARGS_BEAM),
                opened(light::ARGS_CELLS),
                opened(light::ARGS_SAMPLE),
                opened(light::ARGS_SAMPLE_BOX),
                opened(light::ARGS_SLOT),
                [light::ARGS_PLACED, 1],
                [light::ARGS_STRADDLE, 1],
                [light::ARGS_LEFT, 1],
                count(light::LISTED_CORNERS),
                count(light::LISTED_FED),
                count(light::LISTED_CLAIMED),
                marks(light::LISTED_CORNERS),
                marks(light::LISTED_FED),
                marks(light::LISTED_CLAIMED),
            ],
            reads: &[
                opened(light::ARGS_BEAM),
                opened(light::ARGS_CELLS),
                opened(light::ARGS_SAMPLE),
                opened(light::ARGS_SAMPLE_BOX),
                opened(light::ARGS_SLOT),
                [light::ARGS_PLACED, 1],
                [light::ARGS_STRADDLE, 1],
                [light::ARGS_LEFT, 1],
                count(light::LISTED_CORNERS),
                count(light::LISTED_FED),
                count(light::LISTED_CLAIMED),
                count(light::LISTED_WANTED),
                marks(light::LISTED_CORNERS),
                marks(light::LISTED_FED),
                marks(light::LISTED_CLAIMED),
                RINGS,
            ],
        },
        // The stood list's args a receivers' pass or the record's draw lists a tile by.
        Touch {
            compute: COMPUTE_LIGHT_MARK_WALKED,
            binding: 22,
            writes: &[STOOD],
            reads: &[STOOD],
        },
        Touch {
            compute: COMPUTE_LIGHT_MARK_DRAWS,
            binding: 22,
            writes: &[STOOD],
            reads: &[STOOD],
        },
        Touch {
            compute: COMPUTE_RECORD_DRAW,
            binding: 22,
            writes: &[STOOD],
            reads: &[STOOD],
        },
        // The node cache's head: the stops' dispatch raised by the last walk, and each
        // observer's owed counted (queue_last.wgsl).
        Touch {
            compute: COMPUTE_QUEUE_LAST,
            binding: 9,
            writes: &[
                [node_cache::HITS_ARGS, 1],
                [node_cache::OWED_AT, count::OBSERVERS],
            ],
            reads: &[
                [node_cache::HITS_ARGS, 1],
                [node_cache::OWED_AT, count::OBSERVERS],
            ],
        },
        // The holes' queue's words: the stops' count and dispatch its walks over the node cache
        // raise (queue_walk.wgsl), which the last walk goes on from.
        Touch {
            compute: COMPUTE_QUEUE,
            binding: 10,
            writes: &[QUEUE_HITS],
            reads: &[QUEUE_HITS],
        },
        Touch {
            compute: COMPUTE_QUEUE_PARKED,
            binding: 10,
            writes: &[QUEUE_HITS],
            reads: &[QUEUE_HITS],
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

/// Whether two names are one.
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

/// The kind `layout` lays `binding` out as, none where it lays it out not at all.
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

/// The `n`th range `s` writes (`writing`) or reads, `NO_RANGE` past the last: a dispatch's
/// through its group's bindings, as its touches declare or, an args buffer, through its whole
/// view, and the count it reads; a draw's count; a copy's source and destination; a host's
/// write's destination.
pub const fn nth(f: &Frame, s: &Step, writing: bool, n: usize) -> Range {
    let mut k = 0;
    match s.kind {
        COPY | WRITE_BY_HOST => {
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

/// Binding `binding`'s role in step `s`: its buffer's, or args the call reads its count from
/// or writes; `NONE` where the call's groups bind none of the frame's buffers there.
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

/// A hazard: its rule, the step it is held at, and the other step or the buffer.
#[derive(Clone, Copy)]
pub struct Found {
    pub rule: u32,
    pub step: usize,
    pub other: usize,
    pub buf: u32,
}

/// The declarations held to the shaders and to each other, a declaration that is not refused
/// by the step's name: every binding a group names in its layout; a dispatch's group laying out
/// every binding its shader reads; and each touch of a binding its pass uses, as it uses it.
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
        if (s.kind == DISPATCH || s.kind == DRAW) && s.stage as usize >= f.stages.len() {
            panic!("{}", s.name);
        }
        k += 1;
    }
}

/// Rule 1: where a group set for step `k`'s call binds writable the buffer it reads its count
/// from, the group's place among its groups.
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

/// Whether steps `a` and `b` are one writer: the same step, or run in place of each other.
const fn one_writer(f: &Frame, a: usize, b: usize) -> bool {
    let (x, y) = (&f.steps[a], &f.steps[b]);
    a == b || (x.one_of != 0 && x.one_of == y.one_of && x.stage == y.stage)
}

/// Rule 2: the step after `a` in its stage, and the buffer, where both write one word.
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
                if writes_into(t, b, t.writes[a][n]) {
                    return Some((b, t.writes[a][n].buf));
                }
                n += 1;
            }
        }
        b += 1;
    }
    None
}

/// Rule 3: where a word step `r` reads is written by no step before it, the buffer; or, a word
/// it does not write itself, by a later step of its own stage, that step. Two that write a
/// word, each reading what it adds to, are rule 2's.
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
                    && s.kind != WRITE_BY_HOST
                    && f.steps[w].kind != WRITE_BY_HOST
                    && !one_writer(f, r, w)
                {
                    return Some((w, read.buf));
                }
            }
            w += 1;
        }
        if !before {
            return Some((usize::MAX, read.buf));
        }
        n += 1;
    }
    None
}

/// The most hazards a frame is held to.
pub const FOUND_CAP: usize = 64;

/// Every hazard the frame holds, rule by rule, step by step, and how many.
pub const fn all_found(f: &Frame) -> ([Found; FOUND_CAP], usize) {
    let none = Found {
        rule: 0,
        step: 0,
        other: 0,
        buf: NO_BUFFER,
    };
    let t = table(f);
    let mut out = [none; FOUND_CAP];
    let mut n = 0;
    let mut k = 0;
    while k < f.steps.len() {
        let mut each = [none; 3];
        if let Some(g) = bound_writable(f, k) {
            each[0] = Found {
                rule: 1,
                step: k,
                other: g,
                buf: f.steps[k].indirect.buf,
            };
        }
        if let Some((b, buf)) = second_writer(f, &t, k) {
            each[1] = Found {
                rule: 2,
                step: k,
                other: b,
                buf,
            };
        }
        if let Some((w, buf)) = unwritten_read(f, &t, k) {
            each[2] = Found {
                rule: 3,
                step: k,
                other: w,
                buf,
            };
        }
        let mut e = 0;
        while e < each.len() {
            if each[e].rule != 0 {
                if n == FOUND_CAP {
                    panic!("more hazards than `FOUND_CAP`");
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

/// What a hazard is held with, as a known one names it: the other step, or the buffer where
/// there is none (rule 1, rule 3 with no writer before).
pub const fn with(f: &Frame, found: &Found) -> &'static str {
    if found.rule == 2 || (found.rule == 3 && found.other != usize::MAX) {
        f.steps[found.other].name
    } else {
        f.buffers[found.buf as usize].name
    }
}

/// Whether `known` names `found`.
const fn names(f: &Frame, known: &Known, found: &Found) -> bool {
    known.rule == found.rule
        && same(known.step, f.steps[found.step].name)
        && same(known.with, with(f, found))
}

/// Rule 1 refused at the step whose count a group set for it binds writable.
const fn refuse_args_bound_writable(name: &str) {
    panic!("{}", name)
}
/// Rule 2 refused at the first of two steps of a stage that write one word.
const fn refuse_two_writers(name: &str) {
    panic!("{}", name)
}
/// Rule 3 refused at the step that reads a word before it is written.
const fn refuse_read_before_written(name: &str) {
    panic!("{}", name)
}
/// A known hazard the frame no longer holds, refused by its step's name.
const fn refuse_known_gone(name: &str) {
    panic!("{}", name)
}

/// The frame's declarations held (`declared`), and every hazard it holds one `known` names,
/// each of them one it holds; refused at the step's name, in the function of its rule.
pub const fn check(f: &Frame, known: &[Known]) {
    declared(f);
    let (found, n) = all_found(f);
    let mut k = 0;
    while k < n {
        let mut listed = false;
        let mut m = 0;
        while m < known.len() {
            listed |= names(f, &known[m], &found[k]);
            m += 1;
        }
        if !listed {
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
    let (found, n) = all_found(f);
    found[..n]
        .iter()
        .map(|h| (h.rule, f.steps[h.step].name, with(f, h)))
        .collect()
}
