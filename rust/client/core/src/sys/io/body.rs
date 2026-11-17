// pxl-core/sys/io/body.rs

//! The memory a body is read into. Terrain's and the Earth's bodies, the
//! largest flow, take slots in `.fixed`: `BODY_SLOT_CNT` of them (sys/io/param.rs),
//! each as long as the longest such body, so that flow's memory is fixed and
//! takes nothing of the heap. Every other stream's body is a heap buffer of
//! its own.
//!
//! A slot is held by its guard, `Slot`, from `take` until the guard drops. A
//! read lends it (xpute-link `Lend`) and the read's answer hands it
//! back, so a read stopped mid-way still holds its slot until the host
//! has answered. A layer holds a body until it is decoded; an Earth body
//! until a drawn frame takes its texels (domain/layer/earth.rs `upload`). With
//! every slot out a body waits for one given back (sys/io/room.rs
//! `Refusal::bodies`), and the next slot given back asks for a frame, in which
//! the want is asked again.

use crate::spec::step::InvalidationReason;
use core::ops::{Deref, DerefMut};

use pxl_global::Global;
use xpute_link::abi::types::Lent;
use xpute_link::mem;

use crate::sys::heap::PAGE_SZ;
use crate::sys::io::demand::Stream;
use crate::sys::io::param::BODY_SLOT_CNT;
use crate::sys::range::OwnedRange;
use crate::sys::sched;

/// The longest body of a stream whose bodies take a slot.
const fn longest() -> u32 {
    let (mut most, mut k) = (0, 0);
    while k < Stream::ALL.len() {
        let s = Stream::ALL[k];
        if let (true, Some(max)) = (s.row().slotted, s.max_body())
            && max > most
        {
            most = max;
        }
        k += 1;
    }
    most
}

/// A slot's bytes: the longest such body, on a page.
pub(crate) const SLOT_SZ: u32 = longest().next_multiple_of(PAGE_SZ);

/// The slots' range in `.fixed`, which the boot hands in.
pub(crate) static RANGE: OwnedRange = OwnedRange::new();

const _: () = assert!(BODY_SLOT_CNT <= u32::BITS, "a slot is a bit of `out`");

#[derive(Default)]
struct Slots {
    /// One bit for each slot, set while it is out.
    out: u32,
    /// A count that moves with every slot given back. A refusal by the slots
    /// waits on it (`Refusal::bodies`).
    release_cnt: u32,
    /// The most slots out at once since the profiler last took the peak.
    peak: u32,
    /// A take found every slot out, so the next slot given back asks for a
    /// frame.
    waited: bool,
}

static SLOTS: Global<Slots> = Global::new();

fn slots() -> &'static mut Slots {
    SLOTS.get(Slots::default)
}

/// A slot out, and the bytes of it a body takes.
pub(crate) struct Slot {
    idx: u32,
    len: u32,
}

/// A free slot, or None with every slot out.
pub(crate) fn take() -> Option<Slot> {
    let s = slots();
    let idx = (!s.out).trailing_zeros();
    if idx >= BODY_SLOT_CNT {
        s.waited = true;
        return None;
    }
    s.out |= 1 << idx;
    s.peak = s.peak.max(s.out.count_ones());
    Some(Slot { idx, len: 0 })
}

impl Slot {
    /// Sets the bytes of the slot the body takes; more than the slot is a bug.
    pub(crate) fn set_len(&mut self, len: u32) {
        xpute_kit::ensure!(len <= SLOT_SZ, EFBIG, len);
        self.len = len;
    }

    fn bytes(&self) -> &'static mut [u8] {
        // SAFETY: the slot is this guard's alone while it lives, and its
        // bytes lie in `.fixed`.
        unsafe { RANGE.arr::<u8>(self.idx * SLOT_SZ, self.len as usize) }
    }
}

impl Deref for Slot {
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        self.bytes()
    }
}

impl DerefMut for Slot {
    fn deref_mut(&mut self) -> &mut [u8] {
        self.bytes()
    }
}

impl xpute_link::Lend for Slot {
    fn lent(&mut self) -> Lent {
        mem::lent(self.bytes())
    }
}

impl Drop for Slot {
    fn drop(&mut self) {
        let s = slots();
        s.out &= !(1 << self.idx);
        s.release_cnt = s.release_cnt.wrapping_add(1);
        if core::mem::take(&mut s.waited) {
            sched::invalidate(InvalidationReason::Completion);
        }
    }
}

/// A body: in a slot, or a heap buffer of its own.
pub(crate) enum Body {
    Slot(Slot),
    Heap(Box<[u8]>),
}

impl Deref for Body {
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        match self {
            Body::Slot(s) => s,
            Body::Heap(b) => b,
        }
    }
}

pub(crate) fn release_cnt() -> u32 {
    slots().release_cnt
}

/// The most slots out at once since the last call. The profiler takes it
/// (perf/report.rs `perf_frame`), and `BODY_SLOT_CNT` is judged against it.
pub(crate) fn take_peak() -> u32 {
    let s = slots();
    core::mem::replace(&mut s.peak, s.out.count_ones())
}
