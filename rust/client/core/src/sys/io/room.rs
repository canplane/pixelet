// pxl-core/sys/io/room.rs

//! A refusal of room, by the heap, by the body slots (sys/io/body.rs) or by the ways' pool
//! (domain/layer/terrain/pool.rs), which counts its runs given back here. It records how many
//! times what refused it had given room back, and it is over once that count moves. A refused request waits only on what refused
//! it, never on the other. A slot given back returns nothing to the heap
//! until its whole run is empty, and only then does the heap's count move;
//! memory the heap gets back frees no slot. A body refused a new run of slots
//! waits on both counts, because a slot given back in a run already held
//! serves it too.

use crate::sys::boot::heap_stat;
use crate::sys::io::body;
use xpute_core::global::Global;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Refusal(Room, u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Room {
    Heap,
    Bodies,
    Run,
    Ways,
}

/// How many runs the ways' pool has given back.
static WAYS_GIVEN: Global<u32> = Global::new();

/// Counts a run given back to the ways' pool (domain/layer/terrain/pool.rs `give`).
pub fn way_given_back() {
    let n = WAYS_GIVEN.get(|| 0);
    *n = n.wrapping_add(1);
}

impl Room {
    fn release_cnt(self) -> u32 {
        match self {
            Room::Heap => heap_stat().releases,
            Room::Bodies => body::releases(),
            Room::Run => heap_stat().releases.wrapping_add(body::releases()),
            Room::Ways => *WAYS_GIVEN.get(|| 0),
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

    /// A refusal by the ways' pool, when its cap keeps it from holding a run.
    pub fn ways() -> Refusal {
        Refusal(Room::Ways, Room::Ways.release_cnt())
    }

    /// Whether room has been given back since the refusal, so that asking
    /// again may be served.
    pub fn released(self) -> bool {
        self.0.release_cnt() != self.1
    }
}
