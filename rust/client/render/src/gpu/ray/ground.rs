// pxl-render/gpu/ray/ground.rs

//! The clip levels' bricks on the GPU: each handed brick filled in its turn
//! (`height_cs`, `way_cs`, `bound_cs`, the ways' tree) and its cells and
//! its tree's records read back where the CPU looks them up.

use pxl_engine::clipmap::{
    self, BACK_WORDS, BRICK, BRICK_CELLS, BRICK_WORDS, COLUMNS_AT, ELE_AT, FILL_BRICK, FILL_CAP,
    FILL_WORDS, HANDED_WORDS, LEVELS, LINES_AT, PALETTE_AT, SLOTS,
};
use pxl_engine::{fill, way};
use xpute_guest::mem::off_of;
use xpute_kit::status::bug::OrBug;
use xpute_kit::status::errno::Errno;

use super::RaysPass;
use crate::frame::shader::floor;
use crate::frame::shader::record::{
    dynamic, COMPUTE, SHADER_BOUND, SHADER_HEIGHT, SHADER_WAY, SHADER_WAY_FREE, SHADER_WAY_RETURN,
    SHADER_WAY_TREE, STORAGE, STORAGE_RW, UNIFORM,
};
use crate::frame::shader::word::{HEIGHT_SIDE, LEVEL_SIDE};
use crate::gpu::api::{
    buf_entry, buf_range_entry, buffer_usage, bytes, release, room, word_bytes, write_buffer_bytes,
    Gpu, RenderContext, DONE, UNIFORM_ALIGN_MAX,
};
use crate::gpu::pipeline::{Pipeline, Pipelines};
use crate::gpu::span_spec::GpuSpanName;

/// The ground's clip levels as the GPU holds them (pxl-engine
/// `clipmap`, their bricks in `SLOTS` slots): the bricks to hand it, and where
/// their cells come back.
pub struct GroundBricks<'a> {
    /// Hands each brick whose cells the GPU is to draw to the writer.
    pub upload: &'a mut TerrainUpload,
    /// Told each brick whose cells came back, and the fill they are of
    /// (`CellsBack::ticket`).
    pub on_resident: &'a mut OnResident<'a>,
    /// The ways' pool's child groups at first (pxl-kernel domain/scene/param.rs
    /// `WAYS_POOL_FIRST_BYTES`): it doubles each time a build runs out, up
    /// to `way::tree::POOL_CHILD_GROUPS` (`GroundBuffers::grow`).
    pub pool: u32,
    /// The child groups each handed brick's scratch holds a tree of at first
    /// (pxl-kernel domain/scene/param.rs `WAYS_SCRATCH_FIRST_BYTES`): it
    /// doubles each time a build outgrows it, up to
    /// `way::tree::BRICK_CHILD_GROUPS` (`GroundBuffers::grow_scratch`).
    pub scratch: u32,
    /// A read's records' room at first, in bytes: every read's is doubled
    /// past a brick's records that outgrew it, up to
    /// `way::tree::RECORDS_BYTES` (`GroundBuffers::record_bytes`).
    pub records: u32,
}

/// How a brick's fill landed: its cells are back in every case.
pub enum Resident<'a> {
    /// Its ways' tree built: the child groups it holds, their count first, a
    /// record each (pxl-engine `way::tree::lay`).
    Built(&'a [u32]),
    /// To be filled again, for want of room the renderer grows: its tree not
    /// built, the pool or the scratch it was built in having run out
    /// (`Short`), or built but its records past its read's room, the tree
    /// the refill frees.
    Again,
    /// Its tree refused, the pool or the scratch at its most having run out,
    /// or the brick past its own (`way::tree::past`): the brick stays
    /// absent.
    Refused,
}

/// The read-backs a brick's cells and its ways' records come back in
/// (`GroundBuffers::read_back`): two frames' fills (pxl-engine
/// `clipmap::FILL_CAP`), so one frame's mappings are still landing while
/// the next is copied.
const READS: usize = 2 * FILL_CAP as usize;

/// The pool's records the renderer keeps on the heap at their worst, in
/// the buddy's blocks: every read's room at a brick's most
/// (`GroundBuffers::room`), each a power of two as the heap gives it.
pub const RECORD_ROOMS_BYTES: u64 =
    READS as u64 * (way::tree::RECORDS_BYTES as u64).next_power_of_two();

/// A brick whose cells came back, the fill they are of
/// (`CellsBack::ticket`), and how its ways' tree landed; what is told may
/// borrow its caller's for the frame.
pub type OnResident<'a> = dyn FnMut(u32, u32, Resident<'_>) + 'a;

/// Where a brick's cells, which the GPU draws, come back to in the
/// kernel's memory: its columns, levels, laid cells and root
/// (pxl-engine `clipmap::BACK_WORDS`); and the fill's ticket, never 0,
/// which the kernel is told once this fill's are in.
#[derive(Clone, Copy, Default)]
pub struct CellsBack {
    pub cells: u32,
    pub ticket: u32,
}

/// A brick handed to the GPU's fill, the record the ground's clip levels hand
/// on (pxl-kernel domain/layer/terrain/clipmap.rs `fill`): its words
/// (pxl-engine `clipmap::fill_words`), its four palette indices a cell, its tile's
/// padded samples, its lines as far as they are written
/// (`clipmap::LINES_AT`), and where its cells come back to.
pub struct BrickFill<'a> {
    pub words: [u32; FILL_WORDS as usize],
    pub palette_indices: &'a [u8],
    pub samples: &'a [i16],
    pub lines: &'a [u32],
    pub back: CellsBack,
}

/// Takes a brick while the turn's stream has room for it.
pub type TerrainWrite<'a> = dyn FnMut(BrickFill<'_>) -> bool + 'a;

pub type TerrainUpload = dyn for<'w> FnMut(&mut TerrainWrite<'w>);

/// A buffer of them carrying no brick.
const FREE: u32 = u32::MAX;

/// `way_cs`'s group (way.wgsl): the fresh frames, the frames it reads the
/// columns of, and the scratch it keeps the spans in.
const WAY_GROUP: [u32; 9] = [
    0, COMPUTE, STORAGE, 1, COMPUTE, STORAGE, 2, COMPUTE, STORAGE_RW,
];
/// `way_free_cs`'s (way_free.wgsl): the fresh frames, the frames whose roots it
/// reads, the ways' pool it walks and its free stack.
const WAY_FREE_GROUP: [u32; 12] = [
    0, COMPUTE, STORAGE, 1, COMPUTE, STORAGE, 2, COMPUTE, STORAGE, 3, COMPUTE, STORAGE_RW,
];
/// `way_tree_cs`'s (way_tree.wgsl): the fresh frames, the frames whose levels it
/// reads and roots it writes, the pool, its free stack and the scratch.
const WAY_TREE_GROUP: [u32; 15] = [
    0, COMPUTE, STORAGE, 1, COMPUTE, STORAGE_RW, 2, COMPUTE, STORAGE_RW, 3, COMPUTE, STORAGE_RW, 4,
    COMPUTE, STORAGE_RW,
];
/// `way_return_cs`'s (way_return.wgsl): the scratch and the free stack.
const WAY_RETURN_GROUP: [u32; 6] = [0, COMPUTE, STORAGE, 1, COMPUTE, STORAGE_RW];
const _: () = assert!(
    floor::holds(&[&WAY_GROUP])
        && floor::holds(&[&WAY_FREE_GROUP])
        && floor::holds(&[&WAY_TREE_GROUP])
        && floor::holds(&[&WAY_RETURN_GROUP]),
    "a ways' pass's group binds past WebGPU's floor"
);

/// The passes that fill a handed brick: made with the renderer's pipelines
/// (`RaysPass::make`), before its first frame, and taken into its buffers
/// when the first are made (`hand`).
#[derive(Clone, Copy)]
pub(super) struct GroundPipelines {
    heights: Pipeline,
    bound: Pipeline,
    ways: Pipeline,
    free: Pipeline,
    tree: Pipeline,
    give_back: Pipeline,
}

impl GroundPipelines {
    pub(super) fn new(g: &Gpu, pipelines: &mut Pipelines) -> GroundPipelines {
        GroundPipelines {
            heights: pipelines.build_compute(g, SHADER_HEIGHT, &HEIGHT_GROUP),
            bound: pipelines.build_compute(g, SHADER_BOUND, &BOUND_GROUP),
            ways: pipelines.build_compute(g, SHADER_WAY, &WAY_GROUP),
            free: pipelines.build_compute(g, SHADER_WAY_FREE, &WAY_FREE_GROUP),
            tree: pipelines.build_compute(g, SHADER_WAY_TREE, &WAY_TREE_GROUP),
            give_back: pipelines.build_compute(g, SHADER_WAY_RETURN, &WAY_RETURN_GROUP),
        }
    }
}

/// The terrain's frames on the GPU and the passes that draw their cells.
pub(super) struct GroundBuffers {
    /// Per slot of the clip levels' bricks (`clipmap::SLOTS`), its columns,
    /// levels, cells as laid and root, which `height_cs`, `bound_cs` and
    /// `way_tree_cs` write in the turn it is handed (pxl-engine
    /// `clipmap::BRICK_WORDS`).
    pub(super) ground: u32,
    /// Per slot, its cells' four palette indices (`PALETTE_BYTES`).
    pub(super) palette_indices: u32,
    /// The bricks the turn handed, `clipmap::FILL_CAP` at most, a record each
    /// (`clipmap::HANDED_WORDS`): its words, then its palette indices, its tile's
    /// samples and its lines, which only its fill reads; `height_cs`'s,
    /// `bound_cs`'s and `way_cs`'s one per `id.z`.
    pub(super) fresh: u32,
    pub(super) fresh_len: u32,
    /// Per slot (`clipmap::SLOTS`), its cells' way back: the fill
    /// the next landing must be of (its ticket 0 where none is asked).
    cells_back: [CellsBack; SLOTS as usize],
    /// The bricks the frame handed, their words (`clipmap::fill_words`), in the
    /// order handed, each with a read of its own (`hand` hands none past the
    /// reads free): a fill's records live in its frame's scratch, so its
    /// read is taken in its frame.
    pub(super) handed: [[u32; FILL_WORDS as usize]; FILL_CAP as usize],
    /// The buffers cells are copied into and mapped back through, as many as
    /// a frame hands bricks a mapping still out a frame more (`READS`);
    /// each one's brick and the fill it carries (`FREE` where none), and
    /// its mapping's own word (`read_back`), which the host writes: held
    /// where the guest's allocator puts it, inside the region the host
    /// addresses, as an arriving image's word is. A static is outside it on
    /// a device.
    reads: [u32; READS],
    brick_fills: [(u32, u32); READS],
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
    /// child groups and the child groups a brick's scratch held (`ran_out`).
    built_in: [[u32; 2]; READS],
    /// Whether a build ran out of the pool, or of the scratch, since it last
    /// grew, indexed by `Short` (`grow`, `grow_scratch`).
    ran_out: [bool; 2],
    /// The ways' pool, which the cast reads, its child groups and its free stack;
    /// and the handed bricks' scratch, the child groups each brick's holds a
    /// tree of (`way::tree::scratch_words`), whose records each read copies
    /// in the frame they are built: nothing in it outlives its frame.
    pub(super) pool: u32,
    child_groups: u32,
    stack: u32,
    scratch: u32,
    scratch_child_groups: u32,
    /// Each slot's number, `bound_cs`'s level, at a dynamic offset of its
    /// group.
    slots: u32,
    heights_built: Pipeline,
    heights_group: u32,
    bound_built: Pipeline,
    /// `bound_cs`'s group, its level at a dynamic offset (`SLOT_STRIDE`).
    bound_group: u32,
    /// `way_cs`'s, and its group: the fresh frames, the frames it reads
    /// the columns of, and the scratch it keeps the spans in.
    ways_built: Pipeline,
    ways_group: u32,
    /// A re-filled brick's old ways' tree given back, its new one built,
    /// the child groups the build gave back pushed (pxl-engine `way::tree`):
    /// each pass and its group, over the ways' pool, child pointers then data
    /// words (`pool_bytes`), its free stack, the count taken then the ids,
    /// child group 0 never among them, a word a child group, and each handed brick's
    /// scratch past its build (`way::tree::FREED_AT`).
    free_built: Pipeline,
    free_group: u32,
    tree_built: Pipeline,
    tree_group: u32,
    return_built: Pipeline,
    freed_group: u32,
}

/// The ways' pool of `child_groups` on the GPU, in bytes: a slot two words,
/// eight a child group. Its free stack is a word a child group: the count taken, then
/// every child group's id but child group 0's, every child group free but child group 0 as WebGPU
/// makes it, zeroed (pxl-engine `way::tree::TAKEN_CNT`), so nothing is written to
/// start it, nor to grow it.
const fn pool_bytes(child_groups: u32) -> u32 {
    child_groups * way::tree::CHILD_GROUP_WORDS * 4
}
const _: () = assert!(
    floor::fits(pool_bytes(way::tree::POOL_CHILD_GROUPS) as u64, true),
    "the ways' pool at its most binds past WebGPU's floor (frame/shader/floor.rs `fits`)"
);

/// The ways' pool of `child_groups` and its free stack, zeroed: each copied
/// from as it grows, and into.
fn pool_bufs(g: &Gpu, child_groups: u32) -> (u32, u32) {
    let usage = buffer_usage::STORAGE | buffer_usage::COPY_SRC | buffer_usage::COPY_DST;
    (
        g.buf(pool_bytes(child_groups), usage),
        g.buf(word_bytes(child_groups as usize), usage),
    )
}

/// The handed bricks' scratch, each brick's for a tree of `child_groups`, in
/// bytes: `FILL_CAP` of them, every brick's alike, so each finds its own off
/// the buffer's length (pxl-engine fill.rs `scratch_at`).
const fn scratch_bytes(child_groups: u32) -> u32 {
    FILL_CAP * way::tree::scratch_words(child_groups) * 4
}
const _: () = assert!(
    floor::fits(scratch_bytes(way::tree::BRICK_CHILD_GROUPS) as u64, true),
    "the scratch at its most binds past WebGPU's floor (frame/shader/floor.rs `fits`)"
);

/// The handed bricks' scratch, each brick's for a tree of `child_groups`,
/// which its records are copied out of.
fn scratch_buf(g: &Gpu, child_groups: u32) -> u32 {
    g.buf(
        scratch_bytes(child_groups),
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

/// What a build whose records' count is `n` ran out of (way_tree.wgsl
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
/// of `built` child groups, the one it ran out of now of `child_groups`, at most `most`.
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
fn ran_out(built: u32, child_groups: u32, most: u32) -> RanOut {
    if built < child_groups {
        RanOut::Again
    } else if child_groups < most {
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
                buf_range_entry(0, self.slots, 0, 16),
                buf_entry(1, fresh),
                buf_entry(2, ground),
                buf_entry(3, scratch),
            ],
        );
        self.ways_group = g.bind_group(
            self.ways_built.layouts[0],
            &[
                buf_entry(0, fresh),
                buf_entry(1, ground),
                buf_entry(2, scratch),
            ],
        );
        self.free_group = g.bind_group(
            self.free_built.layouts[0],
            &[
                buf_entry(0, fresh),
                buf_entry(1, ground),
                buf_entry(2, pool),
                buf_entry(3, stack),
            ],
        );
        self.tree_group = g.bind_group(
            self.tree_built.layouts[0],
            &[
                buf_entry(0, fresh),
                buf_entry(1, ground),
                buf_entry(2, pool),
                buf_entry(3, stack),
                buf_entry(4, scratch),
            ],
        );
        self.freed_group = g.bind_group(
            self.return_built.layouts[0],
            &[buf_entry(0, scratch), buf_entry(1, stack)],
        );
    }

    /// The ways' pool twice as large, never past `way::tree::POOL_CHILD_GROUPS`,
    /// the bake's bound, at which no brick of a baked set is refused, once a
    /// build ran out of it (`read_back`): its child pointers and its stack
    /// copied where they were, its data words to the new data's first, so
    /// every id a tree or a read in flight names holds what it held, and the
    /// stack's slots past the old hold the new child groups free (pxl-engine
    /// `way::tree::TAKEN_CNT`). Ahead of the turn's bricks, so their builds and the cast
    /// read the new one; the cast's groups follow its id (`prepare`).
    ///
    /// A known gap: a device out of memory may fail to make the doubled
    /// buffer, and nothing hears it, the host keeping no error scope, so the
    /// renderer reads on with a buffer the device holds as an error. The
    /// last doubling holds the old pool beside the new, 32 MiB with 64,
    /// until the copy is submitted; the scratch's growth has the same gap.
    fn grow(&mut self, g: &Gpu) {
        let child_groups = (2 * self.child_groups).min(way::tree::POOL_CHILD_GROUPS);
        if child_groups <= self.child_groups {
            return;
        }
        let (pool, stack) = pool_bufs(g, child_groups);
        let half = |child_groups: u32| pool_bytes(child_groups) / 2;
        let e = g.encoder();
        g.copy_buf(e, (self.pool, 0), (pool, 0), half(self.child_groups));
        g.copy_buf(
            e,
            (self.pool, half(self.child_groups)),
            (pool, half(child_groups)),
            half(self.child_groups),
        );
        g.copy_buf(
            e,
            (self.stack, 0),
            (stack, 0),
            word_bytes(self.child_groups as usize),
        );
        g.submit(e);
        release(self.pool);
        release(self.stack);
        (self.pool, self.stack, self.child_groups) = (pool, stack, child_groups);
        self.ways_groups(g);
    }

    /// Each brick's tree room in the scratch twice as many child groups, never
    /// past `way::tree::BRICK_CHILD_GROUPS`, the bake's bound on a brick's, once
    /// a build outgrew it (`read_back`); its spans stay as
    /// they are (`clipmap::TREE_AT`), so the buffer grows by less than twice.
    /// Nothing is copied: a brick's records are copied into its read in the
    /// frame it is filled in, before any turn grows it. Ahead of the turn's
    /// bricks, so their passes lay theirs out by the new length.
    fn grow_scratch(&mut self, g: &Gpu) {
        let child_groups = (2 * self.scratch_child_groups).min(way::tree::BRICK_CHILD_GROUPS);
        if child_groups <= self.scratch_child_groups {
            return;
        }
        let scratch = scratch_buf(g, child_groups);
        release(self.scratch);
        (self.scratch, self.scratch_child_groups) = (scratch, child_groups);
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
        self.record_reads[k] = g.buf(
            word_bytes(room.len()),
            buffer_usage::MAP_READ | buffer_usage::COPY_DST,
        );
        self.record_rooms[k] = room.into_boxed_slice();
    }
}

/// A brick on the GPU, in bytes.
const BRICK_BYTES: u32 = BRICK_WORDS * 4;

/// The clip levels' bricks on the GPU, in bytes: one storage buffer, bound
/// whole by the cast's, the levels' and the ways' groups.
const GROUND_BYTES: u64 = SLOTS as u64 * BRICK_BYTES as u64;
const _: () = assert!(
    floor::fits(GROUND_BYTES, true),
    "GROUND_BYTES: the clip levels' bricks bind past WebGPU's floor (frame/shader/floor.rs `fits`)"
);

/// The clip levels' bricks' cells' four palette indices, a word a cell in the
/// slots' order: written by the fill off each handed brick (`height_cs`), read
/// by the cast at a hit (palette.wgsl). Their own
/// buffer, as the bricks' binding has no room for them.
const PALETTE_BYTES: u64 = SLOTS as u64 * BRICK_CELLS as u64 * 4;
const _: () = assert!(
    floor::fits(PALETTE_BYTES, true),
    "PALETTE_BYTES: the bricks' palette indices bind past WebGPU's floor (frame/shader/floor.rs `fits`)"
);

/// `height_cs`'s group (height.wgsl): the fresh frames, the frames it writes
/// the tops and laid cells of, and the bricks' palette indices.
const HEIGHT_GROUP: [u32; 9] = [
    0, COMPUTE, STORAGE, 1, COMPUTE, STORAGE_RW, 2, COMPUTE, STORAGE_RW,
];
/// `bound_cs`'s group (bound.wgsl): its level at a dynamic offset
/// (`SLOT_STRIDE`), the fresh frames, the frames it writes the levels of, and
/// the handed bricks' scratch it reads the spans in. The cast's and
/// `fold_cs`'s one group is the cast's, which binds all of theirs.
const BOUND_GROUP: [u32; 12] = dynamic(
    [
        0, COMPUTE, UNIFORM, 1, COMPUTE, STORAGE, 2, COMPUTE, STORAGE_RW, 3, COMPUTE, STORAGE,
    ],
    0,
);
const _: () = assert!(
    floor::holds(&[&HEIGHT_GROUP]) && floor::holds(&[&BOUND_GROUP]),
    "the fill's tops' or levels' group binds past WebGPU's floor"
);
/// A number's slot in the fill's numbers, read at a dynamic offset: every
/// device's dynamic offset alignment.
const SLOT_STRIDE: u32 = UNIFORM_ALIGN_MAX;

impl RaysPass {
    /// Each held tile the GPU does not yet hold handed to its frame, copied
    /// into the turn's stream as it has room, the packets they live in let
    /// go once handed; whether or not an eye casts, as a tile's cells are
    /// no eye's.
    pub fn hand(&mut self, g: &Gpu, ground: &mut GroundBricks) {
        let child_groups = ground.pool.min(way::tree::POOL_CHILD_GROUPS);
        let scratch_child_groups = ground.scratch.clamp(1, way::tree::BRICK_CHILD_GROUPS);
        let built = self.ground_built.or_bug(Errno::ENOTRECOVERABLE);
        let gb = self.ground.get_or_insert_with(|| {
            let buf = |size: u32, usage: u32| g.buf(size, usage);
            let storage = buffer_usage::STORAGE | buffer_usage::COPY_DST;
            let frames_buf = buf(GROUND_BYTES as u32, storage | buffer_usage::COPY_SRC);
            let fresh = buf(word_bytes((HANDED_WORDS * FILL_CAP) as usize), storage);
            let palette_indices = buf(PALETTE_BYTES as u32, storage);
            let heights_built = built.heights;
            let heights_group = g.bind_group(
                heights_built.layouts[0],
                &[
                    buf_entry(0, fresh),
                    buf_entry(1, frames_buf),
                    buf_entry(2, palette_indices),
                ],
            );
            // Each slot's number in it, written once: `bound_cs`'s level.
            let slots = buf(
                (LEVELS + 1) * SLOT_STRIDE,
                buffer_usage::UNIFORM | buffer_usage::COPY_DST,
            );
            for k in 0..=LEVELS {
                g.write_buf(slots, k * SLOT_STRIDE, bytes(&[k, 0, 0, 0]));
            }
            let scratch = scratch_buf(g, scratch_child_groups);
            let (bound_built, ways_built) = (built.bound, built.ways);
            let (pool, stack) = pool_bufs(g, child_groups);
            let (free_built, tree_built, return_built) = (built.free, built.tree, built.give_back);
            let mut gb = GroundBuffers {
                ground: frames_buf,
                palette_indices,
                fresh,
                fresh_len: 0,
                cells_back: [CellsBack::default(); SLOTS as usize],
                handed: [[0; FILL_WORDS as usize]; FILL_CAP as usize],
                reads: core::array::from_fn(|_| {
                    buf(
                        BACK_WORDS * 4,
                        buffer_usage::MAP_READ | buffer_usage::COPY_DST,
                    )
                }),
                brick_fills: [(FREE, 0); READS],
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
                child_groups,
                stack,
                ran_out: [false; 2],
                scratch,
                scratch_child_groups,
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
                return_built,
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
            .brick_fills
            .iter()
            .filter(|h| h.0 == FREE)
            .count()
            .min(FILL_CAP as usize) as u32;
        let mut fresh = 0;
        (ground.upload)(&mut |q| {
            if fresh == free
                || !room(
                    write_buffer_bytes(q.palette_indices)
                        + write_buffer_bytes(bytes(q.samples))
                        + write_buffer_bytes(bytes(q.lines))
                        + write_buffer_bytes(bytes(&q.words)),
                )
            {
                return false;
            }
            let (brick, at) = (q.words[FILL_BRICK as usize], clipmap::handed_at(fresh) * 4);
            g.write_buf(gb.fresh, at, bytes(&q.words));
            g.write_buf(gb.fresh, at + PALETTE_AT * 4, q.palette_indices);
            g.write_buf(gb.fresh, at + ELE_AT * 4, bytes(q.samples));
            g.write_buf(gb.fresh, at + LINES_AT * 4, bytes(q.lines));
            gb.cells_back[brick as usize] = q.back;
            gb.handed[fresh as usize] = q.words;
            fresh += 1;
            true
        });
        gb.fresh_len = fresh;
    }

    /// The bricks this turn handed filled: their tops from their samples,
    /// their ways' spans, their levels a dispatch, level 0 a column's own and
    /// each over the one before, and their ways' trees. Ahead of the frame,
    /// as they outlive it, so no brick the clip level names is cast before its
    /// cells are written, even when the frame proper is dropped. Whether it
    /// recorded any.
    pub fn grids(&mut self, ctx: &mut RenderContext) -> bool {
        let (g, encoder) = (ctx.g, ctx.encoder);
        let stamp = ctx.stamp(GpuSpanName::fill);
        let Some(c) = self.ground.as_mut().filter(|c| c.fresh_len != 0) else {
            return false;
        };
        let pass = g.compute_pass(encoder, stamp.pass());
        // A re-filled brick's old ways' child groups given back, off its root word
        // before its build writes another (pxl-engine `way::tree`).
        g.set_pipeline(pass, c.free_built.id);
        g.set_bind_group(pass, 0, c.free_group, &[]);
        g.dispatch(pass, 1, 1, c.fresh_len);
        g.set_pipeline(pass, c.heights_built.id);
        g.set_bind_group(pass, 0, c.heights_group, &[]);
        g.dispatch(
            pass,
            BRICK.div_ceil(HEIGHT_SIDE),
            BRICK.div_ceil(HEIGHT_SIDE),
            c.fresh_len,
        );
        // A column a thread, over the tops just drawn: each column's spans
        // once, the brick's and its ring's (pxl-engine fill.rs `spans`),
        // which the levels and the trees read.
        g.set_pipeline(pass, c.ways_built.id);
        g.set_bind_group(pass, 0, c.ways_group, &[]);
        g.dispatch(
            pass,
            clipmap::SPANS_COLUMNS.div_ceil(fill::SPANS_LANES),
            1,
            c.fresh_len,
        );
        g.set_pipeline(pass, c.bound_built.id);
        for j in 0..=LEVELS {
            g.set_bind_group(pass, 0, c.bound_group, &[j * SLOT_STRIDE]);
            let side = (BRICK >> j).div_ceil(LEVEL_SIDE);
            g.dispatch(pass, side, side, c.fresh_len);
        }
        // Each brick's ways as cells, a workgroup a brick, over its spans
        // and the levels just folded; then what its build gave back pushed,
        // a dispatch after, so no push lands under another brick's ids
        // while it reads them.
        g.set_pipeline(pass, c.tree_built.id);
        g.set_bind_group(pass, 0, c.tree_group, &[]);
        g.dispatch(pass, 1, 1, c.fresh_len);
        g.set_pipeline(pass, c.return_built.id);
        g.set_bind_group(pass, 0, c.freed_group, &[]);
        g.dispatch(pass, 1, 1, c.fresh_len);
        g.end(pass);
        true
    }

    /// The bricks the grids wrote mapped back where the CPU looks them up,
    /// once the encoder that wrote them is submitted: a brick's columns,
    /// levels, cells as laid and root lie together (`clipmap::BACK_WORDS`), so one mapping
    /// brings them all.
    ///
    /// A brick asked takes a free buffer of `READS`, which maps once at a
    /// time and lands whatever fill it carried. One that landed for the
    /// brick's latest fill is told `on_resident`. One that landed for an earlier
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
        let on_resident = &mut *ground.on_resident;
        let most = [way::tree::POOL_CHILD_GROUPS, way::tree::BRICK_CHILD_GROUPS];
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
            let (s, ticket) = c.brick_fills[k];
            if s == FREE || c.mapping[k] & DONE == 0 || c.record_mapping[k] & DONE == 0 {
                continue;
            }
            c.brick_fills[k] = (FREE, 0);
            if c.cells_back[s as usize].ticket != ticket {
                continue;
            }
            c.cells_back[s as usize].ticket = 0;
            let room = &c.record_rooms[k];
            let n = room.first().copied().unwrap_or(0);
            let words = 1 + n.min(way::tree::BRICK_CHILD_GROUPS) * way::tree::RECORD_WORDS;
            let landing = if n == way::tree::NONE {
                Resident::Refused
            } else if let Some(short) = short_of(n) {
                let (i, now) = (short as usize, [c.child_groups, c.scratch_child_groups]);
                match ran_out(c.built_in[k][i], now[i], most[i]) {
                    RanOut::Again => Resident::Again,
                    RanOut::Grow => {
                        c.ran_out[i] = true;
                        Resident::Again
                    }
                    RanOut::Refused => Resident::Refused,
                }
            } else if words as usize > room.len() {
                c.record_bytes = c.record_bytes.max(
                    word_bytes(words as usize)
                        .next_power_of_two()
                        .min(way::tree::RECORDS_BYTES),
                );
                Resident::Again
            } else {
                Resident::Built(&room[..words as usize])
            };
            on_resident(s, ticket, landing);
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
            let s = c.handed[h as usize][FILL_BRICK as usize];
            let Some(k) = (free..READS).find(|&k| c.brick_fills[k].0 == FREE) else {
                break;
            };
            free = k + 1;
            c.room(g, k);
            g.copy_buf(
                e,
                (c.ground, s * BRICK_BYTES + COLUMNS_AT * 4),
                (c.reads[k], 0),
                BACK_WORDS * 4,
            );
            let sb = c.scratch_child_groups;
            let records = (h * way::tree::scratch_words(sb) + way::tree::records_at(sb)) as usize;
            let words = c.record_rooms[k]
                .len()
                .min((1 + sb * way::tree::RECORD_WORDS) as usize);
            g.copy_buf(
                e,
                (c.scratch, word_bytes(records)),
                (c.record_reads[k], 0),
                word_bytes(words),
            );
            (c.brick_fills[k], c.built_in[k], taken[k]) = (
                (s, c.cells_back[s as usize].ticket),
                [c.child_groups, sb],
                true,
            );
        }
        g.submit(e);
        for k in (0..READS).filter(|&k| taken[k]) {
            let (s, ticket) = c.brick_fills[k];
            g.map_read(
                c.reads[k],
                0,
                BACK_WORDS * 4,
                c.cells_back[s as usize].cells,
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
