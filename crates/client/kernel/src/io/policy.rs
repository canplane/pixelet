// pxl-kernel/io/policy.rs

//! The kernel's serve policy: the one place every producer's vocabulary is
//! ranked against the others. It changes how soon the view is right, never
//! what it is.
//!
//! 1. By layer, the order a view builds up (the user's order,
//!    2026-09-26): terrain, then the catalog assets decor draws with, then
//!    places.
//! 2. Within a layer, seen before unseen and nearer first;
//!    of tiles as near, the finer first, so the leaf under the eye is not
//!    asked after the coarse tiles over it, which cost a fold where baked on demand.
//!    Ranking holes before coarse ground instead put the top tile's 2 km
//!    cubes in front of the eye for seconds on a cold start (2026-09-26).
//!
//! Everything else ties, after the view. Rank orders what starts; nothing
//! running is stopped for it.

use crate::io::demand::{Ask, Category};
use crate::io::failure::Outcome;
use crate::io::sched::{Rank, Serve};
use crate::param;
use crate::world::view::observation::Order;
use pxl_spec::space::layer::tile::Tile;

/// A layer's classes: seen or unseen.
const SHELLS: u8 = 2;
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

fn tile(layer: Category, subject: u64, ask: Ask) -> Rank {
    if ask.order == Order::LAST {
        return Rank {
            class: ASIDE,
            distance: 0.0,
            detail: 0,
        };
    }
    let base = if layer == Category::terrain { TERRAIN } else { PLACES };
    Rank {
        class: base + !ask.order.seen as u8,
        distance: ask.order.near,
        detail: Tile::of_subject(subject).zoom,
    }
}

impl Serve for Policy {
    fn rank(&self, category: Category, subject: u64, ask: Ask, _enqueue_ms: f64) -> Rank {
        match category {
            Category::terrain | Category::poi | Category::earth => tile(category, subject, ask),
            Category::asset => Rank {
                class: ASSET,
                distance: 0.0,
                detail: 0,
            },
            // Neither blocks the other: the label renders on the turn both are there.
            Category::geocode | Category::geocode_dict => Rank {
                class: ASIDE,
                distance: 0.0,
                detail: 0,
            },
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
