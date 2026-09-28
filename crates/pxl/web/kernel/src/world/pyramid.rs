// pxl-kernel/world/pyramid.rs

//! The pyramid as the client holds it, for every layer the ground carries
//! (pxl-spec space/pyramid.rs is the hierarchy itself): a layer's pages held
//! (world/paging.rs), and what an observation asks for of them.
//!
//! What is asked for is the observation's demand and nothing drawn: what its
//! samples require, before it is pages (pxl-spec space/observation.rs), as a
//! conservative estimator names it from what is known of the heights, mapped
//! to the layer's pages by the layer's storage. Each such page is asked for
//! at the page that says its ground — itself, or where the bake left it no
//! page, the one over it — with every page it depends on; one not resident
//! is a fault, answered meanwhile by the nearest resident page over it. The
//! fallback's pages (paging/floor.rs), the consumers' leases and speculation
//! (prefetch.rs) are merged in, the best reason per page, and what of that
//! is not held is said to the scheduler once a turn. How the pages are drawn
//! — the pieces the view is cut into (pyramid/domain.rs), what a layer draws
//! of them (pyramid/cover.rs, chunk/frontier.rs) — is made from what is
//! resident and enters none of it, but through the one quality both are
//! held to (world/quality.rs), which is decided before either reads it.
//!
//! The tree is as deep as a layer's data needs and no deeper: where a page
//! says all of a child's ground (`Folds::settles`), the child is no page, is
//! never asked for, and its ground is drawn from the page over it
//! (`source`). Only the finest level is a bound.
//!
//! None of that is a layer's. A layer says what its page is and what it
//! depends on, how a fetched body becomes one in a record, what a page with
//! no object is, and who reads its pages; the rest is here, once.

pub mod chunk;
pub mod cover;
pub mod domain;

use std::collections::HashMap;

use pxl_spec::space::lod::Tau;
use pxl_spec::space::observation::{Bounds, ByBounds, DemandEstimator, Observation, Paged, Requirement, Storage as _};
use pxl_spec::space::pyramid::{Folds, LevelPage, TOP_LEVEL};

use crate::io::demand::{Ask, Category, Phases, Why};
use crate::io::sched::declare;
use crate::param;
use crate::sched::profiler::record::Failure;
use crate::world::observation::{heights_revision, observation, Known, Order};
use crate::world::paging::floor::{Fallback, Floor};
use crate::world::paging::{Candidate, Paging, Victim, ASKS, FRAMES, LEVEL_DEMAND_PAGES};
use crate::world::prefetch::{NoPrefetch, Prefetch};
use crate::world::quality;

/// A layer the pyramid holds: its page is itself, folded by `Folds`.
pub trait Layer: Folds + Clone + 'static {
    /// The stream a page of this layer is asked for under.
    const CATEGORY: Category;

    fn pyramid() -> &'static mut Pyramid<Self>;

    /// A fetched body, made the page `record` holds.
    fn receive(record: u32, pkt: &[u8], phases: &mut Phases) -> Result<Self, Failure>;

    /// A page the bake left as no object: what it reads as, set now or once
    /// the layer has it.
    fn absent(p: LevelPage);

    /// A page became resident or stopped being; whoever reads this layer's
    /// pages is told.
    fn changed(_p: LevelPage, _present: bool) {}

    /// The page `p` depends on: held whenever `p` is, and `p` read only once
    /// it is resident. None for a page of the pyramid: a page carries the
    /// index of everything under it in its own lanes, and a page that is held
    /// exists, so nothing over it is needed to read it. The page over a
    /// missing one is the fallback's and the query's, which descend from the
    /// nearest resident page, never a condition on reading one — or a
    /// missing ancestor would make a resident descendant unreachable.
    fn dependencies(_p: LevelPage) -> Option<LevelPage> {
        None
    }

    /// The pages that meet a requirement (pxl-spec space/observation.rs
    /// `Storage`): the pyramid's own, 2D, for every layer on it now.
    fn pages(r: &Requirement, each: &mut dyn FnMut(LevelPage)) {
        Paged.pages(r, each);
    }
}

/// What the asks are a function of: the observation, what is held and
/// leased (`Paging::revision`), what is known of the heights
/// (`heights_revision`), which neither the observation nor this layer's
/// pages move, and the quality the view is held to (world/quality.rs). Two
/// turns agreeing on all of it ask for the same pages, so the second decides
/// nothing again.
type Seen = (Observation, u32, (u32, u32), Tau);

/// The speculation the kernel runs: none. The view is paged by its faults,
/// and with no speculation at all it converges to exact
/// (world/paging.test.rs); what speculation would buy is latency, and which
/// policy buys it is chosen here — `Reach`, with param.rs's numbers, is the
/// one there is.
const PREFETCH: NoPrefetch = NoPrefetch;

/// The victim policy the kernel pages by: a page speculation does not ask
/// for before one it does, and among those, the one last in the view's
/// order, the one the eye is least likely to reach again.
struct Farthest<'a>(Option<&'a Observation>);

impl Victim for Farthest<'_> {
    fn victim(&self, candidates: &[Candidate]) -> Option<usize> {
        let order = |c: &Candidate| self.0.map_or(Order::LAST, |o| Order::of_page(o, c.page));
        (0..candidates.len()).max_by_key(|&i| (!candidates[i].hinted, order(&candidates[i])))
    }
}

pub struct Pyramid<L: Layer> {
    paging: Paging<L>,
    /// What the fallback leases now.
    floor: Vec<LevelPage>,
    /// The observation last asked for.
    observation: Option<Observation>,
    /// The pages it requires, as the estimator names them.
    demand: Vec<LevelPage>,
    /// What is asked for and not held, said again every turn as it stands.
    wants: Vec<(u64, Ask)>,
    last: Option<Seen>,
}

impl<L: Layer> Default for Pyramid<L> {
    fn default() -> Self {
        Pyramid {
            paging: Paging::default(),
            floor: Vec::new(),
            observation: None,
            demand: Vec::new(),
            wants: Vec::new(),
            last: None,
        }
    }
}

impl<L: Layer> Pyramid<L> {
    /// A resident page. None past the pyramid's top too, which is what a top
    /// page's parent is.
    pub fn get(&self, p: LevelPage) -> Option<&L> {
        self.paging.get(p)
    }

    /// What is held, and how.
    pub fn paging(&self) -> &Paging<L> {
        &self.paging
    }

    /// Holds a page the bake left as no object (`Paging::set`).
    pub fn set(&mut self, p: LevelPage, page: L) {
        self.paging.set(p, page);
    }

    /// Which resident set this is (`Paging::revision`).
    pub fn revision(&self) -> u32 {
        self.paging.revision()
    }

    /// The bucket has no object for `p`, under the top (`Paging::answer_above`).
    pub fn answer_above(&mut self, p: LevelPage) {
        self.paging.answer_above(p);
    }

    /// The resident page that says `p`'s ground exactly (`Paging::source`).
    pub fn source(&self, p: LevelPage) -> Option<LevelPage> {
        self.paging.source(p)
    }

    /// The resident page `p`'s ground is drawn from (`Paging::drawn_from`).
    pub fn drawn_from(&self, p: LevelPage) -> Option<LevelPage> {
        self.paging.drawn_from(p)
    }

    /// Holds `p` hard for a consumer that takes no coarser answer, until
    /// released.
    pub fn lease(&mut self, p: LevelPage) {
        self.paging.lease(p);
    }

    pub fn release(&mut self, p: LevelPage) {
        self.paging.release(p);
    }

    /// Whether anything asks for `p` now.
    pub fn asked(&self, p: LevelPage) -> bool {
        self.paging.asked(p).is_some()
    }

    /// Arrivals given no frame since the last take, for the profiler.
    pub fn take_refusals(&mut self) -> u32 {
        self.paging.take_refusals()
    }

    /// The pages the last observation requires, as the estimator named them.
    pub fn demand(&self) -> &[LevelPage] {
        &self.demand
    }

    // ---- arriving ----

    /// A page's fetched body, made resident.
    pub fn adopt(&mut self, p: LevelPage, pkt: &[u8], phases: &mut Phases) -> Result<(), Failure> {
        self.admit(p, |record| L::receive(record, pkt, phases))
    }

    /// `p` has arrived, and `receive` makes it the page a frame holds
    /// (`Paging::admit`), the frame given up if one must be by the kernel's
    /// victim policy.
    pub fn admit(&mut self, p: LevelPage, receive: impl FnOnce(u32) -> Result<L, Failure>) -> Result<(), Failure> {
        self.paging.admit(p, &Farthest(self.observation.as_ref()), receive)
    }

    // ---- asking ----

    /// What the observation `o` asks for: the fallback leased, and every page
    /// asked for — what `estimator` says `o` requires, as far as `bounds`
    /// know the heights, the leases, and what `prefetch` speculates on at
    /// `tau`, at most `allowance` of it — each with what it depends on, the
    /// best reason per page. What of it is not held is what the layer wants.
    ///
    /// Speculation is asked for only as far as frames are left over once
    /// everything held hard and everything demanded has one, so none of it
    /// is fetched only to find no frame.
    pub fn observe(&mut self, o: &Observation, bounds: &impl Bounds, estimator: &impl DemandEstimator, prefetch: &impl Prefetch, tau: Tau, allowance: u32) {
        let mut floor = Vec::with_capacity(Floor::PAGES as usize);
        Floor::floor(o.top, o.eye, o.far, |p| floor.push(p));
        xpute_core::ensure!(floor.len() as u32 <= Floor::PAGES, ENOTRECOVERABLE, floor.len());
        for &p in self.floor.iter().filter(|p| !floor.contains(p)) {
            self.paging.release(p);
        }
        for &p in floor.iter().filter(|p| !self.floor.contains(p)) {
            self.paging.lease(p);
        }
        self.floor = floor;

        let mut demand = Vec::new();
        estimator.requirements(o, bounds, &mut |r| L::pages(&r, &mut |p| demand.push(p)));
        demand.sort_unstable();
        demand.dedup();
        let mut asks: HashMap<LevelPage, Ask> = HashMap::new();
        let ask = |asks: &mut HashMap<LevelPage, Ask>, p: LevelPage, a: Ask| {
            let mut q = Some(p);
            while let Some(p) = q {
                asks.entry(p).and_modify(|b| *b = b.best(a)).or_insert(a);
                q = L::dependencies(p);
            }
        };
        let order = |p: LevelPage| Order::of(o, p.rect(), bounds.heights(p.level, p.rect()));
        for &p in &demand {
            let at = self.paging.source(p).unwrap_or(p);
            ask(&mut asks, at, Ask { why: Why::Fault, order: order(p) });
        }
        // What is demanded is inside the demand's bound, whatever the ground:
        // more is the structure's bound broken, and the capacity with it.
        let mut read = [0u32; TOP_LEVEL as usize + 1];
        for p in asks.keys() {
            read[p.level as usize] += 1;
        }
        xpute_core::ensure!(read.iter().all(|&n| n <= LEVEL_DEMAND_PAGES), ENOTRECOVERABLE, *read.iter().max().unwrap_or(&0));
        for &p in &self.floor {
            ask(&mut asks, p, Ask { why: Why::Floor, order: order(p) });
        }
        // A lease is held for its consumer, not the view: it has no place in
        // the view's order unless the view reads it too.
        for p in self.paging.leases().filter(|p| !self.floor.contains(p)) {
            ask(&mut asks, p, Ask { why: Why::Lease, order: Order::LAST });
        }

        let needed = asks.keys().filter(|&&p| self.paging.takes_frame(p)).count() as u32;
        let mut room = FRAMES.saturating_sub(needed).min(allowance);
        let mut hints: Vec<(Order, LevelPage)> = Vec::new();
        prefetch.speculate(o, tau, &mut |p| {
            if !asks.contains_key(&p) {
                hints.push((order(p), p));
            }
        });
        hints.sort_unstable();
        hints.dedup_by_key(|h| h.1);
        // Nor for more pages than are left of what may be asked at once,
        // whether or not each takes a frame: a page settled or held in
        // `ABSENT` takes none, and is known only while it is asked for
        // (paging.rs `ASKS`).
        let mut pages = ASKS.saturating_sub(asks.len() as u32);
        for (order, p) in hints {
            let fresh: Vec<LevelPage> = std::iter::successors(Some(p), |&q| L::dependencies(q)).filter(|q| !asks.contains_key(q)).collect();
            let new = fresh.iter().filter(|&&q| self.paging.takes_frame(q)).count() as u32;
            if new > room || fresh.len() as u32 > pages {
                break;
            }
            room -= new;
            pages -= fresh.len() as u32;
            ask(&mut asks, p, Ask { why: Why::Hint, order });
        }

        self.wants.clear();
        self.wants
            .extend(asks.iter().filter(|(p, _)| !self.paging.held(**p) && !self.paging.settled(**p)).map(|(p, &a)| (p.subject(), a)));
        self.paging.ask(asks);

        self.observation = Some(*o);
        self.demand = demand;
    }

    /// What the layer wants of the scheduler: every page asked for and not
    /// held.
    pub fn wants(&self) -> &[(u64, Ask)] {
        &self.wants
    }

    /// The layer's turn. The quality is decided first (world/quality.rs),
    /// so what is asked for is at the quality what is drawn is cut at.
    /// Wherever the observation, what is held or leased, the heights or the
    /// quality have changed, it is observed again (`observe`), since the asks
    /// are functions of all of them. A turn where none changed decides
    /// nothing and says the wants it has.
    ///
    /// The estimator is the one from bounds alone, hidden ground asked for
    /// too; one from the samples' hits is a smaller superset and changes
    /// nothing else here.
    pub fn step(&mut self) {
        let Some(o) = observation() else { return };
        quality::step();
        let tau = quality::effective().tau();
        let seen = (o, self.paging.revision(), heights_revision(), tau);
        if self.last != Some(seen) {
            self.observe(&o, &Known, &ByBounds { tau }, &PREFETCH, tau, param::HINT_ALLOWANCE);
            // Observing leases and lets go; what is recorded is the state the
            // observation was made against.
            self.last = Some((o, self.paging.revision(), heights_revision(), tau));
        }
        declare(L::CATEGORY, &self.wants);
    }

    /// Asks for nothing: a layer the view does not show holds what it has
    /// and wants no more.
    pub fn idle(&mut self) {
        self.wants.clear();
        self.paging.ask(HashMap::new());
        declare(L::CATEGORY, &[]);
        self.last = None;
    }
}

#[cfg(test)]
#[path = "pyramid.test.rs"]
mod test;
