// pxl-core/sys/io/body.rs

//! The blocks that bodies are read into. Terrain and Earth packets run just
//! past a mebibyte, where the buddy allocator would give each a block of two.
//! Their bodies take slots instead. A slot is an even share of a run in whole
//! pages, which holds an Earth tile and a terrain tile with the ways it
//! usually has, cut from a run that the heap gives whole and takes back once
//! the run's last slot is free. A terrain body longer than a slot, as far as a
//! packet can be (pxl-spec `BaseLayer::row`), is a heap block of
//! its own. This is Bonwick's slab allocator for one large object, with
//! its bookkeeping kept outside the run, and the stream and the body's length
//! decide whether it takes a slot. A new run is taken only when every run held is full,
//! and no more than `BODY_SLOTS` slots are out at once (sys/io/param.rs).
//! Every other body is a heap block of its own, freed as it is decoded.
//!
//! A body is a `Packet`, owned by what holds it and given back when it is
//! dropped. A terrain body is dropped once it is decoded and its clip levels
//! have cut what they want. An Earth body is dropped only when a drawn frame
//! takes its texels (domain/layer/earth.rs `upload`). The
//! Earth's frames bound those bodies apart from the rest, so a body on every
//! credit still finds a slot and the tile cache fills while no frame is
//! drawn. A refused body waits on what refused it (sys/io/room.rs). When
//! every slot was out, it waits for a slot to be given back. When the heap
//! could not give a run, it waits for the heap to get pages back or for a
//! slot in a run held to be given back. After a refusal, the next slot given
//! back asks for a turn, in which the want is asked again.

use pxl_spec::bake::tile::terrain::{ELE_COLUMN, FOLD_COLUMN};
use xpute_core::global::Global;
use xpute_core::mem;
use xpute_core::mem::heap::{alloc, free, Block};
use xpute_core::mem::section::PAGE_BYTES;

use crate::sys::io::demand::Stream;
use crate::sys::io::param::{BODY_RUN_BYTES, BODY_RUN_SLOTS, BODY_SLOTS};
use crate::sys::io::room::Refusal;
use crate::sys::sched;

struct Run {
    at: usize,
    /// One bit for each slot, set while the slot is out.
    out: u32,
}

#[derive(Default)]
struct SlotTable {
    runs: Vec<Run>,
    /// A count that moves with every slot given back. A refusal by the body
    /// slots waits on it (`Refusal::bodies`).
    releases: u32,
    /// The most slots out at once since the profiler last took the peak.
    peak: u32,
    /// A body was refused, so the next slot given back asks for a turn.
    waited: bool,
}

/// The longest packet of a slotted stream, or 0 for a stream whose bodies
/// take no slot.
const fn longest(stream: Stream) -> u32 {
    match stream.max_body() {
        Some(max) if stream.row().slotted => max,
        _ => 0,
    }
}

/// The size of a slot: an even share of a run, in whole pages.
const SLOT_BYTES: u32 = BODY_RUN_BYTES / BODY_RUN_SLOTS / PAGE_BYTES * PAGE_BYTES;

/// The slots in a run.
const SLOTS: u32 = BODY_RUN_BYTES / SLOT_BYTES;

const FULL: u32 = u32::MAX >> (u32::BITS - SLOTS);

const _: () = {
    assert!(
        BODY_RUN_BYTES.is_power_of_two(),
        "a run is the buddy's own block"
    );
    assert!(
        SLOTS >= 2 && SLOTS <= u32::BITS,
        "a run holds two slots or more, a bit each"
    );
    assert!(SLOTS == BODY_RUN_SLOTS, "a run holds its slots whole");
    let mut k = 0;
    while k < Stream::ALL.len() {
        let s = Stream::ALL[k];
        assert!(
            !s.row().slotted || BODY_RUN_BYTES / SLOTS < longest(s).next_power_of_two(),
            "a slot with its share of the run costs less than the buddy's block for it"
        );
        k += 1;
    }
    assert!(
        longest(Stream::earth) <= SLOT_BYTES,
        "an Earth tile fits a slot"
    );
    assert!(
        (FOLD_COLUMN + 2 * ELE_COLUMN) as u32 <= SLOT_BYTES,
        "a terrain tile's columns fit a slot"
    );
};

static SLOT_TABLE: Global<SlotTable> = Global::new();

fn slot_table() -> &'static mut SlotTable {
    SLOT_TABLE.get(SlotTable::default)
}

impl SlotTable {
    fn out(&self) -> u32 {
        self.runs.iter().map(|r| r.out.count_ones()).sum()
    }
}

/// The most heap that `slot_cnt` slots out at once can take: the whole runs that
/// hold them.
pub const fn slots_bytes(slot_cnt: u32) -> u64 {
    slot_cnt.div_ceil(SLOTS) as u64 * BODY_RUN_BYTES as u64
}

/// A body read into a block: what the fetch fills, what the layer that asked
/// reads, and what goes back when its holder lets it go.
pub struct Packet(Block);

impl core::ops::Deref for Packet {
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        // SAFETY: the block `take` made for this body alone, `self.0.1` long,
        // read by its holder only once the runtime has filled it.
        unsafe { core::slice::from_raw_parts(mem::at::<u8>(self.0 .0), self.0 .1 as usize) }
    }
}

impl core::ops::DerefMut for Packet {
    fn deref_mut(&mut self) -> &mut [u8] {
        // SAFETY: as `deref`, and its only writer is the fetch that fills it.
        unsafe { core::slice::from_raw_parts_mut(mem::at::<u8>(self.0 .0), self.0 .1 as usize) }
    }
}

impl Drop for Packet {
    fn drop(&mut self) {
        // SAFETY: a block `take` made, given back once, here.
        unsafe { give(self.0) };
    }
}

/// A block for a body of `n` bytes. `stream` is the body's stream, or None
/// where the scheduler no longer holds its attempt. The block is a slot where
/// the stream's bodies take one and `n` fits it, and a heap block of its own
/// otherwise.
pub fn take(stream: Option<Stream>, n: u32) -> Result<Packet, Refusal> {
    if !stream.is_some_and(|s| s.row().slotted) || n > SLOT_BYTES {
        return alloc(n).map(Packet).ok_or_else(Refusal::now);
    }
    let s = slot_table();
    let out = s.out();
    if out >= BODY_SLOTS {
        s.waited = true;
        return Err(Refusal::bodies());
    }
    let i = match s.runs.iter().position(|r| r.out != FULL) {
        Some(i) => i,
        None => {
            let Some((at, _)) = alloc(BODY_RUN_BYTES) else {
                s.waited = true;
                return Err(Refusal::run());
            };
            s.runs.push(Run { at, out: 0 });
            s.runs.len() - 1
        }
    };
    s.peak = s.peak.max(out + 1);
    let run = &mut s.runs[i];
    let slot = (!run.out).trailing_zeros();
    run.out |= 1 << slot;
    Ok(Packet((run.at + (slot * SLOT_BYTES) as usize, n)))
}

/// # Safety
/// `block` came from `take` and has not been given back.
unsafe fn give(block: Block) {
    let s = slot_table();
    let Some(i) = s
        .runs
        .iter()
        .position(|r| (r.at..r.at + BODY_RUN_BYTES as usize).contains(&block.0))
    else {
        // SAFETY: the caller's; a block outside every run is the heap's own.
        return unsafe { free(block) };
    };
    let run = &mut s.runs[i];
    let off = block.0 - run.at;
    // A block in a run that is not a slot out was given back twice or never
    // taken.
    xpute_kit::ensure!(
        off < (SLOTS * SLOT_BYTES) as usize && off.is_multiple_of(SLOT_BYTES as usize),
        ENOTRECOVERABLE,
        off
    );
    let bit = 1 << (off / SLOT_BYTES as usize);
    xpute_kit::ensure!(run.out & bit != 0, ENOTRECOVERABLE, off);
    run.out &= !bit;
    s.releases = s.releases.wrapping_add(1);
    if core::mem::take(&mut s.waited) {
        sched::request_world_tick();
    }
    if run.out == 0 {
        let run = s.runs.swap_remove(i);
        // SAFETY: the run `take` borrowed, no slot of it out.
        unsafe { free((run.at, BODY_RUN_BYTES)) };
    }
}

pub fn releases() -> u32 {
    slot_table().releases
}

/// The most slots out at once since the last call. The profiler takes it
/// (perf/report.rs `perf_frame`), and `BODY_SLOTS` is judged against it.
pub fn take_peak() -> u32 {
    let s = slot_table();
    let out = s.out();
    core::mem::replace(&mut s.peak, out)
}

#[cfg(test)]
#[path = "body.test.rs"]
mod test;
