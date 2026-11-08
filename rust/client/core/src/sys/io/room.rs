// pxl-core/sys/io/room.rs

//! A request refused room, by the heap or by the body slots (sys/io/body.rs),
//! as what refused it counted its releases then: it waits until that gives
//! some back, not on the other. A slot given back gives the heap nothing
//! until its run is empty, which the heap's count then shows, and the heap's
//! pages give no slot; a body refused a run of slots waits on both, as a
//! slot given back in a run held serves it too.

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
    /// By the heap.
    pub fn now() -> Refusal {
        Refusal(Room::Heap, Room::Heap.release_cnt())
    }

    /// By the body slots, every one out.
    pub fn bodies() -> Refusal {
        Refusal(Room::Bodies, Room::Bodies.release_cnt())
    }

    /// By the heap, for a run of body slots, every run held full.
    pub fn run() -> Refusal {
        Refusal(Room::Run, Room::Run.release_cnt())
    }

    /// Room has been given back since, so asking again may be served.
    pub fn over(self) -> bool {
        self.0.release_cnt() != self.1
    }
}
