// pxl-core/sys/io/room.rs

//! A refusal of room, by the heap, by the body slots (sys/io/body.rs) or by
//! the ways' pool (domain/layer/terrain/pool.rs), which counts its runs given
//! back here. It
//! records how many times what refused it had given room back, and it is over
//! once that count moves. A refused request waits only on what refused it,
//! never on the other.

use crate::sys::heap;
use crate::sys::io::body;
use pxl_global::Global;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Refusal(Room, u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Room {
    Heap,
    Bodies,
    Ways,
}

/// How many runs the ways' pool has given back.
static WAYS_GIVEN: Global<u32> = Global::new();

/// Counts a run given back to the ways' pool (domain/layer/terrain/pool.rs `give`).
pub(crate) fn way_given_back() {
    let n = WAYS_GIVEN.get(|| 0);
    *n = n.wrapping_add(1);
}

impl Room {
    fn release_cnt(self) -> u32 {
        match self {
            Room::Heap => heap::stat().releases,
            Room::Bodies => body::release_cnt(),
            Room::Ways => *WAYS_GIVEN.get(|| 0),
        }
    }
}

impl Refusal {
    /// A refusal by the heap.
    pub(crate) fn now() -> Refusal {
        Refusal(Room::Heap, Room::Heap.release_cnt())
    }

    /// A refusal by the body slots, when every slot is out.
    pub(crate) fn bodies() -> Refusal {
        Refusal(Room::Bodies, Room::Bodies.release_cnt())
    }

    /// A refusal by the ways' pool, when its cap keeps it from holding a run.
    pub(crate) fn ways() -> Refusal {
        Refusal(Room::Ways, Room::Ways.release_cnt())
    }

    /// Whether room has been given back since the refusal, so that asking
    /// again may be served.
    pub(crate) fn released(self) -> bool {
        self.0.release_cnt() != self.1
    }
}
