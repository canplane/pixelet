// pxl-kernel/io/body.rs

//! The blocks bodies are read into. Terrain's and the Earth's packets run
//! just past a mebibyte (a tile's looks and the Earth's texels are one
//! each, pxl-spec `bake::tile::max_packet_bytes`), where the buddy would
//! give each body a block of two. Their bodies take slots instead, each
//! their longest packet in pages, carved from a run the heap gives whole
//! and takes back once its last slot is free. A run is borrowed only when
//! every one held is full, so no more are held than the most slots out at
//! once fill. Every other body is a heap block of its own.

use xpute_guest::global::Global;
use xpute_guest::mem::heap::{alloc, free, Block};
use xpute_guest::mem::section::PAGE_BYTES;

use crate::io::demand::Stream;

/// The kinds whose bodies take a slot.
const SLOTTED: [Stream; 2] = [Stream::terrain, Stream::earth];

const fn longest(stream: Stream) -> u32 {
    match stream.max_body() {
        Some(max) => max,
        None => 0,
    }
}

/// The longest slotted packet, in whole pages.
const SLOT_BYTES: u32 = {
    let mut most = 0;
    let mut k = 0;
    while k < SLOTTED.len() {
        if longest(SLOTTED[k]) > most {
            most = longest(SLOTTED[k]);
        }
        k += 1;
    }
    most.div_ceil(PAGE_BYTES) * PAGE_BYTES
};

/// A power of two, the buddy's own block; seven slots at today's packets.
const RUN_BYTES: u32 = 8 << 20;

const SLOTS: u32 = RUN_BYTES / SLOT_BYTES;

const FULL: u32 = if SLOTS == u32::BITS { u32::MAX } else { (1 << SLOTS) - 1 };

const _: () = {
    assert!(SLOTS >= 2 && SLOTS <= u32::BITS, "a run holds two slots or more, a bit each");
    let mut k = 0;
    while k < SLOTTED.len() {
        assert!(
            RUN_BYTES / SLOTS < longest(SLOTTED[k]).next_power_of_two(),
            "a slot with its share of the run costs less than the buddy's block for it"
        );
        k += 1;
    }
};

pub const fn slotted(stream: Stream) -> bool {
    let mut k = 0;
    while k < SLOTTED.len() {
        if SLOTTED[k] as u8 == stream as u8 {
            return true;
        }
        k += 1;
    }
    false
}

/// The heap `slots` slots out at once take at worst: the runs that hold them.
pub const fn slots_bytes(slots: u32) -> u64 {
    slots.div_ceil(SLOTS) as u64 * RUN_BYTES as u64
}

struct Run {
    at: usize,
    /// A bit a slot out.
    out: u32,
}

#[derive(Default)]
struct Slots {
    runs: Vec<Run>,
    releases: u32,
}

static HELD: Global<Slots> = Global::new();

fn held() -> &'static mut Slots {
    HELD.get(Slots::default)
}

/// A block for a body of `n` bytes, of `stream` where the request
/// scheduler still holds it; None where the heap refused it room.
pub fn take(stream: Option<Stream>, n: u32) -> Option<Block> {
    if !stream.is_some_and(slotted) || n > SLOT_BYTES {
        return alloc(n);
    }
    let runs = &mut held().runs;
    let i = match runs.iter().position(|r| r.out != FULL) {
        Some(i) => i,
        None => {
            let (at, _) = alloc(RUN_BYTES)?;
            runs.push(Run { at, out: 0 });
            runs.len() - 1
        }
    };
    let run = &mut runs[i];
    let slot = (!run.out).trailing_zeros();
    run.out |= 1 << slot;
    Some((run.at + (slot * SLOT_BYTES) as usize, n))
}

/// # Safety
/// `block` came from `take` and has not been given back.
pub unsafe fn give(block: Block) {
    let s = held();
    let Some(i) = s.runs.iter().position(|r| (r.at..r.at + RUN_BYTES as usize).contains(&block.0)) else {
        // SAFETY: the caller's; a block outside every run is the heap's own.
        return unsafe { free(block) };
    };
    s.releases = s.releases.wrapping_add(1);
    let run = &mut s.runs[i];
    run.out &= !(1 << ((block.0 - run.at) / SLOT_BYTES as usize));
    if run.out == 0 {
        let run = s.runs.swap_remove(i);
        // SAFETY: the run `take` borrowed, no slot of it out.
        unsafe { free((run.at, RUN_BYTES)) };
    }
}

/// Moves whenever a slot is given back, as the heap's count moves for its
/// blocks: a body refused room may be served once either has.
pub fn releases() -> u32 {
    held().releases
}

#[cfg(test)]
#[path = "body.test.rs"]
mod test;
