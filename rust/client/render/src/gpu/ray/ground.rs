// pxl-render/gpu/ray/ground.rs

//! The windows' bricks on the GPU: each handed brick filled in its turn
//! (`height_cs`, `way_cs`, `bound_cs`, the ways' tree) and its cells and
//! its tree's records read back where the CPU looks them up.

use pxl_spec_core::way;
use pxl_spec_core::window::{
    self, BACK_WORDS, BRICK, BRICK_CELLS, BRICK_WORDS, COLUMNS_AT, ELE_AT, FRESH_BRICK,
    FRESH_WORDS, HANDED, HANDED_WORDS, LEVELS, LINES_AT, LOOKS_AT, SLOTS,
};
use xpute_guest::mem::off_of;

use super::RaysPass;
use crate::frame::shader::record::{
    compute, dynamic, SHADER_BOUND, SHADER_HEIGHT, SHADER_WAY, SHADER_WAY_FREE, SHADER_WAY_FREED,
    SHADER_WAY_TREE,
};
use crate::gpu::api::{
    buffer_entry, buffer_range_entry, buffer_usage, bytes, release, room, word_bytes,
    write_buffer_bytes, Gpu, Stamp, DONE, UNIFORM_ALIGN_MAX,
};
use crate::gpu::pipeline::{Built, Pipelines};
use crate::gpu::renderer::{CellsBack, GroundBricks, Landing};

/// The buffers bricks' cells come back through: what a frame hands, a
/// mapping still out a frame more (pxl-spec-core `window::HANDED`).
const READS: usize = crate::gpu::renderer::READS as usize;
/// A buffer of them carrying no brick.
const FREE: u32 = u32::MAX;

/// The terrain's frames on the GPU and the passes that draw their cells.
pub(super) struct GroundBuffers {
    /// Per slot of the windows' bricks (`window::SLOTS`), its columns,
    /// levels, cells as laid and root, which `height_cs`, `bound_cs` and
    /// `way_tree_cs` write in the turn it is handed (pxl-spec-core
    /// `window::BRICK_WORDS`).
    pub(super) ground: u32,
    /// Per slot, its cells' four looks (`LOOKS_BYTES`).
    pub(super) looks: u32,
    /// The bricks the turn handed, `window::HANDED` at most, a record each
    /// (`window::HANDED_WORDS`): its words, then its looks, its tile's
    /// samples and its lines, which only its fill reads; `height_cs`'s,
    /// `bound_cs`'s and `way_cs`'s one per `id.z`.
    pub(super) fresh: u32,
    pub(super) fresh_len: u32,
    /// Per slot (`window::SLOTS`), its cells' way back: the fill
    /// the next landing must be of (its ticket 0 where none is asked).
    asked: [CellsBack; SLOTS as usize],
    /// The bricks the frame handed, their words (`window::fresh`), in the
    /// order handed, each with a read of its own (`hand` hands none past the
    /// reads free): a fill's records live in its frame's scratch, so its
    /// read is taken in its frame.
    pub(super) handed: [[u32; FRESH_WORDS as usize]; HANDED as usize],
    /// The buffers cells are copied into and mapped back through, as many as
    /// a frame hands bricks a mapping still out a frame more (`READS`);
    /// each one's brick and the fill it carries (`FREE` where none), and
    /// its mapping's own word (`read_back`), which the host writes: held
    /// where the guest's allocator puts it, inside the region the host
    /// addresses, as an arriving image's word is. A static is outside it on
    /// a device.
    reads: [u32; READS],
    held: [(u32, u32); READS],
    mapping: Box<[u32; READS]>,
    /// Each read's records, mapped beside its brick's words into its room
    /// on the heap, as many bytes as the room holds, the buffer of its size
    /// (none before its first read), and its mapping's word: a read lands
    /// with both. Every room is made `record_bytes` as its read is next
    /// taken, the heap granting it; else it keeps the room it has.
    record_reads: [u32; READS],
    record_rooms: [Box<[u32]>; READS],
    record_mapping: Box<[u32; READS]>,
    record_bytes: u32,
    /// What each read's brick was built in, indexed by `Short`: the pool's
    /// families and the families a brick's scratch held (`ran_out`).
    built_in: [[u32; 2]; READS],
    /// Whether a build ran out of the pool, or of the scratch, since it last
    /// grew, indexed by `Short` (`grow`, `grow_scratch`).
    ran_out: [bool; 2],
    /// The ways' pool, which the cast reads, its families and its free stack;
    /// and the handed bricks' scratch, the families each brick's holds a
    /// tree of (`way::tree::scratch_words`), whose records each read copies
    /// in the frame they are built: nothing in it outlives its frame.
    pub(super) pool: u32,
    families: u32,
    stack: u32,
    scratch: u32,
    scratch_families: u32,
    /// Each slot's number, `bound_cs`'s level, at a dynamic offset of its
    /// group.
    slots: u32,
    heights_built: Built,
    heights_group: u32,
    bound_built: Built,
    /// `bound_cs`'s group, its level at a dynamic offset (`SLOT_STRIDE`).
    bound_group: u32,
    /// `way_cs`'s, and its group: the fresh frames, the frames it reads
    /// the columns of, and the scratch it keeps the spans in.
    ways_built: Built,
    ways_group: u32,
    /// A re-filled brick's old ways' tree given back, its new one built,
    /// the families the build gave back pushed (pxl-spec-core `way::tree`):
    /// each pass and its group, over the ways' pool, heap words then data
    /// words (`pool_bytes`), its free stack, the count taken then the ids,
    /// family 0 never among them, a word a family, and each handed brick's
    /// scratch past its build (`way::tree::FREED_AT`).
    free_built: Built,
    free_group: u32,
    tree_built: Built,
    tree_group: u32,
    freed_built: Built,
    freed_group: u32,
}

/// The ways' pool of `families` on the GPU, in bytes: a slot two words,
/// eight a family. Its free stack is a word a family: the count taken, then
/// every family's id but family 0's, every family free but family 0 as WebGPU
/// makes it, zeroed (pxl-shader `TAKEN`, `id`), so nothing is written to
/// start it, nor to grow it.
const fn pool_bytes(families: u32) -> u32 {
    families * way::tree::FAMILY_WORDS * 4
}
const _: () = assert!(
    crate::frame::shader::floor::fits(pool_bytes(way::tree::POOL_FAMILIES) as u64, true),
    "the ways' pool at its most binds past WebGPU's floor (frame/shader/floor.rs `fits`)"
);

/// The ways' pool of `families` and its free stack, zeroed: each copied
/// from as it grows, and into.
fn pool_buffers(g: &Gpu, families: u32) -> (u32, u32) {
    let usage = buffer_usage::STORAGE | buffer_usage::COPY_SRC | buffer_usage::COPY_DST;
    (
        g.buffer(pool_bytes(families), usage),
        g.buffer(word_bytes(families as usize), usage),
    )
}

/// The handed bricks' scratch, each brick's for a tree of `families`, in
/// bytes: `HANDED` of them, every brick's alike, so each finds its own off
/// the buffer's length (pxl-spec-core fill.rs `scratch_at`).
const fn scratch_bytes(families: u32) -> u32 {
    HANDED * way::tree::scratch_words(families) * 4
}
const _: () = assert!(
    crate::frame::shader::floor::fits(scratch_bytes(way::tree::BRICK_FAMILIES) as u64, true),
    "the scratch at its most binds past WebGPU's floor (frame/shader/floor.rs `fits`)"
);

/// The handed bricks' scratch, each brick's for a tree of `families`,
/// which its records are copied out of.
fn scratch_buffer(g: &Gpu, families: u32) -> u32 {
    g.buffer(
        scratch_bytes(families),
        buffer_usage::STORAGE | buffer_usage::COPY_SRC,
    )
}

/// What a build may run out of, each grown as builds do: the ways' pool
/// (`grow`) and the handed bricks' scratch (`grow_scratch`), an index of
/// `GroundBuffers::built_in` and `ran_out`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Short {
    Pool,
    Scratch,
}

/// What a build whose records' count is `n` ran out of (pxl-shader
/// `way_tree_cs`): `STARVED` the pool, `CRAMPED` the scratch; neither
/// for a tree built, none counted where the brick has no ways, nor for one
/// past the brick's own most (`NONE`), which no growth lets it build.
fn short_of(n: u32) -> Option<Short> {
    match n {
        way::tree::STARVED => Some(Short::Pool),
        way::tree::CRAMPED => Some(Short::Scratch),
        _ => None,
    }
}

/// What a build that ran out of the pool or the scratch does, built in one
/// of `built` families, the one it ran out of now of `families`, at most `most`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RanOut {
    /// It grew since: built again, as the larger may hold it.
    Again,
    /// The one it ran out of, which grows (`grow`, `grow_scratch`), and
    /// built again.
    Grow,
    /// It ran out at its most: refused.
    Refused,
}

/// A read is out a frame more than its build (`READS`), so a build that
/// ran out of the pool or the scratch may land after it grew for another:
/// judged against what it was built in, it grows it no further and is
/// refused only by it at its most. The pool's and the scratch's alike,
/// each against its own (`Short`).
fn ran_out(built: u32, families: u32, most: u32) -> RanOut {
    if built < families {
        RanOut::Again
    } else if families < most {
        RanOut::Grow
    } else {
        RanOut::Refused
    }
}
impl GroundBuffers {
    /// The ways' passes' groups over the pool, its stack and the scratch as
    /// they are, and `bound_cs`'s and `way_cs`'s over the scratch.
    fn ways_groups(&mut self, g: &Gpu) {
        for group in [
            self.bound_group,
            self.ways_group,
            self.free_group,
            self.tree_group,
            self.freed_group,
        ] {
            release(group);
        }
        let (fresh, ground, pool, stack, scratch) =
            (self.fresh, self.ground, self.pool, self.stack, self.scratch);
        self.bound_group = g.bind_group(
            self.bound_built.layouts[0],
            &[
                buffer_range_entry(0, self.slots, 0, 16),
                buffer_entry(1, fresh),
                buffer_entry(2, ground),
                buffer_entry(3, scratch),
            ],
        );
        self.ways_group = g.bind_group(
            self.ways_built.layouts[0],
            &[
                buffer_entry(0, fresh),
                buffer_entry(1, ground),
                buffer_entry(2, scratch),
            ],
        );
        self.free_group = g.bind_group(
            self.free_built.layouts[0],
            &[
                buffer_entry(0, fresh),
                buffer_entry(1, ground),
                buffer_entry(2, pool),
                buffer_entry(3, stack),
            ],
        );
        self.tree_group = g.bind_group(
            self.tree_built.layouts[0],
            &[
                buffer_entry(0, fresh),
                buffer_entry(1, ground),
                buffer_entry(2, pool),
                buffer_entry(3, stack),
                buffer_entry(4, scratch),
            ],
        );
        self.freed_group = g.bind_group(
            self.freed_built.layouts[0],
            &[buffer_entry(0, scratch), buffer_entry(1, stack)],
        );
    }

    /// The ways' pool twice as large, never past `way::tree::POOL_FAMILIES`,
    /// the bake's bound, at which no brick of a baked set is refused, once a
    /// build ran out of it (`read_back`): its heap words and its stack
    /// copied where they were, its data words to the new data's first, so
    /// every id a tree or a read in flight names holds what it held, and the
    /// stack's slots past the old hold the new families free (pxl-shader
    /// `TAKEN`). Ahead of the turn's bricks, so their builds and the cast
    /// read the new one; the cast's groups follow its id (`prepare`).
    ///
    /// A known gap: a device out of memory may fail to make the doubled
    /// buffer, and nothing hears it, the host keeping no error scope, so the
    /// renderer reads on with a buffer the device holds as an error. The
    /// last doubling holds the old pool beside the new, 32 MiB with 64,
    /// until the copy is submitted; the scratch's growth has the same gap.
    fn grow(&mut self, g: &Gpu) {
        let families = (2 * self.families).min(way::tree::POOL_FAMILIES);
        if families <= self.families {
            return;
        }
        let (pool, stack) = pool_buffers(g, families);
        let half = |families: u32| pool_bytes(families) / 2;
        let e = g.encoder();
        g.copy_buffer(e, (self.pool, 0), (pool, 0), half(self.families));
        g.copy_buffer(
            e,
            (self.pool, half(self.families)),
            (pool, half(families)),
            half(self.families),
        );
        g.copy_buffer(
            e,
            (self.stack, 0),
            (stack, 0),
            word_bytes(self.families as usize),
        );
        g.submit(e);
        release(self.pool);
        release(self.stack);
        (self.pool, self.stack, self.families) = (pool, stack, families);
        self.ways_groups(g);
    }

    /// Each brick's tree room in the scratch twice as many families, never
    /// past `way::tree::BRICK_FAMILIES`, the bake's bound on a brick's, once
    /// a build outgrew it (`read_back`); its spans stay as
    /// they are (`window::TREE_AT`), so the buffer grows by less than twice.
    /// Nothing is copied: a brick's records are copied into its read in the
    /// frame it is filled in, before any turn grows it. Ahead of the turn's
    /// bricks, so their passes lay theirs out by the new length.
    fn grow_scratch(&mut self, g: &Gpu) {
        let families = (2 * self.scratch_families).min(way::tree::BRICK_FAMILIES);
        if families <= self.scratch_families {
            return;
        }
        let scratch = scratch_buffer(g, families);
        release(self.scratch);
        (self.scratch, self.scratch_families) = (scratch, families);
        self.ways_groups(g);
    }

    /// Read `k`'s records' room, free, made `record_bytes` where it is
    /// another, its buffer with it, the old room given back first, so no
    /// two of a read's are held at once; where the heap refuses it, a room
    /// of the count alone, whose read lands short and is filled again, as a
    /// brick waits on the heap. A read's first room is made here.
    fn room(&mut self, g: &Gpu, k: usize) {
        let words = (self.record_bytes / 4) as usize;
        if self.record_rooms[k].len() == words {
            return;
        }
        self.record_rooms[k] = Box::default();
        let mut room = Vec::new();
        if room.try_reserve_exact(words).is_err() {
            room = vec![0];
        } else {
            room.resize(words, 0);
        }
        release(self.record_reads[k]);
        self.record_reads[k] = g.buffer(
            word_bytes(room.len()),
            buffer_usage::MAP_READ | buffer_usage::COPY_DST,
        );
        self.record_rooms[k] = room.into_boxed_slice();
    }
}

/// A brick on the GPU, in bytes.
const BRICK_BYTES: u32 = BRICK_WORDS * 4;

/// The windows' bricks on the GPU, in bytes: one storage buffer, bound
/// whole by the cast's, the levels' and the ways' groups.
const GROUND_BYTES: u64 = SLOTS as u64 * BRICK_BYTES as u64;
const _: () = assert!(
    crate::frame::shader::floor::fits(GROUND_BYTES, true),
    "GROUND_BYTES: the windows' bricks bind past WebGPU's floor (frame/shader/floor.rs `fits`)"
);

/// The windows' bricks' cells' four looks, a word a cell in the slots'
/// order: written by the fill off each handed brick (`height_cs`), read
/// by the cast at a hit (pxl-shader `World`'s `Looks`). Their own
/// buffer, as the bricks' binding has no room for them.
const LOOKS_BYTES: u64 = SLOTS as u64 * BRICK_CELLS as u64 * 4;
const _: () = assert!(
    crate::frame::shader::floor::fits(LOOKS_BYTES, true),
    "LOOKS_BYTES: the bricks' looks bind past WebGPU's floor (frame/shader/floor.rs `fits`)"
);

/// `bound_cs`'s group: its level at a dynamic offset (`SLOT_STRIDE`), the
/// fresh frames, the frames it writes the levels of, and the handed
/// bricks' scratch it reads the spans in. The cast's and `joined_cs`'s one
/// group is the cast's, which binds all of theirs.
const BOUND_GROUP: [u32; compute::BOUND_GROUP.len()] = dynamic(compute::BOUND_GROUP, 0);
/// A number's slot in the fill's numbers, read at a dynamic offset: every
/// device's dynamic offset alignment.
const SLOT_STRIDE: u32 = UNIFORM_ALIGN_MAX;

impl RaysPass {
    /// Each held tile the GPU does not yet hold handed to its frame, copied
    /// into the turn's stream as it has room, the packets they live in let
    /// go once handed; whether or not an eye casts, as a tile's cells are
    /// no eye's.
    pub fn hand(&mut self, g: &Gpu, pipelines: &mut Pipelines, ground: &mut GroundBricks) {
        let families = ground.pool.min(way::tree::POOL_FAMILIES);
        let scratch_families = ground.scratch.clamp(1, way::tree::BRICK_FAMILIES);
        let gb = self.ground.get_or_insert_with(|| {
            let buffer = |size: u32, usage: u32| g.buffer(size, usage);
            let storage = buffer_usage::STORAGE | buffer_usage::COPY_DST;
            let frames_buffer = buffer(GROUND_BYTES as u32, storage | buffer_usage::COPY_SRC);
            let fresh = buffer(word_bytes((HANDED_WORDS * HANDED) as usize), storage);
            let looks = buffer(LOOKS_BYTES as u32, storage);
            let heights_built = pipelines.build_compute(g, SHADER_HEIGHT, &compute::HEIGHT_GROUP);
            let heights_group = g.bind_group(
                heights_built.layouts[0],
                &[
                    buffer_entry(0, fresh),
                    buffer_entry(1, frames_buffer),
                    buffer_entry(2, looks),
                ],
            );
            // Each slot's number in it, written once: `bound_cs`'s level.
            let slots = buffer(
                (LEVELS + 1) * SLOT_STRIDE,
                buffer_usage::UNIFORM | buffer_usage::COPY_DST,
            );
            for k in 0..=LEVELS {
                g.write_buffer(slots, k * SLOT_STRIDE, bytes(&[k, 0, 0, 0]));
            }
            let scratch = scratch_buffer(g, scratch_families);
            let (bound_built, ways_built) = (
                pipelines.build_compute(g, SHADER_BOUND, &BOUND_GROUP),
                pipelines.build_compute(g, SHADER_WAY, &compute::WAY_GROUP),
            );
            let (pool, stack) = pool_buffers(g, families);
            let free_built = pipelines.build_compute(g, SHADER_WAY_FREE, &compute::WAY_FREE_GROUP);
            let tree_built = pipelines.build_compute(g, SHADER_WAY_TREE, &compute::WAY_TREE_GROUP);
            let freed_built =
                pipelines.build_compute(g, SHADER_WAY_FREED, &compute::WAY_FREED_GROUP);
            let mut gb = GroundBuffers {
                ground: frames_buffer,
                looks,
                fresh,
                fresh_len: 0,
                asked: [CellsBack::default(); SLOTS as usize],
                handed: [[0; FRESH_WORDS as usize]; HANDED as usize],
                reads: core::array::from_fn(|_| {
                    buffer(
                        BACK_WORDS * 4,
                        buffer_usage::MAP_READ | buffer_usage::COPY_DST,
                    )
                }),
                held: [(FREE, 0); READS],
                mapping: Box::new([0; READS]),
                record_reads: [0; READS],
                record_rooms: core::array::from_fn(|_| Box::default()),
                record_mapping: Box::new([0; READS]),
                built_in: [[0; 2]; READS],
                record_bytes: (ground.records.next_multiple_of(4)).clamp(
                    word_bytes(1 + way::tree::RECORD_WORDS as usize),
                    way::tree::RECORDS_BYTES,
                ),
                pool,
                families,
                stack,
                ran_out: [false; 2],
                scratch,
                scratch_families,
                slots,
                heights_built,
                heights_group,
                bound_built,
                bound_group: 0,
                ways_built,
                ways_group: 0,
                free_built,
                free_group: 0,
                tree_built,
                tree_group: 0,
                freed_built,
                freed_group: 0,
            };
            gb.ways_groups(g);
            gb
        });
        if core::mem::take(&mut gb.ran_out[Short::Pool as usize]) {
            gb.grow(g);
        }
        if core::mem::take(&mut gb.ran_out[Short::Scratch as usize]) {
            gb.grow_scratch(g);
        }
        // A brick handed takes a read its frame: none past those free.
        let free = gb
            .held
            .iter()
            .filter(|h| h.0 == FREE)
            .count()
            .min(HANDED as usize) as u32;
        let mut fresh = 0;
        (ground.upload)(&mut |q| {
            if fresh == free
                || !room(
                    write_buffer_bytes(q.looks)
                        + write_buffer_bytes(bytes(q.samples))
                        + write_buffer_bytes(bytes(q.lines))
                        + write_buffer_bytes(bytes(&q.fresh)),
                )
            {
                return false;
            }
            let (brick, at) = (
                q.fresh[window::FRESH_BRICK as usize],
                window::handed_at(fresh) * 4,
            );
            g.write_buffer(gb.fresh, at, bytes(&q.fresh));
            g.write_buffer(gb.fresh, at + LOOKS_AT * 4, q.looks);
            g.write_buffer(gb.fresh, at + ELE_AT * 4, bytes(q.samples));
            g.write_buffer(gb.fresh, at + LINES_AT * 4, bytes(q.lines));
            gb.asked[brick as usize] = q.back;
            gb.handed[fresh as usize] = q.fresh;
            fresh += 1;
            true
        });
        gb.fresh_len = fresh;
    }

    /// The bricks this turn handed filled: their tops from their samples,
    /// their ways' spans, their levels a dispatch, level 0 a column's own and
    /// each over the one before, and their ways' trees. Ahead of the frame,
    /// as they outlive it, so no brick the window names is cast before its
    /// cells are written, even when the frame proper is dropped. Whether it
    /// recorded any.
    pub fn grids(&mut self, g: &Gpu, encoder: u32, stamp: &mut Stamp) -> bool {
        let Some(c) = self.ground.as_mut().filter(|c| c.fresh_len != 0) else {
            return false;
        };
        let pass = g.compute_pass(encoder, stamp.pass());
        // A re-filled brick's old ways' families given back, off its root word
        // before its build writes another (pxl-spec-core `way::tree`).
        g.set_pipeline(pass, c.free_built.pipeline);
        g.set_bind_group(pass, 0, c.free_group, &[]);
        g.dispatch(pass, 1, 1, c.fresh_len);
        g.set_pipeline(pass, c.heights_built.pipeline);
        g.set_bind_group(pass, 0, c.heights_group, &[]);
        let w = compute::HEIGHT_WORKGROUP;
        g.dispatch(
            pass,
            BRICK.div_ceil(w[0]),
            BRICK.div_ceil(w[1]),
            c.fresh_len,
        );
        // A column a thread, over the tops just drawn: each column's spans
        // once, the brick's and its ring's (pxl-spec-core fill.rs `spans`),
        // which the levels and the trees read.
        g.set_pipeline(pass, c.ways_built.pipeline);
        g.set_bind_group(pass, 0, c.ways_group, &[]);
        g.dispatch(
            pass,
            window::SPANS_COLUMNS.div_ceil(compute::WAY_WORKGROUP[0]),
            1,
            c.fresh_len,
        );
        g.set_pipeline(pass, c.bound_built.pipeline);
        for j in 0..=LEVELS {
            g.set_bind_group(pass, 0, c.bound_group, &[j * SLOT_STRIDE]);
            let side = (BRICK >> j).div_ceil(compute::BOUND_WORKGROUP[0]);
            g.dispatch(pass, side, side, c.fresh_len);
        }
        // Each brick's ways as cells, a workgroup a brick, over its spans
        // and the levels just folded; then what its build gave back pushed,
        // a dispatch after, so no push lands under another brick's ids
        // while it reads them.
        g.set_pipeline(pass, c.tree_built.pipeline);
        g.set_bind_group(pass, 0, c.tree_group, &[]);
        g.dispatch(pass, 1, 1, c.fresh_len);
        g.set_pipeline(pass, c.freed_built.pipeline);
        g.set_bind_group(pass, 0, c.freed_group, &[]);
        g.dispatch(pass, 1, 1, c.fresh_len);
        g.end(pass);
        true
    }

    /// The bricks the grids wrote mapped back where the CPU looks them up,
    /// once the encoder that wrote them is submitted: a brick's columns,
    /// levels, cells as laid and root lie together (`window::BACK_WORDS`), so one mapping
    /// brings them all.
    ///
    /// A brick asked takes a free buffer of `READS`, which maps once at a
    /// time and lands whatever fill it carried. One that landed for the
    /// brick's latest fill is told `landed`. One that landed for an earlier
    /// fill is discarded: its bytes went into the brick's cells while the
    /// kernel holds the brick on its way (or absent, if it moved), which
    /// nothing reads, and the latest fill's landing writes them again before
    /// it is told. A brick with no buffer free waits, Sent, for a later
    /// turn. A mapping still on its way is looked at only when a turn runs:
    /// nothing asks a turn for it.
    ///
    /// On a device, a mapping that completes after the last asked turn is
    /// seen at the next turn: the host learns of it only by polling the
    /// device, and a thread waiting there (wgpu `poll(Wait)`) holds the lock a
    /// surface's present takes (wgpu-core's `Device::poll` reads its
    /// `snatchable_lock` across the fence wait, `Surface::present` writes
    /// it), so none waits.
    pub fn read_back(&mut self, g: &Gpu, ground: &mut GroundBricks) {
        let landed = &mut *ground.landed;
        let most = [way::tree::POOL_FAMILIES, way::tree::BRICK_FAMILIES];
        let Some(c) = self.ground.as_mut() else {
            return;
        };
        // A read lands with its brick's words and its records both: told
        // where it is of the brick's latest fill, discarded where a later
        // fill asked again, whose own read brings it. A build that ran out
        // of the pool or the scratch it was built in keeps no record, its
        // count saying which (`short_of`), which grows ahead of the next
        // turn's bricks where it is as built still and may (`ran_out`,
        // `grow`, `grow_scratch`), the brick filled again; one past its
        // own most is refused at once; one whose records outgrew its read is
        // filled again, every read's room grown past them.
        for k in 0..READS {
            let (s, ticket) = c.held[k];
            if s == FREE || c.mapping[k] & DONE == 0 || c.record_mapping[k] & DONE == 0 {
                continue;
            }
            c.held[k] = (FREE, 0);
            if c.asked[s as usize].ticket != ticket {
                continue;
            }
            c.asked[s as usize].ticket = 0;
            let room = &c.record_rooms[k];
            let n = room.first().copied().unwrap_or(0);
            let words = 1 + n.min(way::tree::BRICK_FAMILIES) * way::tree::RECORD_WORDS;
            let landing = if n == way::tree::NONE {
                Landing::Refused
            } else if let Some(short) = short_of(n) {
                let (i, now) = (short as usize, [c.families, c.scratch_families]);
                match ran_out(c.built_in[k][i], now[i], most[i]) {
                    RanOut::Again => Landing::Again,
                    RanOut::Grow => {
                        c.ran_out[i] = true;
                        Landing::Again
                    }
                    RanOut::Refused => Landing::Refused,
                }
            } else if words as usize > room.len() {
                c.record_bytes = c.record_bytes.max(
                    word_bytes(words as usize)
                        .next_power_of_two()
                        .min(way::tree::RECORDS_BYTES),
                );
                Landing::Again
            } else {
                Landing::Built(&room[..words as usize])
            };
            landed(s, ticket, landing);
        }
        // Each brick the frame handed, its words and its build's records
        // copied in its frame, into a read `hand` kept free for it: as much
        // of its records as the read's room holds, and its scratch's do.
        let fresh = core::mem::take(&mut c.fresh_len);
        if fresh == 0 {
            return;
        }
        let e = g.encoder();
        let mut taken = [false; READS];
        let mut free = 0;
        for h in 0..fresh {
            let s = c.handed[h as usize][FRESH_BRICK as usize];
            let Some(k) = (free..READS).find(|&k| c.held[k].0 == FREE) else {
                break;
            };
            free = k + 1;
            c.room(g, k);
            g.copy_buffer(
                e,
                (c.ground, s * BRICK_BYTES + COLUMNS_AT * 4),
                (c.reads[k], 0),
                BACK_WORDS * 4,
            );
            let sb = c.scratch_families;
            let records = (h * way::tree::scratch_words(sb) + way::tree::records_at(sb)) as usize;
            let words = c.record_rooms[k]
                .len()
                .min((1 + sb * way::tree::RECORD_WORDS) as usize);
            g.copy_buffer(
                e,
                (c.scratch, word_bytes(records)),
                (c.record_reads[k], 0),
                word_bytes(words),
            );
            (c.held[k], c.built_in[k], taken[k]) =
                ((s, c.asked[s as usize].ticket), [c.families, sb], true);
        }
        g.submit(e);
        for k in (0..READS).filter(|&k| taken[k]) {
            let (s, ticket) = c.held[k];
            g.map_read(
                c.reads[k],
                0,
                BACK_WORDS * 4,
                c.asked[s as usize].cells,
                &mut c.mapping[k],
                ticket,
            );
            let room = &mut c.record_rooms[k];
            g.map_read(
                c.record_reads[k],
                0,
                word_bytes(room.len()),
                off_of(room.as_ptr()) as u32,
                &mut c.record_mapping[k],
                ticket,
            );
        }
    }
}

#[cfg(test)]
#[path = "ground.test.rs"]
mod test;
