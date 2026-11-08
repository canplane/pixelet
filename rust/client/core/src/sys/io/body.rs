// pxl-core/sys/io/body.rs

//! The blocks bodies are read into. Terrain's and the Earth's packets run
//! just past a mebibyte (pxl-spec `bake::tile::max_pkt_bytes`), where the
//! buddy would give each a block of two; their bodies take slots instead,
//! their longest packet in pages, carved from a run the heap gives whole and
//! takes back once its last slot is free: Bonwick's slab for one large
//! object, its bookkeeping off the run, picked by the stream. A run is
//! borrowed only when every one held is full, and no more slots are out than
//! `BODY_SLOTS` (sys/io/param.rs). Every other body is a heap block of its
//! own, freed as it is decoded.
//!
//! A terrain body is freed once decoded; an Earth body only by a drawn frame
//! (domain/layer/earth.rs `earth_upload`), bounded apart by its frames, so a
//! body on every credit finds a slot and the tile cache fills with no frame
//! drawn. A body refused waits on what refused it (sys/io/room.rs): every
//! slot out on one given back, a run the heap could not give on its pages or
//! a slot given back in a run held. The next slot given back asks a turn, in
//! which the want is asked again.

use xpute_core::global::Global;
use xpute_core::mem::heap::{alloc, free, Block};
use xpute_core::mem::section::PAGE_BYTES;

use crate::sys::io::demand::Stream;
use crate::sys::io::param::{BODY_RUN_BYTES, BODY_SLOTS};
use crate::sys::io::room::Refusal;
use crate::sys::sched::edge;

struct Run {
    at: usize,
    /// A bit a slot out.
    out: u32,
}

#[derive(Default)]
struct Slots {
    runs: Vec<Run>,
    /// Moves with every slot given back (`Refusal::bodies`).
    releases: u32,
    /// The most out at once since the profiler last took it.
    peak: u32,
    /// A body was refused: the next slot given back asks a turn.
    waited: bool,
}

/// A slotted stream's longest packet; 0 for one whose bodies take no slot.
const fn longest(stream: Stream) -> u32 {
    match stream.max_body() {
        Some(max) if stream.row().slotted => max,
        _ => 0,
    }
}

/// The longest slotted packet, in whole pages.
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

/// A run's slots: seven at today's packets.
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

static SLOT_TABLE: Global<Slots> = Global::new();

fn slot_table() -> &'static mut Slots {
    SLOT_TABLE.get(Slots::default)
}

impl Slots {
    fn out(&self) -> u32 {
        self.runs.iter().map(|r| r.out.count_ones()).sum()
    }
}

/// The heap `slots` slots out at once take at worst: the runs that hold them.
pub const fn slots_bytes(slots: u32) -> u64 {
    slots.div_ceil(SLOTS) as u64 * BODY_RUN_BYTES as u64
}

/// A block for a body of `n` bytes, of `stream` where the requests still
/// hold it: a slot where its stream's bodies take one, else the heap's own.
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
    // A block in a run that is no slot out is given back twice, or was never
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

/// The most slots out at once since the last call, the profiler's
/// (perf/report.rs `perf_frame`): what `BODY_SLOTS` is measured by.
pub fn take_peak() -> u32 {
    let s = slot_table();
    let out = s.out();
    core::mem::replace(&mut s.peak, out)
}

#[cfg(test)]
#[path = "body.test.rs"]
mod test;
