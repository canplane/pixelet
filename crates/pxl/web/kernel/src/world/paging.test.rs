// pxl-kernel/world/paging.test.rs
//
// The split between mechanism and policy, proved by running the mechanism
// under policies that do their worst: the ledger serving in an arbitrary
// order, last in first out, or stopping whatever it may; speculation off, or
// a flood of it; and a host answering the newest attempt first. With the
// camera at rest, whatever the policy:
//
// - residency never holds more frames than it has;
// - the fallback's pages and what the view reads are never refused a frame
//   (a refusal of one is a bug and aborts), and a page of the fallback once
//   resident stays so;
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
use crate::world::prefetch::{Prefetch, Reach};
use crate::world::pyramid::Pyramid;
use pxl_spec::space::lod::{fault_radius, Tau};
use pxl_spec::space::observation::{ByBounds, Observation, Unknown};

/// How high the eye stands over flat ground.
const HEIGHT: f64 = 1000.0;

fn the_observation() -> Observation {
    observation_at([30_000.0, -17_000.0])
}

/// Looking all around from `HEIGHT` over flat ground, with nothing known of
/// the heights: the most an observation from there can demand.
fn observation_at([x, y]: [f64; 2]) -> Observation {
    Observation {
        eye: [x, y, HEIGHT],
        frustum: None,
        far: reach_at(HEIGHT).min(fault_radius(TOP_LEVEL)),
        top: TOP_LEVEL,
        planet: f64::INFINITY,
        ground: 0.0,
    }
}

/// The fallback's pages for `o`.
fn floor_of(o: &Observation) -> Vec<LevelPage> {
    let mut out = Vec::new();
    Floor::floor(o.top, o.eye, o.far, |p| out.push(p));
    out
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
        p.children().into_iter().flat_map(|k| [(at(k), Place { name: "a", kind: 0, rank: 100 }), (at(k), Place { name: "b", kind: 0, rank: 100 })]),
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
    let floor = floor_of(&observation());
    for turn in 0..2_000 {
        let at = turn as f64 * 16.0;
        pyramid.observe(
            Some(&observation()),
            core::iter::empty(),
            &[],
            &Unknown,
            &ByBounds { tau: Tau::FINEST },
            prefetch,
            Tau::FINEST,
            allowance,
        );
        if exact(pyramid) && pyramid.wants().is_empty() {
            assert!(floor.iter().all(|&p| pyramid.paging().resident(p)), "the floor is held");
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
        for &p in floor.iter().filter(|p| resident.contains(p)) {
            assert!(pyramid.paging().resident(p), "turn {turn}: the floor's {p:?} given up");
        }
    }
    panic!("the view never came exact");
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
