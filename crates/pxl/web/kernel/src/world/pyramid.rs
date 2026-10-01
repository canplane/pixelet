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
//! page, the one over it; one not resident is a fault, answered meanwhile
//! by the nearest resident page over it. The fallback's pages
//! (paging/floor.rs), a destination's (control/navigation/destination.rs)
//! and speculation (prefetch.rs) are merged in, the best reason per page,
//! and what of that is not held is said to the scheduler once a turn. How the pages are drawn
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
//! None of that is a layer's. A layer says what its page is, how a fetched
//! body becomes one in a record, what a page with
//! no object is, and who reads its pages; the rest is here, once.

pub mod chunk;
pub mod cover;
pub mod domain;

use std::collections::HashMap;

use pxl_spec::space::lod::{fault_radius, pages_within, Tau};
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

    /// The pages that meet a requirement (pxl-spec space/observation.rs
    /// `Storage`): the pyramid's own, 2D, for every layer on it now.
    fn pages(r: &Requirement, each: &mut dyn FnMut(LevelPage)) {
        Paged.pages(r, each);
    }
}

/// What the asks are a function of: the observation, the destination, what
/// is held
/// (`Paging::revision`), what is known of the heights
/// (`heights_revision`), which neither the observation nor this layer's
/// pages move, the quality the view is held to (world/quality.rs), and
/// which casting of the screen's tiles the demand is estimated from
/// (world/pyramid/domain.rs `walks`). Two turns agreeing on all of it ask
/// for the same pages, so the second decides nothing again.
type Seen = (Option<Observation>, Option<LevelPage>, u32, (u32, u32), Tau, u32);

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
    /// The observation last asked for.
    observation: Option<Observation>,
    /// The pages it requires, as the estimator names them.
    demand: Vec<LevelPage>,
    /// What is asked for and not held, said again every turn as it stands.
    wants: Vec<(u64, Ask)>,
    last: Option<Seen>,
    /// The `readers` the last observation was made with.
    readers: Vec<LevelPage>,
    /// What an observation asks for, and the speculation it gathers on the
    /// way, held across observations so observing allocates nothing once it
    /// has held the most it asks for: the asks are at most `ASKS`.
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
            readers: Vec::new(),
            asks: HashMap::with_capacity(ASKS as usize),
            hints: Vec::new(),
        }
    }
}

impl<L: Layer> Pyramid<L> {
    /// A resident page. None past the pyramid's top too, which is what a top
    /// page's parent is.
    pub fn get(&self, p: LevelPage) -> Option<&L> {
        self.paging.get(p)
    }

    /// Whether `p` is no page because one over it says its ground
    /// (`Paging::settled`).
    pub fn settled(&self, p: LevelPage) -> bool {
        self.paging.settled(p)
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

    /// What the observation `o` asks for, where there is one: what
    /// `estimator` says `o` requires, as far as `bounds` know the heights,
    /// the fallback's pages, and what `prefetch` speculates on at `tau`, at
    /// most `allowance` of it — and with or without it, the `destination`'s
    /// pages, the best reason per page; and the pages another layer reads
    /// from this one (`readers`). What of it is not held is what the layer
    /// wants.
    ///
    /// A reader's page is demanded where a fault could name it — inside
    /// its level's fault radius about the eye's foot, where the demand's
    /// bound already counts it — and is a hint past that. Decor's cells
    /// reach past the radius by a parent's diagonal, a split forced by a
    /// sibling's, which would take a level's pages from three a side to
    /// four; what such a cell does not get, it draws its parent's
    /// population for (layer/decor/cell.rs `step`).
    ///
    /// Speculation is asked for only as far as frames are left over once
    /// everything held hard and everything demanded has one, so none of it
    /// is fetched only to find no frame.
    #[allow(clippy::too_many_arguments)]
    pub fn observe(
        &mut self,
        o: Option<&Observation>,
        destination: impl Iterator<Item = LevelPage>,
        readers: &[LevelPage],
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
            // What is demanded is inside the demand's bound, whatever the
            // ground: more is the structure's bound broken, and the capacity
            // with it.
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
        // Every page on the line rather than the finest alone: which of them
        // says the destination's ground is known only once those over it are
        // (`Paging::source`), and asking them one after another would be a
        // round trip a level. First in the order, since the eye is going
        // there.
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
            hints.extend(readers.iter().filter(|p| !asks.contains_key(p)).map(|&p| (order(o, p), p)));
            hints.sort_unstable();
            hints.dedup_by_key(|h| h.1);
            // Nor for more pages than are left of what may be asked at once,
            // whether or not each takes a frame: a page settled or held in
            // `ABSENT` takes none, and is known only while it is asked for
            // (paging.rs `ASKS`).
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

    /// What the layer wants of the scheduler: every page asked for and not
    /// held.
    pub fn wants(&self) -> &[(u64, Ask)] {
        &self.wants
    }

    /// The layer's turn. The quality is decided first (world/quality.rs),
    /// so what is asked for is at the quality what is drawn is cut at.
    /// Wherever the observation, the destination, what is held, the heights,
    /// the quality or what the screen's tiles saw have changed, it is
    /// observed again (`observe`), since the asks are functions of all of
    /// them. A turn where none changed decides nothing and says the wants it
    /// has.
    ///
    /// The estimator is the one from what the tiles saw (world/observation.rs
    /// `ByFootprints`), which the quality's turn has just cast: the ground
    /// the screen reaches, and what it lacked, and not what is hidden.
    ///
    /// `destination` is the z16 page a destination stands in, whose ground
    /// is wanted exactly: asked for with the line over it, up to the top the
    /// source supplies, whether or not there is an observation yet — at the
    /// first fix there is none, since the camera waits for that ground to be
    /// put at all.
    ///
    /// `readers` are the pages another layer reads from this one, as
    /// `observe` takes them.
    ///
    /// `far` is how far the layer is seen, its observation cut there: what
    /// it draws past that is nothing, so it asks for nothing past it.
    pub fn step(&mut self, destination: Option<LevelPage>, readers: &[LevelPage], far: f64) {
        let Some(top) = floor::level() else { return };
        let o = observation().map(|o| Observation { far: o.far.min(far), ..o });
        if o.is_none() && destination.is_none() {
            return;
        }
        let tau = match o {
            Some(_) => {
                quality::step();
                quality::effective().tau()
            }
            // Nothing is observed, so nothing is asked at a quality.
            None => Tau::FINEST,
        };
        let seen = (o, destination, self.paging.revision(), heights_revision(), tau, domain::walks());
        if self.last != Some(seen) || self.readers != readers {
            let line = destination.into_iter().flat_map(|p| std::iter::successors(Some(p), move |q| (q.level < top).then(|| q.parent())));
            self.observe(o.as_ref(), line, readers, &Known, &ByFootprints { tau, sight: domain::sight() }, &PREFETCH, tau, param::HINT_ALLOWANCE);
            // Observing lets go; what is recorded is the state the
            // observation was made against.
            self.last = Some((o, destination, self.paging.revision(), heights_revision(), tau, domain::walks()));
            self.readers.clear();
            self.readers.extend_from_slice(readers);
        }
        declare(L::CATEGORY, &self.wants);
    }

    /// Asks for nothing: a layer the view does not show holds what it has
    /// and wants no more.
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
