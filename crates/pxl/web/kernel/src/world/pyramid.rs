// pxl-kernel/world/pyramid.rs

//! The pyramid as the client holds it, for every layer on the ground: the
//! pages held (paging.rs) and what an observation asks for of them.
//!
//! Asks are the observation's demand, as the estimator names it, merged with
//! the fallback's pages, a destination's and speculation, the best reason per
//! page. Nothing drawn enters them except the shared quality, which is
//! decided first. Where a page settles a child's ground (`Folds::settles`),
//! the child is never asked for.

pub mod chunk;
pub mod cover;
pub mod domain;

use std::collections::HashMap;

use pxl_spec::space::lod::Tau;
use pxl_spec::space::observation::{Bounds, DemandEstimator, Observation, Paged, Requirement, Storage as _};
use pxl_spec::space::pyramid::{Folds, LevelPage, TOP_LEVEL};

use crate::io::demand::{Ask, Category, Phases, Why};
use crate::io::sched::declare;
use crate::param;
use crate::sched::profiler::record::Failure;
use crate::world::observation::{heights_revision, observation, ByFootprints, Known, Order};
use crate::world::paging::floor::{self, Fallback, Floor};
use crate::world::paging::{Candidate, Paging, Victim, ASKS, DESTINATION_PAGES, FRAMES, LEVEL_DEMAND_PAGES};
use crate::world::prefetch::{NoPrefetch, Prefetch};
use crate::world::quality;

pub trait Layer: Folds + Clone + 'static {
    const CATEGORY: Category;

    fn pyramid() -> &'static mut Pyramid<Self>;

    fn receive(record: u32, pkt: &[u8], phases: &mut Phases) -> Result<Self, Failure>;

    fn absent(p: LevelPage);

    fn changed(_p: LevelPage, _present: bool) {}

    fn pages(r: &Requirement, each: &mut dyn FnMut(LevelPage)) {
        Paged.pages(r, each);
    }
}

/// Everything the asks depend on; two turns agreeing on all of it ask for
/// the same pages, so the second decides nothing.
type Seen = (Option<Observation>, Option<LevelPage>, u32, (u32, u32), Tau, u32);

/// No speculation: the view converges to exact on faults alone, and
/// speculation would only buy latency.
const PREFETCH: NoPrefetch = NoPrefetch;

/// Evicts unhinted pages before hinted ones, then the one last in view order.
struct Farthest<'a>(Option<&'a Observation>);

impl Victim for Farthest<'_> {
    fn victim(&self, candidates: &[Candidate]) -> Option<usize> {
        let order = |c: &Candidate| self.0.map_or(Order::LAST, |o| Order::of_page(o, c.page));
        (0..candidates.len()).max_by_key(|&i| (!candidates[i].hinted, order(&candidates[i])))
    }
}

pub struct Pyramid<L: Layer> {
    paging: Paging<L>,
    observation: Option<Observation>,
    demand: Vec<LevelPage>,
    wants: Vec<(u64, Ask)>,
    last: Option<Seen>,
    readers: Vec<LevelPage>,
    /// Reused across observations so observing stops allocating once warm.
    asks: HashMap<LevelPage, Ask>,
    hints: Vec<(Order, LevelPage)>,
}

impl<L: Layer> Default for Pyramid<L> {
    fn default() -> Self {
        Pyramid {
            paging: Paging::default(),
            observation: None,
            demand: Vec::new(),
            wants: Vec::new(),
            last: None,
            asks: HashMap::with_capacity(ASKS as usize),
            hints: Vec::new(),
        }
    }
}

impl<L: Layer> Pyramid<L> {
    pub fn get(&self, p: LevelPage) -> Option<&L> {
        self.paging.get(p)
    }

    pub fn settled(&self, p: LevelPage) -> bool {
        self.paging.settled(p)
    }

    pub fn paging(&self) -> &Paging<L> {
        &self.paging
    }

    pub fn set(&mut self, p: LevelPage, page: L) {
        self.paging.set(p, page);
    }

    pub fn revision(&self) -> u32 {
        self.paging.revision()
    }

    pub fn answer_above(&mut self, p: LevelPage) {
        self.paging.answer_above(p);
    }

    pub fn source(&self, p: LevelPage) -> Option<LevelPage> {
        self.paging.source(p)
    }

    pub fn drawn_from(&self, p: LevelPage) -> Option<LevelPage> {
        self.paging.drawn_from(p)
    }

    pub fn asked(&self, p: LevelPage) -> bool {
        self.paging.asked(p).is_some()
    }

    pub fn take_refusals(&mut self) -> u32 {
        self.paging.take_refusals()
    }

    pub fn demand(&self) -> &[LevelPage] {
        &self.demand
    }

    pub fn adopt(&mut self, p: LevelPage, pkt: &[u8], phases: &mut Phases) -> Result<(), Failure> {
        self.admit(p, |record| L::receive(record, pkt, phases))
    }

    pub fn admit(&mut self, p: LevelPage, receive: impl FnOnce(u32) -> Result<L, Failure>) -> Result<(), Failure> {
        self.paging.admit(p, &Farthest(self.observation.as_ref()), receive)
    }

    /// Builds the asks for `o`: the estimator's demand, the floor, the
    /// destination's line, `readers`, and speculation up to `allowance`.
    ///
    /// A reader's page is demanded only inside its level's fault radius, where
    /// the demand bound already counts it, and a hint past that. Speculation
    /// only takes frames left over after everything hard and demanded, so
    /// nothing is fetched just to find no frame.
    #[allow(clippy::too_many_arguments)]
    pub fn observe(
        &mut self,
        o: Option<&Observation>,
        destination: impl Iterator<Item = LevelPage>,
        bounds: &impl Bounds,
        estimator: &impl DemandEstimator,
        prefetch: &impl Prefetch,
        tau: Tau,
        allowance: u32,
    ) {
        let mut demand = core::mem::take(&mut self.demand);
        demand.clear();
        let mut asks = core::mem::take(&mut self.asks);
        asks.clear();
        let ask = |asks: &mut HashMap<LevelPage, Ask>, p: LevelPage, a: Ask| {
            asks.entry(p).and_modify(|b| *b = b.best(a)).or_insert(a);
        };
        let order = |o: &Observation, p: LevelPage| Order::of_region(o, bounds, p.level, p.region());
        if let Some(o) = o {
            estimator.requirements(o, bounds, &mut |r| L::pages(&r, &mut |p| demand.push(p)));
            demand.sort_unstable();
            demand.dedup();
            for &p in &demand {
                let at = self.paging.source(p).unwrap_or(p);
                ask(&mut asks, at, Ask { why: Why::Fault, order: order(o, p) });
            }
            let foot = [o.eye[0], o.eye[1]];
            for &p in readers {
                if pages_within(p.level, foot, fault_radius(p.level)).any(|q| q == p) {
                    ask(&mut asks, p, Ask { why: Why::Fault, order: order(o, p) });
                }
            }
            // More demanded than the structure's bound means the capacity is broken too.
            let mut read = [0u32; TOP_LEVEL as usize + 1];
            for p in asks.keys() {
                read[p.level as usize] += 1;
            }
            xpute_core::ensure!(read.iter().all(|&n| n <= LEVEL_DEMAND_PAGES), ENOTRECOVERABLE, *read.iter().max().unwrap_or(&0));
            let mut floor = 0u32;
            Floor::floor(o.top, o.eye, o.far, |p| {
                floor += 1;
                ask(&mut asks, p, Ask { why: Why::Floor, order: order(o, p) });
            });
            xpute_core::ensure!(floor <= Floor::PAGES, ENOTRECOVERABLE, floor);
        }
        // The whole line, not only the finest page: which one says the ground is
        // known only once those above are, and asking one at a time would cost a
        // round trip per level.
        let mut named = 0u32;
        for p in destination {
            named += 1;
            ask(
                &mut asks,
                p,
                Ask {
                    why: Why::Destination,
                    order: Order::FIRST,
                },
            );
        }
        xpute_core::ensure!(named <= DESTINATION_PAGES, ENOTRECOVERABLE, named);

        let mut hints = core::mem::take(&mut self.hints);
        hints.clear();
        if let Some(o) = o {
            let needed = asks.keys().filter(|&&p| self.paging.takes_frame(p)).count() as u32;
            let mut room = FRAMES.saturating_sub(needed).min(allowance);
            prefetch.speculate(o, tau, &mut |p| {
                if !asks.contains_key(&p) {
                    hints.push((order(o, p), p));
                }
            });
            hints.sort_unstable();
            hints.dedup_by_key(|h| h.1);
            // Settled or `ABSENT` pages take no frame but still count against `ASKS`.
            let mut pages = ASKS.saturating_sub(asks.len() as u32);
            for &(order, p) in &hints {
                let new = u32::from(self.paging.takes_frame(p));
                if new > room || pages == 0 {
                    break;
                }
                room -= new;
                pages -= 1;
                ask(&mut asks, p, Ask { why: Why::Hint, order });
            }
        }

        self.wants.clear();
        self.wants
            .extend(asks.iter().filter(|(p, _)| !self.paging.resident(**p) && !self.paging.settled(**p)).map(|(p, &a)| (p.subject(), a)));
        self.paging.ask(&asks);
        (self.asks, self.hints) = (asks, hints);

        self.observation = o.copied();
        self.demand = demand;
    }

    pub fn wants(&self) -> &[(u64, Ask)] {
        &self.wants
    }

    /// The layer's turn. Quality is decided first so asks match what is drawn.
    /// Observes again whenever anything in `Seen` or `readers` changed.
    ///
    /// `destination` is asked for with its line even with no observation yet:
    /// at the first fix the camera waits for that ground. `far` cuts the
    /// observation where the layer stops being drawn.
    pub fn step(&mut self, destination: Option<LevelPage>, readers: &[LevelPage], far: f64) {
        let Some(top) = floor::level() else { return };
        let o = observation();
        if o.is_none() && destination.is_none() {
            return;
        }
        let tau = match o {
            Some(_) => {
                quality::step();
                quality::effective().tau()
            }
            None => Tau::FINEST,
        };
        let seen = (o, destination, self.paging.revision(), heights_revision(), tau, domain::walks());
        if self.last != Some(seen) {
            let line = destination.into_iter().flat_map(|p| std::iter::successors(Some(p), move |q| (q.level < top).then(|| q.parent())));
            self.observe(o.as_ref(), line, readers, &Known, &ByFootprints { tau, sight: domain::sight() }, &PREFETCH, tau, param::HINT_ALLOWANCE);
            // Observing can let pages go, moving the revision: record the state after it.
            self.last = Some((o, destination, self.paging.revision(), heights_revision(), tau, domain::walks()));
        }
        declare(L::CATEGORY, &self.wants);
    }

    /// A layer the view does not show keeps what it holds and asks for nothing.
    pub fn idle(&mut self) {
        self.wants.clear();
        self.asks.clear();
        self.paging.ask(&self.asks);
        declare(L::CATEGORY, &[]);
        self.last = None;
    }
}

#[cfg(test)]
#[path = "pyramid.test.rs"]
mod test;
