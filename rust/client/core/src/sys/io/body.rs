// pxl-core/sys/io/body.rs

//! The blocks that bodies are read into. Terrain and Earth packets run just
//! past a mebibyte (pxl-spec `bake::tile::max_pkt_bytes`), where the buddy
//! allocator would give each a block of two. Their bodies take slots
//! instead. A slot is the longest such packet rounded up to whole pages, cut
//! from a run that the heap gives whole and takes back once the run's last
//! slot is free. This is Bonwick's slab allocator for one large object, with
//! its bookkeeping kept outside the run, and the stream decides whether a
//! body takes a slot. A new run is taken only when every run held is full,
//! and no more than `BODY_SLOTS` slots are out at once (sys/io/param.rs).
//! Every other body is a heap block of its own, freed as it is decoded.
//!
//! A terrain body is freed once it is decoded. An Earth body is freed only
//! when a drawn frame takes it (domain/layer/earth.rs `earth_upload`). The
//! Earth's frames bound those bodies apart from the rest, so a body on every
//! credit still finds a slot and the tile cache fills while no frame is
//! drawn. A refused body waits on what refused it (sys/io/room.rs). When
//! every slot was out, it waits for a slot to be given back. When the heap
//! could not give a run, it waits for the heap to get pages back or for a
//! slot in a run held to be given back. After a refusal, the next slot given
//! back asks for a turn, in which the want is asked again.

use xpute_core::global::Global;
use xpute_core::mem::heap::{alloc, free, Block};
use xpute_core::mem::section::PAGE_BYTES;

use crate::sys::io::demand::Stream;
use crate::sys::io::param::{BODY_RUN_BYTES, BODY_SLOTS};
use crate::sys::io::room::Refusal;
use crate::sys::sched::edge;

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

/// The size of a slot: the longest slotted packet, rounded up to whole
/// pages.
const SLOT_BYTES: u32 = {
    let (mut most, mut k) = (0, 0);
    while k < Stream::ALL.len() {
        if longest(Stream::ALL[k]) > most {
            most = longest(Stream::ALL[k]);
        }
        k += 1;
    }
    most.div_ceil(PAGE_BYTES) * PAGE_BYTES
};

/// The slots in a run: seven at the current packet sizes.
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
    let mut k = 0;
    while k < Stream::ALL.len() {
        let s = Stream::ALL[k];
        assert!(
            !s.row().slotted || BODY_RUN_BYTES / SLOTS < longest(s).next_power_of_two(),
            "a slot with its share of the run costs less than the buddy's block for it"
        );
        k += 1;
    }
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

/// The most heap that `slots` slots out at once can take: the whole runs that
/// hold them.
pub const fn slots_bytes(slots: u32) -> u64 {
    slots.div_ceil(SLOTS) as u64 * BODY_RUN_BYTES as u64
}

/// A block for a body of `n` bytes. `stream` is the body's stream, or None
/// where the scheduler no longer holds its attempt. The block is a slot where
/// the stream's bodies take one, and a heap block of its own otherwise.
pub fn take(stream: Option<Stream>, n: u32) -> Result<Block, Refusal> {
    if !stream.is_some_and(|s| s.row().slotted) || n > SLOT_BYTES {
        return alloc(n).ok_or_else(Refusal::now);
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
    Ok((run.at + (slot * SLOT_BYTES) as usize, n))
}

/// # Safety
/// `block` came from `take` and has not been given back.
pub unsafe fn give(block: Block) {
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
        edge::request_world_tick();
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
