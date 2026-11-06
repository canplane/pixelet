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
//! A frame declares more facts of a range: a count several passes add to (`appends`), one
//! fact with several appenders; the copy that opens such a count before any of them adds to it
//! (`seeds`), its value what the frame starts counting from; a count read as a bound while it
//! is added to (`bounds`), a reader taking what is added so far, its overrun named; and what it keeps over frames
//! (`kept`), each kept range with the one step or the one rule that owns it and what makes it
//! stale.
//!
//! `check` refuses, as the crate is built:
//! 1. a call that reads its count from a buffer a group set for it binds writable (a
//!    dispatch's groups, every group of a render pass), which WebGPU refuses in one usage
//!    scope;
//! 2. two calls of one stage that write one word, one of them not run in place of the other
//!    (`Step::one_of`), but a count both add to through atomics, or that the first, a copy
//!    declared its seed, opens before the second adds to it;
//! 3. a call that reads a word no call before it in the frame wrote, unless the word is kept
//!    over frames, or that a later call of its own stage writes, a word it does not write
//!    itself, but the kept word's owner writing it for the frames after and a bound;
//! 4. a kept range written by a call that is not its owner, or an owner that writes it not,
//!    or, a rule its owner, a call that applies it but not through atomics.
//!
//! Each hazard the frame holds today is named in its known list, and one named there that it
//! no longer holds is refused as well.

use crate::frame::shader::record::{compute, lays_out_writable, ACCESS, NONE, STAGES, STORAGE_RW};
use crate::gpu::shadow::SIZE;
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

    /// Render pipeline `render` drawn in a pass setting `groups`, a draw a region, each region's
    /// count in `buf` where the host lays it (`Step::off`).
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

    /// The same, a draw in a render pass that draws `pipelines` (its own among them), every
    /// group of which the pass sets is one of theirs.
    pub const fn in_pass(self, pipelines: &'static [u32]) -> Step {
        Step {
            pass: pipelines,
            ..self
        }
    }

    /// The byte its count is read from, as the host's indirect call takes it.
    pub const fn at(&self) -> u32 {
        self.indirect.at * 4
    }

    /// The byte `bytes` past it, where the host picks a region among the step's.
    pub const fn off(&self, bytes: u32) -> u32 {
        self.at() + bytes
    }

    /// The bytes a copy's source and a copy's or a host's write's destination start at.
    pub const fn from_at(&self) -> u32 {
        self.from.at * 4
    }
    pub const fn to_at(&self) -> u32 {
        self.to.at * 4
    }
}

/// The frame's calls in the order the host encodes them, its stages' names, and the buffers
/// they share.
pub struct Frame {
    pub stages: &'static [&'static str],
    pub buffers: &'static [Buffer],
    pub steps: &'static [Step],
    pub appends: &'static [Range],
    pub seeds: &'static [&'static str],
    pub bounds: &'static [Bound],
    pub kept: &'static [Kept],
}

/// A count read as a bound while it is added to, and the most it may run past the bound a
/// reader took before the adds after it land, as its owner bounds it.
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

/// A range kept over frames: its words; the one step that writes them (or those run in place
/// of it, `Step::one_of`), or, where `by` names the steps that apply it, the one rule they
/// apply, each through atomics to a value it sets alike wherever it is applied, the range an
/// append range; and what makes what it holds stale, said.
#[derive(Clone, Copy)]
pub struct Kept {
    pub range: Range,
    pub owner: &'static str,
    pub by: &'static [&'static str],
    pub invalidation: &'static str,
}

/// A hazard the frame holds today: its rule (1 to 4), the call and the other call, the buffer
/// or the kept range's owner it holds it with.
#[derive(Clone, Copy)]
pub struct Known {
    pub rule: u32,
    pub step: &'static str,
    pub with: &'static str,
}

/// List `which`'s marks in the light's args (pxl-engine `light::listed_marks_at`): a word a
/// tile, the number of the cast that last listed it.
pub const fn light_marks(which: u32) -> [u32; 2] {
    [
        light::ARGS_WORDS + light::listed_marks_at(SIZE, which),
        light::listed_room(SIZE, which),
    ]
}

/// The words each pass touches where it shares a buffer with others, within the binding's view
/// (the binding's group's `Bind::at`), as its shader's text indexes them.
pub const TOUCHES: &[Touch] = {
    use crate::frame::shader::record::*;
    /// The light's args (pxl-engine `light::ARGS_WORDS`): a pass's dispatch's x, raised by
    /// the claim as it lists (light_enlist.wgsl `opened`); the claims' counts; each list's
    /// count; each list's marks past the counts, and the rings'.
    const fn opened(w: u32) -> [u32; 2] {
        [w + 1, 1]
    }
    const fn count(which: u32) -> [u32; 2] {
        [light::ARGS_COUNTS + which, 1]
    }
    const fn marks(which: u32) -> [u32; 2] {
        light_marks(which)
    }
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
    const QUEUE_HITS: [u32; 2] = [ray::QUEUE_HITS, 4];
    const QUEUE_VISITED: [u32; 2] = [ray::QUEUE_VISITED, 1];
    /// The frame's runs' work so far in the node cache's head, and every run's caps.
    const WORK_DONE: [u32; 2] = [node_cache::VISITED, 2];
    const VISITS_DONE: [u32; 2] = [node_cache::VISITED, 1];
    const CAPS: [u32; 2] = [node_cache::CAPS_AT, 2 * count::RUNS];
    /// The eye's rolled lattice, its samples' parked count and its owed word (roll.wgsl).
    const ROLL: [u32; 2] = [node_cache::ROLL_K, node_cache::ROLL_WORDS];
    const LATTICE_LEFT: [u32; 2] = [node_cache::LATTICE_LEFT, 1];
    const EYE_OWED: [u32; 2] = [node_cache::OWED_AT + count::EYE, 1];
    &[
        // The eye's samples' stops, their faces' asks (sample_box.eye.wgsl); the light's lay
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
        // The drawable entries' boxes counted into its draw's args, each box's tiles stood on by the
        // world's receivers (record_draw.wgsl, light_stood.wgsl `stand_box`).
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
        // The holes' stops: the eye's faces' asks laid at their own place (record_stops.wgsl,
        // record_stops.eye.wgsl); the light's lay none (record_stops.light.wgsl).
        Touch {
            compute: COMPUTE_RECORD_STOPS,
            binding: 16,
            writes: &[asks(raster::STOP_ASKED_AT)],
            reads: &[asks(raster::STOP_ASKED_AT)],
        },
        Touch {
            compute: COMPUTE_LIGHT_RECORD_STOPS,
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
        // The receivers' marks: the tiles stood on, the world's and the drawn's alike
        // (light_stood.wgsl `stand`).
        Touch {
            compute: COMPUTE_LIGHT_MARK_WALKED,
            binding: 21,
            writes: &[marks(light::LISTED_STOOD)],
            reads: &[marks(light::LISTED_STOOD)],
        },
        Touch {
            compute: COMPUTE_LIGHT_MARK_DRAWS,
            binding: 21,
            writes: &[marks(light::LISTED_STOOD)],
            reads: &[marks(light::LISTED_STOOD)],
        },
        // The propagation: the tiles read listed (light_propagate.wgsl `light_propagate_cs`).
        Touch {
            compute: COMPUTE_LIGHT_PROPAGATE,
            binding: 21,
            writes: &[count(light::LISTED_READ), marks(light::LISTED_READ)],
            reads: &[count(light::LISTED_READ), marks(light::LISTED_READ)],
        },
        // The wanting: each tile read wanted where it is due, its ring counted, and the lay's
        // tiles set to those read (light_propagate.wgsl `light_want_cs`).
        Touch {
            compute: COMPUTE_LIGHT_WANT,
            binding: 21,
            writes: &[[light::ARGS_LAY + 1, 1], count(light::LISTED_WANTED), RINGS],
            reads: &[
                count(light::LISTED_READ),
                count(light::LISTED_WANTED),
                RINGS,
            ],
        },
        // The claim: the claims counted, the claimed, fed and cornered listed, and the passes
        // that cast them opened (light_claim.wgsl, light_enlist.wgsl).
        Touch {
            compute: COMPUTE_LIGHT_CLAIM,
            binding: 21,
            writes: &[
                opened(light::ARGS_CELLS),
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
                opened(light::ARGS_CELLS),
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
        // The node cache's head: the stops' dispatch and count the last walk adds to, and each
        // observer's owed counted (queue_last.wgsl).
        Touch {
            compute: COMPUTE_QUEUE_LAST,
            binding: 9,
            writes: &[
                [node_cache::HITS_ARGS, 4],
                [node_cache::OWED_AT, count::OBSERVERS],
                WORK_DONE,
            ],
            reads: &[
                [node_cache::HITS_ARGS, 4],
                [node_cache::OWED_AT, count::OBSERVERS],
                WORK_DONE,
                CAPS,
            ],
        },
        // The holes' queue's words: the stops' count and dispatch its walks over the node cache
        // raise, and the visits they count against their budget beside the frame's, read off
        // the node cache's head with its run's caps (queue_walk.wgsl); which its asks fold
        // into the frame's, and count their own against the caps (queue_ask.wgsl).
        Touch {
            compute: COMPUTE_QUEUE,
            binding: 10,
            writes: &[QUEUE_HITS, QUEUE_VISITED],
            reads: &[QUEUE_HITS, QUEUE_VISITED],
        },
        Touch {
            compute: COMPUTE_QUEUE_PARKED,
            binding: 10,
            writes: &[QUEUE_HITS, QUEUE_VISITED],
            reads: &[QUEUE_HITS, QUEUE_VISITED],
        },
        Touch {
            compute: COMPUTE_QUEUE,
            binding: 9,
            writes: &[],
            reads: &[VISITS_DONE, CAPS],
        },
        Touch {
            compute: COMPUTE_QUEUE_PARKED,
            binding: 9,
            writes: &[],
            reads: &[VISITS_DONE, CAPS],
        },
        Touch {
            compute: COMPUTE_QUEUE_ASK,
            binding: 10,
            writes: &[QUEUE_VISITED],
            reads: &[QUEUE_VISITED],
        },
        Touch {
            compute: COMPUTE_QUEUE_ASK_PARKED,
            binding: 10,
            writes: &[QUEUE_VISITED],
            reads: &[QUEUE_VISITED],
        },
        Touch {
            compute: COMPUTE_QUEUE_ASK,
            binding: 9,
            writes: &[WORK_DONE],
            reads: &[WORK_DONE, CAPS],
        },
        Touch {
            compute: COMPUTE_QUEUE_ASK_PARKED,
            binding: 9,
            writes: &[WORK_DONE],
            reads: &[WORK_DONE, CAPS],
        },
        // The samples' runs, the eye's and the light's, as the holes' (queue_walk.wgsl,
        // queue_ask.wgsl, queue_last.wgsl over samples.wgsl), but that no walk keeps a stop: the
        // eye's read where its rolled lattice casts and whether its parked samples go on, and
        // its last walk counts those it leaves parked (samples_run.eye.wgsl `sample_owed`).
        Touch {
            compute: COMPUTE_QUEUE_SAMPLES,
            binding: 10,
            writes: &[QUEUE_VISITED],
            reads: &[QUEUE_VISITED],
        },
        Touch {
            compute: COMPUTE_QUEUE_SAMPLES_PARKED,
            binding: 10,
            writes: &[QUEUE_VISITED],
            reads: &[QUEUE_VISITED],
        },
        Touch {
            compute: COMPUTE_QUEUE_LIGHT_SAMPLES,
            binding: 10,
            writes: &[QUEUE_VISITED],
            reads: &[QUEUE_VISITED],
        },
        Touch {
            compute: COMPUTE_QUEUE_LIGHT_SAMPLES_PARKED,
            binding: 10,
            writes: &[QUEUE_VISITED],
            reads: &[QUEUE_VISITED],
        },
        Touch {
            compute: COMPUTE_QUEUE_SAMPLES,
            binding: 9,
            writes: &[],
            reads: &[VISITS_DONE, CAPS, ROLL],
        },
        Touch {
            compute: COMPUTE_QUEUE_SAMPLES_PARKED,
            binding: 9,
            writes: &[],
            reads: &[VISITS_DONE, CAPS, ROLL],
        },
        Touch {
            compute: COMPUTE_QUEUE_LIGHT_SAMPLES,
            binding: 9,
            writes: &[],
            reads: &[VISITS_DONE, CAPS],
        },
        Touch {
            compute: COMPUTE_QUEUE_LIGHT_SAMPLES_PARKED,
            binding: 9,
            writes: &[],
            reads: &[VISITS_DONE, CAPS],
        },
        Touch {
            compute: COMPUTE_QUEUE_ASK_SAMPLES,
            binding: 10,
            writes: &[QUEUE_VISITED],
            reads: &[QUEUE_VISITED],
        },
        Touch {
            compute: COMPUTE_QUEUE_ASK_SAMPLES_PARKED,
            binding: 10,
            writes: &[QUEUE_VISITED],
            reads: &[QUEUE_VISITED],
        },
        Touch {
            compute: COMPUTE_QUEUE_ASK_LIGHT_SAMPLES,
            binding: 10,
            writes: &[QUEUE_VISITED],
            reads: &[QUEUE_VISITED],
        },
        Touch {
            compute: COMPUTE_QUEUE_ASK_LIGHT_SAMPLES_PARKED,
            binding: 10,
            writes: &[QUEUE_VISITED],
            reads: &[QUEUE_VISITED],
        },
        Touch {
            compute: COMPUTE_QUEUE_ASK_SAMPLES,
            binding: 9,
            writes: &[WORK_DONE],
            reads: &[WORK_DONE, CAPS],
        },
        Touch {
            compute: COMPUTE_QUEUE_ASK_SAMPLES_PARKED,
            binding: 9,
            writes: &[WORK_DONE],
            reads: &[WORK_DONE, CAPS],
        },
        Touch {
            compute: COMPUTE_QUEUE_ASK_LIGHT_SAMPLES,
            binding: 9,
            writes: &[WORK_DONE],
            reads: &[WORK_DONE, CAPS],
        },
        Touch {
            compute: COMPUTE_QUEUE_ASK_LIGHT_SAMPLES_PARKED,
            binding: 9,
            writes: &[WORK_DONE],
            reads: &[WORK_DONE, CAPS],
        },
        Touch {
            compute: COMPUTE_QUEUE_LAST_SAMPLES,
            binding: 9,
            writes: &[WORK_DONE, LATTICE_LEFT],
            reads: &[WORK_DONE, CAPS, ROLL, LATTICE_LEFT],
        },
        Touch {
            compute: COMPUTE_QUEUE_LAST_LIGHT_SAMPLES,
            binding: 9,
            writes: &[WORK_DONE],
            reads: &[WORK_DONE, CAPS],
        },
        // The beam rays' runs, the eye's and the light's, as the samples' but that none reads
        // the rolled lattice (queue_walk.wgsl, queue_ask.wgsl, queue_last.wgsl over
        // corners.wgsl).
        Touch {
            compute: COMPUTE_QUEUE_CORNERS,
            binding: 10,
            writes: &[QUEUE_VISITED],
            reads: &[QUEUE_VISITED],
        },
        Touch {
            compute: COMPUTE_QUEUE_CORNERS_PARKED,
            binding: 10,
            writes: &[QUEUE_VISITED],
            reads: &[QUEUE_VISITED],
        },
        Touch {
            compute: COMPUTE_QUEUE_LIGHT_CORNERS,
            binding: 10,
            writes: &[QUEUE_VISITED],
            reads: &[QUEUE_VISITED],
        },
        Touch {
            compute: COMPUTE_QUEUE_LIGHT_CORNERS_PARKED,
            binding: 10,
            writes: &[QUEUE_VISITED],
            reads: &[QUEUE_VISITED],
        },
        Touch {
            compute: COMPUTE_QUEUE_ASK_CORNERS,
            binding: 10,
            writes: &[QUEUE_VISITED],
            reads: &[QUEUE_VISITED],
        },
        Touch {
            compute: COMPUTE_QUEUE_ASK_CORNERS_PARKED,
            binding: 10,
            writes: &[QUEUE_VISITED],
            reads: &[QUEUE_VISITED],
        },
        Touch {
            compute: COMPUTE_QUEUE_ASK_LIGHT_CORNERS,
            binding: 10,
            writes: &[QUEUE_VISITED],
            reads: &[QUEUE_VISITED],
        },
        Touch {
            compute: COMPUTE_QUEUE_ASK_LIGHT_CORNERS_PARKED,
            binding: 10,
            writes: &[QUEUE_VISITED],
            reads: &[QUEUE_VISITED],
        },
        Touch {
            compute: COMPUTE_QUEUE_CORNERS,
            binding: 9,
            writes: &[],
            reads: &[VISITS_DONE, CAPS],
        },
        Touch {
            compute: COMPUTE_QUEUE_CORNERS_PARKED,
            binding: 9,
            writes: &[],
            reads: &[VISITS_DONE, CAPS],
        },
        Touch {
            compute: COMPUTE_QUEUE_LIGHT_CORNERS,
            binding: 9,
            writes: &[],
            reads: &[VISITS_DONE, CAPS],
        },
        Touch {
            compute: COMPUTE_QUEUE_LIGHT_CORNERS_PARKED,
            binding: 9,
            writes: &[],
            reads: &[VISITS_DONE, CAPS],
        },
        Touch {
            compute: COMPUTE_QUEUE_ASK_CORNERS,
            binding: 9,
            writes: &[WORK_DONE],
            reads: &[WORK_DONE, CAPS],
        },
        Touch {
            compute: COMPUTE_QUEUE_ASK_CORNERS_PARKED,
            binding: 9,
            writes: &[WORK_DONE],
            reads: &[WORK_DONE, CAPS],
        },
        Touch {
            compute: COMPUTE_QUEUE_ASK_LIGHT_CORNERS,
            binding: 9,
            writes: &[WORK_DONE],
            reads: &[WORK_DONE, CAPS],
        },
        Touch {
            compute: COMPUTE_QUEUE_ASK_LIGHT_CORNERS_PARKED,
            binding: 9,
            writes: &[WORK_DONE],
            reads: &[WORK_DONE, CAPS],
        },
        Touch {
            compute: COMPUTE_QUEUE_LAST_CORNERS,
            binding: 9,
            writes: &[WORK_DONE],
            reads: &[WORK_DONE, CAPS],
        },
        Touch {
            compute: COMPUTE_QUEUE_LAST_LIGHT_CORNERS,
            binding: 9,
            writes: &[WORK_DONE],
            reads: &[WORK_DONE, CAPS],
        },
        // The eye's close: the offset counted or its samples held to go on, and the eye's
        // lattice owed in its owed word (roll.wgsl).
        Touch {
            compute: COMPUTE_ROLL,
            binding: 9,
            writes: &[ROLL, EYE_OWED],
            reads: &[ROLL, LATTICE_LEFT, EYE_OWED],
        },
        Touch {
            compute: COMPUTE_ROLL_OWED,
            binding: 9,
            writes: &[EYE_OWED],
            reads: &[ROLL, EYE_OWED],
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

/// Whether step `s` writes buffer `buf` through atomics alone: a dispatch whose binding of it
/// its shader holds atomic. A copy's or the host's write is plain.
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
    while k < f.kept.len() {
        if within(r, &[f.kept[k].range]) {
            return Some(k);
        }
        k += 1;
    }
    None
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
    while k < f.kept.len() {
        let kept = &f.kept[k];
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
    // A render pass is one usage scope whole: a pipeline it draws that lays out writable
    // storage may have a group set that binds the count's buffer so.
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

/// Rule 2: the step after `a` in its stage, and the buffer, where both write one word, but a
/// count both add to through atomics, or `a` its seed, a copy ahead of the appenders after it
/// (`Frame::appends`); a plain write after an appender, or beside another plain one, is
/// refused.
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

/// Rule 3: where a word step `r` reads is written by no step before it, and kept over frames
/// by none (`Frame::kept`), the buffer; or, a word
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
                    && !rewrites_kept(f, w, read)
                    && !within_bound(read, f.bounds)
                    && s.kind != WRITE_BY_HOST
                    && f.steps[w].kind != WRITE_BY_HOST
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

/// Rule 4: where step `w` writes a kept range not its owner's, the kept range's place; or, `w`
/// its owner, where it writes it not at all.
const fn not_owner(f: &Frame, t: &Table, w: usize) -> Option<usize> {
    let s = &f.steps[w];
    let mut k = 0;
    while k < f.kept.len() {
        let kept = f.kept[k];
        let writes = writes_into(t, w, kept.range);
        let applies = applies(&kept, s.name);
        let owner = same(s.name, kept.owner) || applies;
        let atomic = within(kept.range, f.appends) && writes_atomic(s, kept.range.buf);
        if (writes && !owner && s.kind != WRITE_BY_HOST && !owned_with(f, w, kept.owner))
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
const fn applies(kept: &Kept, name: &str) -> bool {
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

/// Whether step `w` is the owner of the kept range `read` lies within, which writes it for the
/// frames after the one that read it.
const fn rewrites_kept(f: &Frame, w: usize, read: Range) -> bool {
    match kept_of(f, read) {
        Some(k) => {
            same(f.steps[w].name, f.kept[k].owner)
                || owned_with(f, w, f.kept[k].owner)
                || applies(&f.kept[k], f.steps[w].name)
        }
        None => false,
    }
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
        let mut each = [none; 4];
        if let Some(kept) = not_owner(f, &t, k) {
            each[3] = Found {
                rule: 4,
                step: k,
                other: kept,
                buf: f.kept[kept].range.buf,
            };
        }
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
    if found.rule == 4 {
        f.kept[found.other].owner
    } else if found.rule == 2 || (found.rule == 3 && found.other != usize::MAX) {
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
