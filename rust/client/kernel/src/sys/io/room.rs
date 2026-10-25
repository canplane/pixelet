// pxl-kernel/sys/io/room.rs

//! A request refused room, by the heap or by the body slots (sys/io/body.rs),
//! as what refused it counted its releases then: it waits until that gives
//! some room back, never on a clock, since only a release can change the
//! answer, and not on the other. A slot given back gives the heap nothing
//! until its run is empty, which the heap's count then shows; the heap's
//! pages given back give no slot. A body refused a run of slots waits on
//! both, since a slot given back in a run held serves it as well.

use crate::sys::boot::heap_stat;
use crate::sys::io::body;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Refused(Room, u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Room {
    Heap,
    Bodies,
    /// A run of slots: the heap's or a slot.
    Run,
}

impl Room {
    fn released(self) -> u32 {
        match self {
            Room::Heap => heap_stat().releases,
            Room::Bodies => body::releases(),
            Room::Run => heap_stat().releases.wrapping_add(body::releases()),
        }
    }
}

impl Refused {
    /// By the heap.
    pub fn now() -> Refused {
        Refused(Room::Heap, Room::Heap.released())
    }

    /// By the body slots, every one out.
    pub fn bodies() -> Refused {
        Refused(Room::Bodies, Room::Bodies.released())
    }

    /// By the heap, for a run of body slots, every run held full.
    pub fn run() -> Refused {
        Refused(Room::Run, Room::Run.released())
    }

    /// Whether room has been given back since, so asking again may be served.
    pub fn over(self) -> bool {
        self.0.released() != self.1
    }
}
