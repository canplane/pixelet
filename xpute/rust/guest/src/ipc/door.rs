// xpute-guest/ipc/door.rs

//! The guest's end of its rings: submissions applied in order, completions
//! posted in place. The host reads the completion ring dry before it rings
//! again, so overrunning it is a sizing bug, not a state to wait out.

use core::cell::Cell;

use xpute_kit::status::errno::Errno;
use xpute_kit::status::error::MarshalError;
use xpute_kit::wire::xtp::PacketWriter;

use crate::ipc::frame;
use crate::ipc::ring::Ring;

/// None for an unknown command, which replies ENOSYS.
pub type Applied = Result<Option<i32>, MarshalError>;

/// How full the rings ran since the stat was last taken (`Door::take_stat`):
/// each figure the most it reached.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DoorStat {
    /// The completion ring's length.
    pub cq_peak: u32,
    /// The submission ring's length as a drain started: how close it came to full.
    pub sq_peak: u32,
    /// The submissions a drain left for lack of room.
    pub left_peak: u32,
}

pub struct Door {
    sq: Ring,
    cq: Ring,
    /// What one command may leave: its signals, or the turn's after the drain, and its reply.
    reserve: u32,
    stat: Cell<DoorStat>,
}

impl Door {
    /// Over the two rings whoever laid them out built, the door's alone.
    /// `most_signals` is the most one command raises, or the turn once its
    /// drain is done: the program's bound.
    pub fn new(submission: Ring, completion: Ring, most_signals: u32) -> Door {
        Door {
            sq: submission,
            cq: completion,
            reserve: most_signals + 1,
            stat: Cell::default(),
        }
    }

    /// The peaks since the last take, which starts the next from zero.
    pub fn take_stat(&self) -> DoorStat {
        self.stat.take()
    }

    fn keep(&self, f: impl FnOnce(&mut DoorStat)) {
        let mut s = self.stat.get();
        f(&mut s);
        self.stat.set(s);
    }

    pub fn submission(&self) -> &Ring {
        &self.sq
    }

    pub fn completion(&self) -> &Ring {
        &self.cq
    }

    pub fn post(&self, tag: u32, cmd: u32, flags: u32, result: i32) {
        self.posted(cmd, self.cq.push(tag, cmd, flags, result, None));
    }

    /// `write` fills the slot's payload and answers the packet's length, or
    /// none when it did not fit.
    pub fn post_in_place(&self, tag: u32, cmd: u32, flags: u32, result: i32, write: impl FnOnce(&mut [u8]) -> Option<u32>) {
        self.posted(cmd, self.cq.push_in_place(tag, cmd, flags, result, write));
    }

    fn posted(&self, cmd: u32, e: Errno) {
        self.keep(|s| s.cq_peak = s.cq_peak.max(self.cq.len()));
        xpute_kit::ensure!(e == Errno::OK, ENOSPC, cmd, e as i32);
    }

    pub fn raise(&self, cmd: u32, count: u32, f: impl Fn(&mut PacketWriter)) {
        self.post_in_place(0, cmd, 0, 0, |out: &mut [u8]| {
            let mut w = PacketWriter::new(out, count);
            f(&mut w);
            w.fitted()
        });
    }

    /// An unapplied command can wait in the submission ring; what it would
    /// raise could not wait anywhere, so room is checked before it starts.
    fn has_room(&self) -> bool {
        self.cq.len() + self.reserve <= self.cq.capacity
    }

    pub fn pending(&self) -> bool {
        !self.sq.is_empty()
    }

    /// Applies submissions in order while there is room; the rest waits for
    /// the next turn. Replies only to `ACKREQ` and to failures. Returns how
    /// many completions it made.
    pub fn drain(&self, mut apply: impl FnMut(u32, &Option<&[u8]>) -> Applied) -> u32 {
        let tail = self.cq.tail();
        self.keep(|s| s.sq_peak = s.sq_peak.max(self.sq.len()));
        while self.has_room() {
            let Some(f) = self.sq.peek() else {
                break;
            };
            let result = match self.sq.packet(&f) {
                Ok(pkt) => match apply(f.cmd, &pkt) {
                    Ok(Some(value)) => Ok(value),
                    Ok(None) => Err(Errno::ENOSYS as i32),
                    Err(e) => Err(e.errno as i32),
                },
                Err(e) => Err(e.errno as i32),
            };
            self.sq.advance();
            if f.flags & frame::ACKREQ != 0 || result.is_err() {
                self.post(f.tag, f.cmd, frame::RES, result.unwrap_or_else(|errno| errno));
            }
        }
        self.keep(|s| s.left_peak = s.left_peak.max(self.sq.len()));
        self.cq.tail().wrapping_sub(tail)
    }
}

#[cfg(test)]
#[path = "door.test.rs"]
mod test;
