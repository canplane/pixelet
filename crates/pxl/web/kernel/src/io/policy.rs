// pxl-kernel/io/policy.rs

//! The kernel's serve policy (io/sched.rs `Serve`): where a producer's own
//! fact becomes a place in one order, when a waiting want is worth a running
//! attempt's credit, and how long a failure waits.
//!
//! This is the only module that knows every producer's vocabulary. A
//! producer says what it knows in its own terms — why a page is asked for,
//! where it falls in the view's order — and the number it lands on is decided
//! here, once, where the producers can be read against each other. Nothing a
//! want's answer depends on is decided here: another policy changes how soon
//! the view is right and never what it is once it is.
//!
//! **Only the orderings the code gives a reason for are kept.**
//!
//! 1. The shared ocean tile before any page. One arrival sets every page
//!    waiting on it (`layer/terrain/page.rs`'s `adopt_ocean` drains
//!    `waiting`); a page's own arrival sets one.
//! 2. The layers in the order a view is built up: the ground, then what
//!    stands on it, then the marks over it — terrain's pages, then the
//!    catalog's models and images the decor is drawn with, then the places'
//!    pages (the user's order, 2026-09-26). Placed objects are all resident
//!    and are built with the ground's chunks, so they are never fetched.
//! 3. Within a layer, one order for every page the view needs, out from the
//!    eye (world/observation.rs `Order`): what the view sees before what
//!    it does not, and the nearer the eye first. What it needs is the pages
//!    it reads now, the pages over them they depend on — which carry the
//!    place of the nearest ground that needs them, so the chain over the
//!    ground in front of the eye goes first, all of it at once — and the
//!    fallback's pages (world/paging/floor.rs), at their own place. Then
//!    speculation, last of the layer.
//!
//!    Ranking a hole before ground drawn coarsely filled the view coarsest
//!    first instead: from a cold start the eye's ancestors took the first six
//!    connections, and the top page's two-kilometer cubes stood in front of
//!    the eye for seconds while its own page waited (measured 2026-09-26).
//!    The pages over a page the view reads are no such hole: the page is read
//!    only once they are resident (world/paging.rs), so they are asked with
//!    it, at its place.
//!
//! **Everything else is a tie.** A label, a lease no view reads — a
//! consumer's page away from the eye — and the geocode dictionary have no
//! relation to each other or to the view that anything in this tree gives a
//! reason for. They rank the same, after the view, and io/sched.rs breaks the
//! tie by category and subject: an order that means nothing and is the same
//! every run.
//!
//! **A running attempt is stopped only for a whole class better.** Distance
//! says which of two alike to start, not that one running is worth
//! restarting; taking a credit costs everything the attempt has already
//! spent, and only a class says the waiting work is a different kind of
//! wanted.

use crate::io::demand::{Ask, Category, Why};
use crate::io::sched::{Rank, Serve};
use crate::param;
use crate::sched::profiler::record::Outcome;
use crate::world::observation::Order;

/// Classes a layer's pages fall in, what the view needs and speculation,
/// each with what the view sees ahead of what it does not (point 3 above).
const SHELLS: u8 = 2 * 2;
/// Where each layer's classes begin (point 2 above).
const TERRAIN: u8 = 1;
const ASSET: u8 = TERRAIN + SHELLS;
const PLACES: u8 = ASSET + 1;
/// Where everything with no argued relation sits: one class after the view,
/// and the tie broken by nothing that means anything.
const ASIDE: u8 = PLACES + SHELLS;

/// The kernel's policy, with the numbers it serves by (param.rs).
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

/// A page of a layer, by why it is asked for and its place in the view's
/// order: a page with none, a lease the view does not read, is aside.
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
            // The shared open-ocean tile: every page waiting on it is a page
            // showing nothing, and one arrival answers all of them.
            Category::ocean => Rank { class: 0, distance: 0.0 },
            Category::terrain | Category::poi => page(category, ask),
            // A catalog model or image: a fixed set, wanted once, never
            // stale, and what the decor standing on the ground is drawn with.
            Category::asset => Rank { class: ASSET, distance: 0.0 },
            // A reverse-geocode group or the dictionary it is read against.
            // Neither blocks the other: a group that arrives first sits in its
            // store until the dictionary comes, and the label renders on the
            // turn both are there (`shell/shell.rs`).
            Category::geocode | Category::geocode_dict => Rank { class: ASIDE, distance: 0.0 },
        }
    }

    fn preempts(&self, waiting: Rank, running: Rank) -> bool {
        waiting.class < running.class
    }

    fn intake(&self) -> u32 {
        self.intake
    }

    /// An answer is not something to try again — the producer has it and
    /// stops asking — and a stop is not a failure. Only a failure waits, and
    /// the wait is the same for every producer because nothing here says it
    /// should differ; when something does, this is where it is said.
    fn retry_after(&self, outcome: Outcome) -> Option<f64> {
        (outcome == Outcome::error).then_some(self.retry_ms)
    }
}

#[cfg(test)]
#[path = "policy.test.rs"]
mod test;
