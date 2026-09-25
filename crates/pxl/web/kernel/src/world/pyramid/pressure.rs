// pxl-kernel/world/pyramid/pressure.rs

//! The byte budget, paid in LOD: how many levels coarser the rule's
//! criterion is pulled while the heap cannot hold what a view wants. It is
//! an input of the domain (domain.rs `domain_for`), as the eye is, so the
//! one criterion the view is chosen by moves, and what it draws gets
//! coarser — fewer chunks, fewer pages, smaller tiers — until it fits, as
//! Cesium raises a tileset's screen-space error when its tiles exceed its
//! cache.
//!
//! What moves it is a refusal (host.rs `HeapStat`) that giving back what
//! the view no longer shows could not answer: a request refused since the
//! last step, in a frame whose reclaim freed nothing, is a step in; a spell
//! with no refusal, and the heap off its mark, is a step out. Not the heap's
//! releases: those move whenever anything anywhere lets a block go, a fetch
//! or a run set, which on a busy heap is every frame.

use crate::global::Global;
use crate::host::heap_stat;

/// The heap's live share past which it counts as pressed: a step out waits
/// for it to fall under, and the chunk table's reclaim gives off-screen
/// decor back past it whatever the retain budget says.
const HEAP_PRESSURE: f64 = 0.9;
/// Levels the criterion may be pulled in, a step at most this often, so the
/// coarser layouts have time to land and free what they replace before the
/// next is taken; a step is let out again after this long with no refusal
/// and the heap off its mark.
const MAX_PRESSURE: u32 = 6;
const STEP_MS: f64 = 500.0;
const RELIEF_MS: f64 = 5000.0;

struct Pressure {
    level: u32,
    /// When it last moved, and when a request was last refused.
    at: f64,
    refused_at: f64,
    /// The heap's refusals as the last frame saw them, and as the last step
    /// did: a refusal inside a step's hold is still one when the hold ends.
    refusals: u32,
    refusals_at_step: u32,
}

static PRESSURE: Global<Pressure> = Global::new();

fn state() -> &'static mut Pressure {
    PRESSURE.get(|| Pressure {
        level: 0,
        at: f64::NEG_INFINITY,
        refused_at: f64::NEG_INFINITY,
        refusals: 0,
        refusals_at_step: 0,
    })
}

/// Levels the criterion is pulled in now.
pub fn pressure() -> u32 {
    state().level
}

/// Whether the heap is live past HEAP_PRESSURE.
pub fn heap_pressed() -> bool {
    let h = heap_stat();
    h.live as f64 > h.range as f64 * HEAP_PRESSURE
}

/// A step, at `now`, taken once a frame after the chunk table's reclaim,
/// which gave back `freed` chunks' decor: in when a request was refused
/// since the last step and the reclaim freed nothing, out when the
/// refusals have stopped and the heap is off its mark. A refusal no step
/// took is let go once none has come for RELIEF_MS.
pub fn pressure_step(now: f64, freed: u32) {
    let (h, p) = (heap_stat(), state());
    if h.refusals != p.refusals {
        (p.refusals, p.refused_at) = (h.refusals, now);
    }
    let pending = h.refusals != p.refusals_at_step && now - p.refused_at < RELIEF_MS;
    let next = if pending && freed == 0 && p.level < MAX_PRESSURE && now - p.at >= STEP_MS {
        p.level + 1
    } else if p.level > 0 && now - p.refused_at >= RELIEF_MS && now - p.at >= RELIEF_MS && !heap_pressed() {
        p.level - 1
    } else {
        return;
    };
    (p.level, p.at, p.refusals_at_step) = (next, now, h.refusals);
}

#[cfg(test)]
#[path = "pressure.test.rs"]
mod test;
