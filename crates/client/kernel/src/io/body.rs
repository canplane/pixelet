// pxl-kernel/io/body.rs

//! The blocks bodies are read into. Terrain's and the Earth's packets run
//! just past a mebibyte (a tile's looks and the Earth's texels are one
//! each, pxl-spec `bake::tile::max_packet_bytes`), where the buddy would
//! give each body a block of two. Their bodies take slots instead, each
//! their longest packet in pages, carved from a run the heap gives whole
//! and takes back once its last slot is free. A run is borrowed only when
//! every one held is full, so no more are held than the most slots out at
//! once fill, and never more slots than `BODY_SLOTS` (param/io.rs), which
//! bounds what the heap gives them: the request scheduler starts a terrain
//! or Earth read only while the slots out and those reads out leave it one
//! (io/request.rs `pump`, `room` here), so a body is not refused a slot
//! after its bytes came, save as a fallback that waits as the heap's
//! refusal does. Every other body is a heap block of its own: a place's or
//! a division's is at most a power of two the buddy gives whole, and freed
//! as it is decoded.
//!
//! The slots never stop the reads, drawn or not. A body on a credit needs
//! no other to be decoded (view/demand.rs `admit_arrived`, one a pass at
//! least), and is then freed or kept. Kept packets are given back only by
//! a drawn frame (scene/cast.rs `upload`, layer/earth.rs `earth_upload`),
//! so they are bounded apart from it: the Earth's by its frames, terrain's
//! by the slots less one for every credit granted and every Earth frame
//! (layer/terrain/tile.rs `most_kept`), a packet past it freed once
//! decoded. Kept packets thus leave a slot for every credit, and since a
//! read out holds one slot at most, counted twice at worst, a read can
//! start whenever fewer than half the credits are out: the residency,
//! which the query and the controls read, fills with no frame drawn. In a
//! drawn frame a terrain packet is given back once no window wants one of
//! its squares absent (layer/terrain/window.rs `fill`): each frame hands
//! `HANDED` squares only of tiles kept or void (`due`), a square handed is
//! wanted no more, and a packet whose tile left or that no window wants is
//! given back on that upload too, the stream full or not (layer/kept.rs
//! `send`). A tile whose packet was freed is asked
//! again on the step after a packet is given back, while one could be kept
//! (`step`), so nothing is fetched only to be freed while nothing is drawn.

use xpute_guest::global::Global;
use xpute_guest::mem::heap::{alloc, free, Block};
use xpute_guest::mem::section::PAGE_BYTES;

use crate::io::demand::Stream;
use crate::io::host::Refused;
use crate::param::{BODY_RUN_BYTES, BODY_SLOTS};

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

/// Seven at today's packets.
const SLOTS: u32 = BODY_RUN_BYTES / SLOT_BYTES;

const FULL: u32 = if SLOTS == u32::BITS { u32::MAX } else { (1 << SLOTS) - 1 };

const _: () = {
    assert!(BODY_RUN_BYTES.is_power_of_two(), "a run is the buddy's own block");
    assert!(SLOTS >= 2 && SLOTS <= u32::BITS, "a run holds two slots or more, a bit each");
    let mut k = 0;
    while k < SLOTTED.len() {
        assert!(
            BODY_RUN_BYTES / SLOTS < longest(SLOTTED[k]).next_power_of_two(),
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
    slots.div_ceil(SLOTS) as u64 * BODY_RUN_BYTES as u64
}

struct Run {
    at: usize,
    /// A bit a slot out.
    out: u32,
}

#[derive(Default)]
struct Slots {
    runs: Vec<Run>,
    /// Slots out, across every run.
    out: u32,
    releases: u32,
    /// The most slots out at once since the profiler last took it.
    peak: u32,
    /// `room` was asked, so a read may be waiting on a slot.
    asked: bool,
}

static HELD: Global<Slots> = Global::new();

fn held() -> &'static mut Slots {
    HELD.get(Slots::default)
}

/// A block for a body of `n` bytes, of `stream` where the request
/// scheduler still holds it; refused where the slots or the heap had no
/// room.
pub fn take(stream: Option<Stream>, n: u32) -> Result<Block, Refused> {
    if !stream.is_some_and(slotted) || n > SLOT_BYTES {
        return alloc(n).ok_or_else(Refused::now);
    }
    let s = held();
    // Never where the scheduler kept to `room`.
    if s.out >= BODY_SLOTS {
        return Err(Refused::now());
    }
    let i = match s.runs.iter().position(|r| r.out != FULL) {
        Some(i) => i,
        None => {
            let (at, _) = alloc(BODY_RUN_BYTES).ok_or_else(Refused::now)?;
            s.runs.push(Run { at, out: 0 });
            s.runs.len() - 1
        }
    };
    let run = &mut s.runs[i];
    let slot = (!run.out).trailing_zeros();
    run.out |= 1 << slot;
    s.out += 1;
    s.peak = s.peak.max(s.out);
    Ok((run.at + (slot * SLOT_BYTES) as usize, n))
}

/// # Safety
/// `block` came from `take` and has not been given back.
pub unsafe fn give(block: Block) {
    let s = held();
    let Some(i) = s.runs.iter().position(|r| (r.at..r.at + BODY_RUN_BYTES as usize).contains(&block.0)) else {
        // SAFETY: the caller's; a block outside every run is the heap's own.
        return unsafe { free(block) };
    };
    let run = &mut s.runs[i];
    let off = block.0 - run.at;
    let bit = 1 << (off / SLOT_BYTES as usize);
    // A block inside a run that is not a slot out is given back twice, or was never taken.
    xpute_kit::ensure!(off.is_multiple_of(SLOT_BYTES as usize) && run.out & bit != 0, ENOTRECOVERABLE, off);
    run.out &= !bit;
    s.out -= 1;
    s.releases = s.releases.wrapping_add(1);
    // A read held for a slot starts on the turn after one is given back.
    if core::mem::take(&mut s.asked) {
        crate::sched::edge::request_world_tick();
    }
    if run.out == 0 {
        let run = s.runs.swap_remove(i);
        // SAFETY: the run `take` borrowed, no slot of it out.
        unsafe { free((run.at, BODY_RUN_BYTES)) };
    }
}

/// The slots a read may still take, for the request scheduler, which asks
/// only where a terrain or Earth read is ready.
pub fn room() -> u32 {
    let s = held();
    s.asked = true;
    BODY_SLOTS.saturating_sub(s.out)
}

/// Moves whenever a slot is given back, as the heap's count moves for its
/// blocks: a body refused room may be served once either has.
pub fn releases() -> u32 {
    held().releases
}

/// The most slots out at once since the last call, the profiler's
/// (profiler/report.rs `profiler_frame`): what `BODY_SLOTS` is measured by.
pub fn take_peak() -> u32 {
    let s = held();
    core::mem::replace(&mut s.peak, s.out)
}

#[cfg(test)]
#[path = "body.test.rs"]
mod test;
