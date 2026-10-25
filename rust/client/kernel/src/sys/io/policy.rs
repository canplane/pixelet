// pxl-kernel/sys/io/policy.rs

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

use crate::sys::io::demand::Order;
use crate::sys::io::demand::{Ask, Stream};
use crate::sys::io::failure::Failure;
use crate::sys::io::param;
use crate::sys::io::request::{Blame, Rank, Serve};
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
        // A zoom fits the rank's byte.
        detail: Tile::of_subject(subject).zoom as u8,
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

    /// As the client's read classes them (`ts/client/src/io/read.ts`). An object's
    /// failure comes back the same however often it is asked: the bytes are
    /// fixed by the key within a version.
    fn blame(&self, failure: Failure) -> Blame {
        match failure.errno {
            // The browser says it is offline.
            Errno::ENETUNREACH => Blame::Network,
            // Any other status but 204 and 404, or no response while online. The
            // Worker answers only 200, 204 and 404, so this is the platform under
            // it: R2's internal error, a Cloudflare incident (R2 down 59 minutes
            // on 2025-02-06), the free plan's daily cap failing closed, an edge
            // refusing the connection.
            Errno::EACCES | Errno::EIO => Blame::Server,
            // ENXIO: 404, a key that names nothing. EBADMSG: a body empty, still
            // packed in an encoding this browser does not undo, or not decoding.
            // EFBIG: larger than the heap takes, or than its kind's packet can be.
            _ => Blame::Object,
        }
    }

    /// Exponential because the server is shared, so every client's retries add up
    /// on it; AWS's "full jitter", uniform up to the capped exponential, so
    /// clients one outage failed together do not return together.
    fn backoff(&self, strikes: u32, seed: u64) -> Option<f64> {
        if strikes > param::RETRY_ATTEMPTS {
            return None;
        }
        let ceiling = (param::RETRY_BASE_MS * libm::exp2(f64::from(strikes - 1))).min(param::RETRY_CAP_MS);
        Some(ceiling * u32_to_unit(SplitMix64::new(seed).next_u32()))
    }
}
