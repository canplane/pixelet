// pxl-kernel/io/body.rs

//! The blocks bodies are read into. Terrain's and the Earth's packets run
//! just past a mebibyte (a tile's looks and the Earth's texels are one
//! each, pxl-spec `bake::tile::max_packet_bytes`), where the buddy would
//! give each body a block of two. Their bodies take a slot of
//! `.fixed.bodies` instead, each their longest packet in pages, as many as
//! are typically out at once (`SLOTS`): with every slot out, a body is
//! refused (`Refused::bodies`), and its want waits until a slot is given
//! back and asks again (io/request.rs `Wait::Room`), losing nothing. Every
//! other body is a heap block of its own: a place's or a division's is at
//! most a power of two the buddy gives whole, and freed as it is decoded.
//!
//! The slots never all wait on one another. A body is out from its arrival
//! until its decode (view/demand.rs `admit_arrived`), which needs no other
//! body, and then is kept or given back (layer.rs `Base::keep`). An Earth
//! packet is kept until its texels are taken (layer/earth.rs
//! `earth_upload`), which needs nothing but the frame. A terrain packet is
//! kept while a window wants one of its tile's squares still absent
//! (layer/terrain/window.rs `fill`): each frame hands the fill up to
//! `HANDED` squares (`due`), only of tiles whose packet is kept or that are
//! void, never of one waiting on a body, and a square handed is no longer
//! wanted, its ticket set; so the kept packets' squares go out over frames
//! from what is already held, and once none of a tile's is wanted, the next
//! frame's upload gives its packet back (`Offered::Taken`). A packet whose
//! tile left the residency, or that no window wants at all, is given back
//! on that upload too (scene/cast.rs `upload`). Only a frame not drawn
//! holds them, and the frames drawn again release them.

use xpute_guest::global::Global;
use xpute_guest::mem::heap::{alloc, free, Block};
use xpute_guest::mem::section::{Section, PAGE_BYTES};
use xpute_kit::status::bug::OrBug;
use xpute_kit::status::errno::Errno;

use crate::io::demand::Stream;
use crate::io::host::Refused;
use crate::param::{KEPT_BODIES, MAX_CREDITS};

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

/// A body on every credit, each out until its decode, and the packets kept
/// past it (param/io.rs `KEPT_BODIES`, which says how many and why).
const SLOTS: u32 = MAX_CREDITS + KEPT_BODIES;

/// `.fixed.bodies` (door/layout.rs).
pub const BODIES_BYTES: u32 = SLOTS * SLOT_BYTES;

const _: () = {
    assert!(SLOTS >= 1 && SLOTS <= u64::BITS, "a slot is a bit of `Slots::out`");
    let mut k = 0;
    while k < SLOTTED.len() {
        assert!(
            longest(SLOTTED[k]) <= SLOT_BYTES && SLOT_BYTES < longest(SLOTTED[k]).next_power_of_two(),
            "a slot costs less than the buddy's block for its body"
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

#[derive(Default)]
struct Slots {
    /// `.fixed.bodies`' first byte, handed in at boot (`place`).
    at: Option<usize>,
    /// A bit a slot out.
    out: u64,
    releases: u32,
    /// The most slots out at once since the profiler last took it.
    peak: u32,
}

static HELD: Global<Slots> = Global::new();

fn held() -> &'static mut Slots {
    HELD.get(Slots::default)
}

/// `.fixed.bodies`, once at boot.
pub fn place(range: Section) {
    xpute_kit::ensure!(range.bytes >= BODIES_BYTES, ENOTRECOVERABLE, range.bytes);
    held().at = Some(range.offset as usize);
}

/// A block for a body of `n` bytes, of `stream` where the request
/// scheduler still holds it; refused where no slot or no heap was free.
pub fn take(stream: Option<Stream>, n: u32) -> Result<Block, Refused> {
    if !stream.is_some_and(slotted) {
        return alloc(n).ok_or_else(Refused::now);
    }
    // A body past its kind's longest is refused before it is read (io/fetch.rs `opened`).
    xpute_kit::ensure!(n <= SLOT_BYTES, ENOTRECOVERABLE, n);
    let s = held();
    let at = s.at.or_bug(Errno::ENOTRECOVERABLE);
    let slot = (!s.out).trailing_zeros();
    if slot >= SLOTS {
        return Err(Refused::bodies());
    }
    s.out |= 1 << slot;
    s.peak = s.peak.max(s.out.count_ones());
    Ok((at + (slot * SLOT_BYTES) as usize, n))
}

/// # Safety
/// `block` came from `take` and has not been given back.
pub unsafe fn give(block: Block) {
    let s = held();
    let Some(slot) = s.at.and_then(|at| block.0.checked_sub(at)).filter(|&off| off < BODIES_BYTES as usize) else {
        // SAFETY: the caller's; a block outside the slots is the heap's own.
        return unsafe { free(block) };
    };
    s.out &= !(1 << (slot / SLOT_BYTES as usize));
    s.releases = s.releases.wrapping_add(1);
}

/// Moves whenever a slot is given back: a body refused one may be served
/// once it has.
pub fn releases() -> u32 {
    held().releases
}

/// The most slots out at once since the last call, the profiler's
/// (profiler/report.rs `profiler_frame`): what `KEPT_BODIES` is measured by.
pub fn take_peak() -> u32 {
    let s = held();
    core::mem::replace(&mut s.peak, s.out.count_ones())
}

#[cfg(test)]
#[path = "body.test.rs"]
mod test;
