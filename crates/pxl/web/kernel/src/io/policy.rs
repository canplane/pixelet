// pxl-kernel/io/policy.rs

//! Where a producer's own fact becomes a place in one order.
//!
//! This is the only module that knows every producer's vocabulary, and the
//! only one that holds a number. A producer says what it knows in its own
//! terms — a page is not drawn, a page is drawn coarsely — and the number it
//! lands on is decided here, once,
//! where the producers can be read against each other.
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
//! 3. Within a layer, one order for every layer, out from the view's centre
//!    (world/pyramid/domain.rs `Domain::order_of`), the point the camera
//!    turns about and the character stands on:
//!    - the page the ground nearest the centre is drawn from, so where the
//!      character stands is right before anything else is;
//!    - the pyramid's top pages, a handful, which every chunk under them can
//!      be drawn from until its own comes, so no ground is left open for
//!      long;
//!    - every page the view draws ground from, nearest the centre first, so
//!      the view fills outwards as a sphere grows;
//!    - the pages held only for ground drawn finer to fall back on.
//!
//!    Ranking a hole before ground drawn coarsely filled the view coarsest
//!    first instead: from a cold start the eye's ancestors took the first six
//!    connections, and the top page's two-kilometre cubes stood in front of
//!    the eye for seconds while its own page waited (measured 2026-09-26).
//!
//! **Everything else is a tie.** A label, the device's own page and the
//! geocode dictionary have no relation to each other or to the view that
//! anything in this tree gives a reason for. They rank the same, after the
//! view, and `io/sched.rs` breaks the tie by category and subject: an order
//! that means nothing and is the same every run.
//!

use crate::io::demand::Category;
use crate::io::sched::Rank;
use crate::sched::ledger::record::Outcome;

/// The shared open-ocean tile: every page waiting on it is a page showing
/// nothing, and one arrival answers all of them.
pub fn ocean() -> Rank {
    Rank { class: 0, distance: 0.0 }
}

/// Where a page stands in its layer's order (point 3 above).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shell {
    /// The page the ground nearest the view's centre is drawn from.
    Centre,
    /// A page of the pyramid's top.
    Top,
    /// A page the view draws ground from.
    Drawn,
    /// A page held only for ground drawn finer to fall back on.
    Fallback,
}

const SHELLS: u8 = 4;
/// Where each layer's shells begin (point 2 above).
const TERRAIN: u8 = 1;
const ASSET: u8 = TERRAIN + SHELLS;
const PLACES: u8 = ASSET + 1;

/// A page of a layer the pyramid holds, where it stands in the layer's order,
/// and its distance from the view's centre.
pub fn page(layer: Category, shell: Shell, distance: f64) -> Rank {
    let base = if layer == Category::terrain { TERRAIN } else { PLACES };
    Rank { class: base + shell as u8, distance }
}

/// Where everything with no argued relation sits: one class after the view,
/// and the tie broken by nothing that means anything.
const ASIDE: u8 = PLACES + SHELLS;

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

/// A catalog model or image: a fixed set, wanted once, never stale, and
/// what the decor standing on the ground is drawn with.
pub fn asset() -> Rank {
    Rank { class: ASSET, distance: 0.0 }
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
    (outcome == Outcome::error).then_some(2_000.0)
}
