// pxl-kernel/io/policy.rs

//! The kernel's serve policy: the one place every producer's vocabulary is
//! ranked against the others. It changes how soon the view is right, never
//! what it is.
//!
//! 1. By layer, the order a view builds up (the user's order,
//!    2026-09-26): terrain, then the catalog assets decor draws with, then
//!    places.
//! 2. Within a layer, seen before unseen and nearer first, speculation last.
//!    Ranking holes before coarse ground instead put the top page's 2 km
//!    cubes in front of the eye for seconds on a cold start (2026-09-26).
//!
//! Everything else ties, after the view. Rank orders what starts; nothing
//! running is stopped for it.

use crate::io::demand::{Ask, Category, Why};
use crate::io::sched::{Rank, Serve};
use crate::param;
use crate::sched::profiler::record::Outcome;
use crate::world::observation::Order;

/// A layer's classes: needed or speculative, each seen or unseen.
const SHELLS: u8 = 2 * 2;
const TERRAIN: u8 = 0;
const ASSET: u8 = TERRAIN + SHELLS;
const PLACES: u8 = ASSET + 1;
const ASIDE: u8 = PLACES + SHELLS;

pub struct Policy {
    pub retry_ms: f64,
    pub intake: u32,
}

impl Policy {
    pub const fn standard() -> Policy {
        Policy {
            retry_ms: param::RETRY_MS,
            intake: param::INTAKE,
        }
    }
}

fn page(layer: Category, ask: Ask) -> Rank {
    if ask.order == Order::LAST {
        return Rank { class: ASIDE, distance: 0.0 };
    }
    let base = if layer == Category::terrain { TERRAIN } else { PLACES };
    let shell = u8::from(ask.why == Why::Hint);
    Rank {
        class: base + 2 * shell + !ask.order.seen as u8,
        distance: ask.order.near,
    }
}

impl Serve for Policy {
    fn rank(&self, category: Category, _subject: u64, ask: Ask, _enqueue_ms: f64) -> Rank {
        match category {
            Category::terrain | Category::poi | Category::earth => page(category, ask),
            Category::asset => Rank { class: ASSET, distance: 0.0 },
            // Neither blocks the other: the label renders on the turn both are there.
            Category::geocode | Category::geocode_dict => Rank { class: ASIDE, distance: 0.0 },
        }
    }

    fn intake(&self) -> u32 {
        self.intake
    }

    /// Only a failure waits; an answer and a stop do not.
    fn retry_after(&self, outcome: Outcome) -> Option<f64> {
        (outcome == Outcome::error).then_some(self.retry_ms)
    }
}

#[cfg(test)]
#[path = "policy.test.rs"]
mod test;
