// xpute-runtime/ipc/doorbell.rs

//! The guest's end of its rings (the host's is ipc/doorbell.ts): what waits in
//! the submission ring applied in order, and what the guest makes posted into
//! the completion ring. What a command does is the guest's, handed to
//! `drain`; the order, the replies and the backpressure are this.
//!
//! The completion ring holds everything one turn leaves: the host reads it
//! dry before it rings again (the contract's rule 5, ARCHITECTURE.xpute.md),
//! so a turn starts on an empty ring and one that would overrun it has left
//! more than it was sized for — a sizing fault, as a full pool is, and not a state to
//! wait out. Each message is written once, in place, and nothing is held on
//! a heap.

use xpute_core::status::errno::Errno;
use xpute_core::status::error::MarshalError;
use xpute_core::wire::xtp::PacketWriter;

use crate::ipc::frame::FrameFlag;
use crate::ipc::ring::Ring;

/// Where one ring lies in the memory: how many messages it holds, its
/// descriptors, and its slots' payloads.
pub struct RingLayout {
    pub capacity: u32,
    pub desc_at: u32,
    pub slot_at: u32,
}

/// Where a door's two rings lie, and how large a slot's payload is in both.
pub struct DoorLayout {
    pub slot_bytes: u32,
    pub submission: RingLayout,
    pub completion: RingLayout,
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

/// What one command may leave in the completion ring: its signals, and its
/// reply.
const MOST_COMPLETIONS: u32 = MOST_SIGNALS + 1;

pub struct Door {
    sq: Ring,
    cq: Ring,
    /// Told the completion ring's length at every post, and how many
    /// submissions a drain left unapplied, for whoever measures how full the
    /// rings run.
    note: fn(u32, u32),
}

impl Door {
    /// Lays out two empty rings over the memory at `mem`, where `layout`
    /// says.
    ///
    /// # Safety
    /// `mem` stays valid and never moves for as long as the door is used, and
    /// the ranges `layout` names are the door's alone.
    pub unsafe fn init(mem: *mut u8, layout: &DoorLayout, note: fn(u32, u32)) -> Door {
        let ring = |r: &RingLayout| {
            // SAFETY: the caller's memory and ranges.
            unsafe { Ring::init(mem, r.desc_at, r.capacity, r.slot_at, layout.slot_bytes) }
        };
        Door {
            sq: ring(&layout.submission),
            cq: ring(&layout.completion),
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

    /// Posts a message into the completion ring.
    pub fn post(&self, tag: u32, cmd: u32, flags: u32, result: i32, write: Option<Write<'_>>) {
        let e = match write {
            Some(w) => self.cq.push_in_place(tag, cmd, flags, result, w),
            None => self.cq.push(tag, cmd, flags, None, result),
        };
        (self.note)(self.cq.len(), 0);
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

    /// Whether another command's signals and reply have somewhere to go. What
    /// `drain` stops on: a command that has not been applied can be left in
    /// the submission ring for the next turn, where what it would have raised
    /// cannot be left anywhere. The receiver's room bounds what the sender
    /// may start, as a credit-based link does, so nothing is refused for want
    /// of it once started — io_uring answers a submission `-EBUSY` while its
    /// completions back up, for the same reason.
    fn has_room(&self) -> bool {
        self.cq.len() + MOST_COMPLETIONS <= self.cq.capacity
    }

    /// Whether a submission waits that `drain` left for a later turn: the
    /// guest asks for the next one while it does.
    pub fn pending(&self) -> bool {
        self.sq.peek() >= 0
    }

    /// Applies every submission waiting, each by `apply`: its number and its
    /// packet, while the completion ring has room for what one more might
    /// raise; the rest waits in the submission ring, in order, for the next
    /// turn (`pending`) — nothing is dropped, and nothing is applied out of
    /// the order it was sent, which is what lets a read see every write
    /// before it. Replies to what asks (`ACKREQ`), and to what failed — a
    /// command nobody numbers, a packet that does not read — since a
    /// command's effect is otherwise its signals. How many completions the
    /// call made.
    pub fn drain(&self, mut apply: impl FnMut(u32, &Option<&[u8]>) -> Applied) -> u32 {
        let tail = self.cq.tail();
        loop {
            if !self.has_room() {
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
        (self.note)(self.cq.len(), self.sq.len());
        self.cq.tail().wrapping_sub(tail)
    }
}

#[cfg(test)]
#[path = "doorbell.test.rs"]
mod test;
