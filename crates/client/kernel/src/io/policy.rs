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

use crate::io::demand::Order;
use crate::io::demand::{Ask, Stream};
use crate::io::failure::Failure;
use crate::io::request::{Rank, Retry, Serve};
use crate::param;
use pxl_spec::space::layer::tile::Tile;
use xpute_kit::math::rng::{u32_to_unit, SplitMix64};
use xpute_kit::status::errno::Errno;

/// A layer's classes: seen or unseen.
const SHELLS: u8 = 2;
const TERRAIN: u8 = 0;
const ASSET: u8 = TERRAIN + SHELLS;
const PLACE: u8 = ASSET + 1;
const ASIDE: u8 = PLACE + SHELLS;

pub struct Policy {
    pub intake: u32,
}

impl Policy {
    pub const fn standard() -> Policy {
        Policy { intake: param::INTAKE }
    }
}

/// The wait before the `failures`th retry: AWS's "full jitter", uniform up to
/// the capped exponential, so clients one outage failed together do not
/// return together.
fn backoff(failures: u32, seed: u64) -> f64 {
    let ceiling = (param::RETRY_BASE_MS * libm::exp2(f64::from(failures - 1))).min(param::RETRY_CAP_MS);
    ceiling * u32_to_unit(SplitMix64::new(seed).next_u32())
}

fn tile(layer: Stream, subject: u64, ask: Ask) -> Rank {
    if ask.order == Order::LAST {
        return Rank {
            class: ASIDE,
            distance: 0.0,
            detail: 0,
        };
    }
    let base = if layer == Stream::terrain { TERRAIN } else { PLACE };
    Rank {
        class: base + !ask.order.seen as u8,
        distance: ask.order.near,
        detail: Tile::of_subject(subject).zoom,
    }
}

impl Serve for Policy {
    fn rank(&self, stream: Stream, subject: u64, ask: Ask, _enqueue_ms: f64) -> Rank {
        match stream {
            Stream::terrain | Stream::place | Stream::earth => tile(stream, subject, ask),
            Stream::asset => Rank {
                class: ASSET,
                distance: 0.0,
                detail: 0,
            },
            Stream::division => Rank {
                class: ASIDE,
                distance: 0.0,
                detail: 0,
            },
        }
    }

    fn intake(&self) -> u32 {
        self.intake
    }

    /// A network that is down is waited on; a server that keeps failing is given up.
    fn retry(&self, failure: Failure, failures: u32, seed: u64) -> Retry {
        if failure.errno != Errno::ENETUNREACH && failures >= param::RETRY_ATTEMPTS {
            return Retry::GiveUp;
        }
        Retry::After(backoff(failures, seed))
    }
}

#[cfg(test)]
#[path = "policy.test.rs"]
mod test;
