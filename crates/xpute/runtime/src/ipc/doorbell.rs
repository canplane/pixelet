// xpute-runtime/ipc/doorbell.rs

//! The guest's end of its rings (the host's is ipc/doorbell.ts): what waits in
//! the submission ring applied in order, and what the guest makes posted into
//! the completion ring — or into a spill behind it while the host has not
//! read, so what was raised first is read first and nothing is dropped. What
//! a command does is the guest's, handed to `drain`; the order, the replies
//! and the backpressure are this.
//!
//! Each message is written once, in place, and nothing is held on a heap. A
//! spill that fills as well is a sizing fault, as a full pool is.

use xpute_core::status::bug::OrBug;
use xpute_core::status::errno::Errno;
use xpute_core::status::error::MarshalError;
use xpute_core::wire::xtp::PacketWriter;

use crate::ipc::frame::FrameFlag;
use crate::ipc::ring::Ring;

/// Where a door's three rings lie in the memory: each ring's descriptors at
/// `desc_at` and a stride apart, its slots' payloads at `slot_at` and a stride
/// apart, submission first, then completion, then the spill.
pub struct DoorLayout {
    pub capacity: u32,
    pub slot_bytes: u32,
    pub desc_at: u32,
    pub desc_stride: u32,
    pub slot_at: u32,
    pub slot_stride: u32,
}

/// A packet's bytes written in place into a ring slot: its length, or none
/// when it did not fit one.
pub type Write<'a> = &'a dyn Fn(&mut [u8]) -> Option<u32>;

/// A command applied: its reply's value, or none for a command the guest does
/// not number, which replies ENOSYS; a packet that does not read replies with
/// its errno.
pub type Applied = Result<Option<i32>, MarshalError>;

/// Signals one command may raise before `drain` checks again. A frame is the
/// many-signaled one — its sensor asks, snapshot, fetches and loads —
/// so this bounds a single command rather than a turn.
const MOST_SIGNALS: u32 = 64;

pub struct Door {
    sq: Ring,
    cq: Ring,
    spill: Ring,
    /// Told the completion ring's and the spill's lengths at every post, for
    /// whoever measures how full they run.
    note: fn(u32, u32),
}

impl Door {
    /// Lays out three empty rings over the memory at `mem`, where `layout`
    /// says.
    ///
    /// # Safety
    /// `mem` stays valid and never moves for as long as the door is used, and
    /// the ranges `layout` names are the door's alone.
    pub unsafe fn init(mem: *mut u8, layout: &DoorLayout, note: fn(u32, u32)) -> Door {
        let ring = |n: u32| {
            // SAFETY: the caller's memory and ranges.
            unsafe {
                Ring::init(
                    mem,
                    layout.desc_at + n * layout.desc_stride,
                    layout.capacity,
                    layout.slot_at + n * layout.slot_stride,
                    layout.slot_bytes,
                )
            }
        };
        Door {
            sq: ring(0),
            cq: ring(1),
            spill: ring(2),
            note,
        }
    }

    /// The submission ring, which the host pushes into.
    pub fn submission(&self) -> &Ring {
        &self.sq
    }

    /// The completion ring, which the host reads dry.
    pub fn completion(&self) -> &Ring {
        &self.cq
    }

    /// Posts a message: the completion ring while it has room and nothing
    /// waits before it, otherwise the spill behind it.
    pub fn post(&self, tag: u32, cmd: u32, flags: u32, result: i32, write: Option<Write<'_>>) {
        let push = |r: &Ring| match write {
            Some(w) => r.push_in_place(tag, cmd, flags, result, w),
            None => r.push(tag, cmd, flags, None, result),
        };
        if self.spill.is_empty() && push(&self.cq) == Errno::OK {
            (self.note)(self.cq.len(), 0);
            return;
        }
        let e = push(&self.spill);
        (self.note)(self.cq.len(), self.spill.len());
        xpute_core::ensure!(e == Errno::OK, ENOSPC, cmd, e as i32);
    }

    /// A signal for the host with a packet of `count` children written by `f`.
    pub fn raise(&self, cmd: u32, count: u32, f: impl Fn(&mut PacketWriter)) {
        self.post(
            0,
            cmd,
            0,
            0,
            Some(&|out: &mut [u8]| {
                let mut w = PacketWriter::new(out, count);
                f(&mut w);
                w.finish()
            }),
        );
    }

    /// Whether another command's worth of signals has somewhere to go. What
    /// `drain` stops on: a command that has not been applied can be left in
    /// the submission ring for the next call, where what it would have raised
    /// cannot be left anywhere.
    ///
    /// The margin is the spill rather than the completion ring, so the host
    /// is asked to read before anything is at risk — io_uring answers a
    /// submission `-EBUSY` while its backlog stands, for the same reason.
    fn has_room(&self) -> bool {
        self.spill.len() + MOST_SIGNALS <= self.spill.capacity
    }

    /// Moves what fits of the spill into the completion ring, in order.
    fn flush(&self) {
        loop {
            let at = self.spill.peek();
            if at < 0 {
                return;
            }
            let at = at as u32;
            let pkt = self.spill.packet(at).or_bug(Errno::ENOTRECOVERABLE);
            if self.cq.push(self.spill.tag(at), self.spill.cmd(at), self.spill.flags(at), pkt, self.spill.result(at)) != Errno::OK {
                return;
            }
            self.spill.advance();
        }
    }

    /// Applies every submission waiting, each by `apply`: its number and its
    /// packet. What they make goes into the completion ring as far as it has
    /// room; the rest waits in the guest, in order, for the next call —
    /// nothing is dropped, and nothing sent waits to be applied, which is
    /// what lets a read see every write before it. Replies to what asks
    /// (`ACKREQ`), and to what failed — a command nobody numbers, a packet
    /// that does not read — since a command's effect is otherwise its
    /// signals. How many completions the call made.
    pub fn drain(&self, mut apply: impl FnMut(u32, &Option<&[u8]>) -> Applied) -> u32 {
        let tail = self.cq.tail();
        self.flush();
        loop {
            if !self.has_room() {
                // Backpressure rather than a panic: what is left stays in the
                // submission ring, in order, and the host reads the
                // completions before it is drained again.
                break;
            }
            let at = self.sq.peek();
            if at < 0 {
                break;
            }
            let at = at as u32;
            let (tag, number, flags) = (self.sq.tag(at), self.sq.cmd(at), self.sq.flags(at));
            let result = match self.sq.packet(at) {
                Ok(pkt) => match apply(number, &pkt) {
                    Ok(Some(value)) => Ok(value),
                    Ok(None) => Err(Errno::ENOSYS as i32),
                    Err(e) => Err(e.errno as i32),
                },
                Err(e) => Err(e.errno as i32),
            };
            self.sq.advance();
            if flags & FrameFlag::ACKREQ as u32 != 0 || result.is_err() {
                self.post(tag, number, FrameFlag::RES as u32, result.unwrap_or_else(|errno| errno), None);
            }
        }
        self.cq.tail().wrapping_sub(tail)
    }
}
