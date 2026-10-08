// xpute-guest/ipc/door.rs

//! The guest's end of its rings: submissions applied in order, completions
//! posted in place. The host reads the completion ring dry before it rings
//! again, so overrunning it is a sizing bug, not a state to wait out.

use xpute_kit::status::errno::Errno;
use xpute_kit::status::error::MarshalError;
use xpute_kit::wire::xtp::PacketWriter;

use crate::ipc::frame;
use crate::ipc::ring::Ring;

pub struct RingLayout {
    pub capacity: u32,
    pub desc_at: u32,
    pub slot_at: u32,
}

pub struct DoorLayout {
    pub slot_bytes: u32,
    pub submission: RingLayout,
    pub completion: RingLayout,
}

/// Answers the packet's length, or none when it did not fit a slot.
pub type Write<'a> = &'a dyn Fn(&mut [u8]) -> Option<u32>;

/// None for an unknown command, which replies ENOSYS.
pub type Applied = Result<Option<i32>, MarshalError>;

pub struct Door {
    sq: Ring,
    cq: Ring,
    /// What one command may leave: its signals and its reply.
    reserve: u32,
    /// Told the completion ring's length and the submissions left unapplied.
    note: fn(u32, u32),
}

impl Door {
    /// # Safety
    /// `mem` stays valid and never moves for as long as the door is used, and
    /// the ranges `layout` names are the door's alone. `most_signals` is what
    /// one command may raise, the program's bound.
    pub unsafe fn init(mem: *mut u8, layout: &DoorLayout, most_signals: u32, note: fn(u32, u32)) -> Door {
        let ring = |r: &RingLayout| {
            // SAFETY: the caller's memory and ranges.
            unsafe { Ring::init(mem, r.desc_at, r.capacity, r.slot_at, layout.slot_bytes) }
        };
        Door {
            sq: ring(&layout.submission),
            cq: ring(&layout.completion),
            reserve: most_signals + 1,
            note,
        }
    }

    pub fn submission(&self) -> &Ring {
        &self.sq
    }

    pub fn completion(&self) -> &Ring {
        &self.cq
    }

    pub fn post(&self, tag: u32, cmd: u32, flags: u32, result: i32, write: Option<Write<'_>>) {
        let e = match write {
            Some(w) => self.cq.push_in_place(tag, cmd, flags, result, w),
            None => self.cq.push(tag, cmd, flags, None, result),
        };
        (self.note)(self.cq.len(), 0);
        xpute_kit::ensure!(e == Errno::OK, ENOSPC, cmd, e as i32);
    }

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

    /// An unapplied command can wait in the submission ring; what it would
    /// raise could not wait anywhere, so room is checked before it starts.
    fn has_room(&self) -> bool {
        self.cq.len() + self.reserve <= self.cq.capacity
    }

    pub fn pending(&self) -> bool {
        self.sq.peek() >= 0
    }

    /// Applies submissions in order while there is room; the rest waits for
    /// the next turn. Replies only to `ACKREQ` and to failures. Returns how
    /// many completions it made.
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
            if flags & frame::ACKREQ != 0 || result.is_err() {
                self.post(tag, number, frame::RES, result.unwrap_or_else(|errno| errno), None);
            }
        }
        (self.note)(self.cq.len(), self.sq.len());
        self.cq.tail().wrapping_sub(tail)
    }
}

#[cfg(test)]
#[path = "door.test.rs"]
mod test;
