// pxl-kernel/io/policy.rs

//! Where a producer's own fact becomes a place in one order.
//!
//! This is the only module that knows every producer's vocabulary, and the
//! only one that holds a number. A producer says what it knows in its own
//! terms — a page is not drawn, a page is drawn coarsely, a page is outside
//! the ring the view draws — and the number it lands on is decided here, once,
//! where the producers can be read against each other.
//!
//! **Only the orderings the code gives a reason for are kept.** There are
//! four, and they are all terrain's or terrain's dependency's:
//!
//! 1. The shared ocean tile before a page that is not drawn. One arrival sets
//!    every page waiting on it (`tile_repository.rs`'s `adopt_ocean` drains
//!    `waiting`); a page's own arrival sets one.
//! 2. A page nothing draws before a page drawn coarsely. The first is a hole
//!    on the screen and the second is already there (`resolve.rs`).
//! 3. A page the view draws before one outside the ring it draws. The second
//!    is margin against a move that has not happened.
//! 4. Nearer before further, within any of those.
//!
//! **Everything else is a tie.** A label, a POI page, the device's own page
//! and a catalog asset have no relation to each other or to terrain that
//! anything in this tree gives a reason for — the numbers they carry today
//! (`BEHIND_TERRAIN`, `PRIORITY_BIAS`, `PINNED_PAGE_PRIORITY`, and a `+1.0`
//! between a dictionary and its group) were each written to place one thing
//! against another and none of them is argued anywhere. They rank the same
//! here, and `io/sched.rs` breaks the tie by category and subject: an order
//! that means nothing and is the same every run.
//!

use crate::io::sched::Rank;
use crate::layer::terrain::resolve::Resolved;
use crate::sched::ledger::record::Outcome;

/// The shared open-ocean tile: every page waiting on it is a page showing
/// nothing, and one arrival answers all of them.
pub fn ocean() -> Rank {
    Rank { class: 0, distance: 0.0 }
}

/// A terrain page the view wants, `distance` from what the view is looking
/// at, and whether it falls outside the ring the view actually draws.
///
/// `Exact` and `Foldable` are not demands — the caller has them already — and
/// rank as the weakest thing here rather than being given a place of their
/// own, since nothing should be asking for one.
pub fn terrain(resolved: Resolved, speculative: bool, distance: f64) -> Rank {
    let class = match (resolved, speculative) {
        (_, true) => 3,
        (Resolved::Missing, _) => 1,
        (Resolved::Covered, _) => 2,
        (Resolved::Exact | Resolved::Foldable, _) => ASIDE,
    };
    Rank { class, distance }
}

/// Where everything with no argued relation sits: one class, and the tie
/// broken by nothing that means anything.
const ASIDE: u8 = 4;

/// The device's own page, which the marker grounds itself against. It is
/// genuinely far and nothing on the screen waits on it.
pub fn pinned() -> Rank {
    Rank { class: ASIDE, distance: 0.0 }
}

/// A reverse-geocode group or the dictionary it is read against. Neither
/// blocks the other: a group that arrives first sits in its store until the
/// dictionary comes, and the label renders on the turn both are there
/// (`shell/shell.rs`).
pub fn geocode() -> Rank {
    Rank { class: ASIDE, distance: 0.0 }
}

/// POI demand has already passed view relevance. Proximity orders only
/// those summaries whose refinement the current view needs.
pub fn poi(distance: f64) -> Rank {
    Rank { class: ASIDE, distance }
}

/// A catalog model or image: a fixed set, wanted once, never stale.
pub fn asset() -> Rank {
    Rank { class: ASIDE, distance: 0.0 }
}

/// How long an identity waits before another attempt.
///
/// An answer is not something to try again — the producer has it and stops
/// asking — and a stop is not a failure. Only a failure waits, and the wait
/// is the one this tree ever argued for: long enough not to ask straight back
/// into whatever turned it away. It is the same for every producer because
/// nothing here says it should differ; when something does, this is where it
/// is said.
pub fn retry_after(outcome: Outcome) -> Option<f64> {
    (outcome == Outcome::Error).then_some(2_000.0)
}
