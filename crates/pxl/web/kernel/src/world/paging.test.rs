// pxl-kernel/world/paging.test.rs
//
// The split between mechanism and policy, proved by running the mechanism
// under policies that do their worst: the ledger serving in an arbitrary
// order, last in first out, or stopping whatever it may; speculation off, or
// a flood of it; and a host answering the newest attempt first. With the
// camera at rest, whatever the policy:
//
// - residency never holds more frames than it has;
// - a lease and what the view reads are never refused a frame (a refusal of
//   one is a bug and aborts), and a leased page once resident stays so;
// - a page, once resident, stays so while it is held: what it depends on is
//   never given up under it;
// - every page the observation demands ends up read exactly, at its level,
//   and nothing is left asked for.

use std::collections::HashSet;

use pxl_spec::bake::poi_page::{Place, PoiPage, POI_LATTICE};

use super::*;
use crate::io::demand::Category;
use crate::io::policy::Policy;
use crate::io::sched::{Ledger, Rank, Serve};
use crate::layer::environment::fog::reach_at;
use crate::sched::profiler::record::{Failure, Outcome};
use crate::world::prefetch::{NoPrefetch, Prefetch, Reach};
use crate::world::pyramid::Pyramid;
use pxl_spec::space::lod::{fault_radius, pages_within};
use pxl_spec::space::observation::{ByBounds, Observation, Unknown};

/// How high the eye stands over flat ground.
const HEIGHT: f64 = 1000.0;

fn the_observation() -> Observation {
    observation_at([30_000.0, -17_000.0])
}

/// An observation some hundreds of kilometers from `the_observation`,
/// sharing no page under the top with it.
fn elsewhere() -> Observation {
    observation_at([-400_000.0, 250_000.0])
}

/// Looking all around from `HEIGHT` over flat ground, with nothing known of
/// the heights: the most an observation from there can demand.
fn observation_at([x, y]: [f64; 2]) -> Observation {
    Observation {
        eye: [x, y, HEIGHT],
        frustum: None,
        far: reach_at(HEIGHT).min(fault_radius(TOP_LEVEL)),
        planet: f64::INFINITY,
        ground: 0.0,
    }
}

/// A page with a crowd over each of its children, so it settles none of them
/// and the tree runs to the finest level everywhere.
fn crowded(p: LevelPage) -> PoiPage {
    if p.level == 0 {
        return PoiPage::default();
    }
    let at = |k: LevelPage| POI_LATTICE.cell_base(k, 0, 0);
    PoiPage::of_places(
        p.level,
        p.children().into_iter().flat_map(|k| [(at(k), Place { name: "a", kind: 0 }), (at(k), Place { name: "b", kind: 0 })]),
    )
}

fn mix(x: u64) -> u64 {
    let x = x.wrapping_mul(0x9e37_79b9_7f4a_7c15);
    (x ^ (x >> 31)).wrapping_mul(0xbf58_476d_1ce4_e5b9)
}

/// Serve policies made hostile.
enum Hostile {
    /// An arbitrary order: a function of the want, with nothing of the view.
    Random(u64),
    /// The newest want first.
    Lifo,
    /// Stops whatever it is let stop, for whatever is ahead of it.
    Preempt,
}

impl Serve for Hostile {
    fn rank(&self, _category: Category, subject: u64, ask: crate::io::demand::Ask, enqueue_ms: f64) -> Rank {
        let distance = match self {
            Hostile::Random(seed) => (mix(subject ^ seed) >> 11) as f64,
            Hostile::Lifo => -enqueue_ms,
            Hostile::Preempt => ask.order.near,
        };
        Rank { class: 0, distance }
    }

    fn preempts(&self, _waiting: Rank, _running: Rank) -> bool {
        matches!(self, Hostile::Preempt)
    }

    fn intake(&self) -> u32 {
        u32::MAX
    }

    fn retry_after(&self, _outcome: Outcome) -> Option<f64> {
        None
    }
}

/// Speculation run wild: every page of every level for dozens of pages about
/// the eye.
struct Flood;

impl Prefetch for Flood {
    fn speculate(&self, o: &Observation, each: &mut dyn FnMut(LevelPage)) {
        let [x, y, _] = o.eye;
        for level in 0..=TOP_LEVEL {
            pages_within(level, [x, y], 8.0 * pxl_spec::space::pyramid::span_of(level)).for_each(&mut *each);
        }
    }
}

/// Whether every page the observation demands is read exactly.
fn exact(pyramid: &Pyramid<PoiPage>) -> bool {
    pyramid.demand().iter().all(|&p| pyramid.source(p).is_some())
}

/// Runs the mechanism under `serve` and `prefetch` with the camera at rest,
/// holding the invariants every turn, until the view is exact and nothing is
/// left asked for; what is held then.
fn converge<S: Serve>(serve: S, prefetch: &impl Prefetch, allowance: u32) -> Pyramid<PoiPage> {
    let mut pyramid = Pyramid::<PoiPage>::default();
    converge_on(&mut pyramid, the_observation, serve, prefetch, allowance);
    pyramid
}

/// As `converge`, for `pyramid` as it is, with the camera at rest on
/// `observation`.
fn converge_on<S: Serve>(pyramid: &mut Pyramid<PoiPage>, observation: fn() -> Observation, serve: S, prefetch: &impl Prefetch, allowance: u32) {
    let mut ledger = Ledger::new(serve);
    ledger.set_credits(4);
    let mut out: Vec<u64> = Vec::new();
    let mut resident: HashSet<LevelPage> = HashSet::new();
    for turn in 0..2_000 {
        let at = turn as f64 * 16.0;
        pyramid.observe(&observation(), &Unknown, &ByBounds, prefetch, allowance);
        if exact(pyramid) && pyramid.wants().is_empty() {
            assert!(pyramid.paging().leases().all(|p| pyramid.paging().resident(p)), "every lease is held");
            return;
        }
        ledger.declare(Category::poi, pyramid.wants(), at);
        ledger.pump(at);
        let mut stops = Vec::new();
        ledger.take_stops(|key| stops.push(key));
        out.retain(|key| !stops.contains(key));
        ledger.take_started(|key, _, _| out.push(key));
        assert!(out.len() <= 4, "no more out than the credits");
        // The host answers the newest half of what is out, newest first.
        let answered = out.len().div_ceil(2);
        for key in out.split_off(out.len() - answered).into_iter().rev() {
            let (_, subject) = ledger.attempt_of(key).expect("an attempt out");
            let p = LevelPage::of_subject(subject);
            pyramid.admit(p, |_| Ok(crowded(p))).expect("a page is made");
            ledger.settle(key, Outcome::ok, None, Failure::default(), at);
        }
        assert!(pyramid.paging().frames_used() <= FRAMES, "turn {turn}: past the frames");
        for (p, _) in (0..=TOP_LEVEL).flat_map(|level| pyramid.paging().level(level)).collect::<Vec<_>>() {
            if pyramid.paging().resident(p) {
                resident.insert(p);
            }
        }
        for &p in &resident {
            if pyramid.paging().held(p) {
                assert!(pyramid.paging().resident(p), "turn {turn}: {p:?} held and no longer resident");
            }
        }
        for p in pyramid.paging().leases() {
            if resident.contains(&p) {
                assert!(pyramid.paging().resident(p), "turn {turn}: the lease on {p:?} given up");
            }
        }
    }
    panic!("the view never came exact");
}

#[test]
fn the_view_converges_exact_with_no_prefetch_under_every_order() {
    let _serial = crate::global::serial();
    converge(Policy::standard(), &NoPrefetch, 0);
    converge(Hostile::Random(7), &NoPrefetch, 0);
    converge(Hostile::Random(0xdead_beef), &NoPrefetch, 0);
    converge(Hostile::Lifo, &NoPrefetch, 0);
    converge(Hostile::Preempt, &NoPrefetch, 0);
}

#[test]
fn the_view_converges_exact_with_the_reach_prefetched_under_every_order() {
    let _serial = crate::global::serial();
    let reach = Reach { slack: 1.0, margin: 0.0 };
    for allowance in [0, 16, u32::MAX] {
        converge(Policy::standard(), &reach, allowance);
        converge(Hostile::Random(11), &reach, allowance);
        converge(Hostile::Lifo, &reach, allowance);
        converge(Hostile::Preempt, &reach, allowance);
    }
}

/// Speculation far past the frames: it is asked for only as far as frames
/// are left once everything held hard and everything the view reads has
/// one, and it never takes one of theirs.
#[test]
fn a_flood_of_speculation_never_displaces_a_lease_or_what_the_view_reads() {
    let _serial = crate::global::serial();
    for mut pyramid in [
        converge(Policy::standard(), &Flood, u32::MAX),
        converge(Hostile::Lifo, &Flood, u32::MAX),
        converge(Hostile::Preempt, &Flood, u32::MAX),
        converge(Hostile::Random(3), &Flood, u32::MAX),
    ] {
        assert_eq!(pyramid.paging().frames_used(), FRAMES, "the flood fills every frame left over");
        assert_eq!(pyramid.take_refusals(), 0, "and never asks for one it cannot have");
    }
}

/// The fallback keeps its law: once its pages are resident, every page the
/// observation demands has an answer, whatever else is held.
#[test]
fn once_the_floor_is_resident_every_observation_has_an_answer() {
    let _serial = crate::global::serial();
    let mut pyramid = Pyramid::<PoiPage>::default();
    let unanswered = |pyramid: &Pyramid<PoiPage>| pyramid.demand().iter().filter(|&&p| pyramid.drawn_from(p).is_none()).count();
    pyramid.observe(&the_observation(), &Unknown, &ByBounds, &NoPrefetch, 0);
    assert!(unanswered(&pyramid) > 0, "a cold start is open");
    let floor: Vec<LevelPage> = pyramid.paging().leases().collect();
    assert!(!floor.is_empty() && floor.iter().all(|p| p.level == TOP_LEVEL));
    for p in floor {
        pyramid.admit(p, |_| Ok(crowded(p))).expect("a page is made");
    }
    pyramid.observe(&the_observation(), &Unknown, &ByBounds, &NoPrefetch, 0);
    assert_eq!(unanswered(&pyramid), 0, "pages with no answer");
}

/// The view holds a page only while it asks for it: once the eye has moved
/// on, what it read before is cache, and a view elsewhere converges exact
/// over it.
#[test]
fn a_page_the_view_no_longer_asks_for_is_no_longer_held_for_it() {
    let _serial = crate::global::serial();
    let mut pyramid = converge(Policy::standard(), &NoPrefetch, 0);
    let read: Vec<LevelPage> = (0..TOP_LEVEL)
        .flat_map(|level| pyramid.paging().level(level).map(|(p, _)| p).collect::<Vec<_>>())
        .filter(|&p| pyramid.paging().hold(p) == Hold::Demand)
        .collect();
    assert!(!read.is_empty(), "the view held its pages for itself");
    pyramid.observe(&elsewhere(), &Unknown, &ByBounds, &NoPrefetch, 0);
    for &p in &read {
        assert!(pyramid.paging().asked(p).is_none(), "{p:?} is asked for from elsewhere");
        assert_eq!(pyramid.paging().hold(p), Hold::Cache, "{p:?} still held for a view that left it");
    }
    converge_on(&mut pyramid, elsewhere, Policy::standard(), &NoPrefetch, 0);
}
