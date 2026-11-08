// pxl-core/sys/io/room.rs

//! A refusal of room, by the heap or by the body slots (sys/io/body.rs). It
//! records how many times what refused it had given room back, and it is
//! over once that count moves. A refused request waits only on what refused
//! it, never on the other. A slot given back returns nothing to the heap
//! until its whole run is empty, and only then does the heap's count move;
//! memory the heap gets back frees no slot. A body refused a new run of slots
//! waits on both counts, because a slot given back in a run already held
//! serves it too.

use crate::sys::boot::heap_stat;
use crate::sys::io::body;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Refusal(Room, u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Room {
    Heap,
    Bodies,
    Run,
}

impl Room {
    fn release_cnt(self) -> u32 {
        match self {
            Room::Heap => heap_stat().releases,
            Room::Bodies => body::releases(),
            Room::Run => heap_stat().releases.wrapping_add(body::releases()),
        }
    }
}

impl Refusal {
    /// A refusal by the heap.
    pub fn now() -> Refusal {
        Refusal(Room::Heap, Room::Heap.release_cnt())
    }

    /// A refusal by the body slots, when every slot is out.
    pub fn bodies() -> Refusal {
        Refusal(Room::Bodies, Room::Bodies.release_cnt())
    }

    /// A refusal by the heap of a new run of body slots, when every run held
    /// is full.
    pub fn run() -> Refusal {
        Refusal(Room::Run, Room::Run.release_cnt())
    }

    /// Whether room has been given back since the refusal, so that asking
    /// again may be served.
    pub fn over(self) -> bool {
        self.0.release_cnt() != self.1
    }
}
