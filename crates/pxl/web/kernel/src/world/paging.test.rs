// pxl-kernel/world/paging.test.rs
//
// Residency under hostile policies (arbitrary, LIFO or nearest-first serving,
// no or flooding speculation, newest-first answers): frames are
// never exceeded, the floor and the view are never refused a frame, and
// every demanded page ends up read exactly.

use std::collections::HashSet;

use pxl_spec::bake::poi_page::{Place, PoiPage, POI_LATTICE};

use super::*;
use crate::io::demand::Category;
use crate::io::policy::Policy;
use crate::io::sched::{Rank, RequestScheduler, Serve};
use crate::layer::environment::fog::reach_at;
use crate::sched::profiler::record::{Failure, Outcome};
use crate::world::prefetch::{Prefetch, Reach};
use crate::world::pyramid::Pyramid;
use pxl_spec::space::lod::{fault_radius, Tau};
use pxl_spec::space::observation::{ByBounds, Observation, Unknown};

const HEIGHT: f64 = 1000.0;

fn the_observation() -> Observation {
    observation_at([30_000.0, -17_000.0])
}

/// Looking all around, knowing nothing of the heights: the most an
/// observation from there can demand.
fn observation_at([x, y]: [f64; 2]) -> Observation {
    Observation {
        eye: [x, y, HEIGHT],
        frustum: None,
        far: reach_at(HEIGHT).min(fault_radius(TOP_Z)),
        top: TOP_Z,
        planet: f64::INFINITY,
        ground: 0.0,
    }
}

fn floor_of(o: &Observation) -> Vec<Page> {
    let mut out = Vec::new();
    Floor::floor(o, |p| out.push(p));
    out
}

/// Settles none of its children, so the tree runs to the leaves everywhere.
fn crowded(p: Page) -> PoiPage {
    if p.z == LEAF_Z {
        return PoiPage::default();
    }
    let at = |k: Page| POI_LATTICE.cell_base(k, 0, 0);
    PoiPage::of_places(
        p.z,
        p.children().into_iter().flat_map(|k| {
            [
                (
                    at(k),
                    Place {
                        name: "a",
                        id: "1",
                        ..Place::default()
                    },
                ),
                (
                    at(k),
                    Place {
                        name: "b",
                        id: "1",
                        ..Place::default()
                    },
                ),
            ]
        }),
        true,
    )
}

fn mix(x: u64) -> u64 {
    let x = x.wrapping_mul(0x9e37_79b9_7f4a_7c15);
    (x ^ (x >> 31)).wrapping_mul(0xbf58_476d_1ce4_e5b9)
}

enum Hostile {
    Random(u64),
    Lifo,
    Nearest,
}

impl Serve for Hostile {
    fn rank(&self, _category: Category, subject: u64, ask: crate::io::demand::Ask, enqueue_ms: f64) -> Rank {
        let distance = match self {
            Hostile::Random(seed) => (mix(subject ^ seed) >> 11) as f64,
            Hostile::Lifo => -enqueue_ms,
            Hostile::Nearest => ask.order.near,
        };
        Rank { class: 0, distance }
    }

    fn intake(&self) -> u32 {
        u32::MAX
    }

    fn retry_after(&self, _outcome: Outcome) -> Option<f64> {
        None
    }
}

fn exact(pyramid: &Pyramid<PoiPage>) -> bool {
    pyramid.demand().iter().all(|&p| pyramid.paging().source(p).is_some())
}

/// Holds the invariants every turn until the view is exact and nothing is asked for.
fn converge<S: Serve>(serve: S, prefetch: &impl Prefetch, allowance: u32) -> Pyramid<PoiPage> {
    let mut pyramid = Pyramid::<PoiPage>::default();
    converge_on(&mut pyramid, the_observation, serve, prefetch, allowance);
    pyramid
}

fn converge_on<S: Serve>(pyramid: &mut Pyramid<PoiPage>, observation: fn() -> Observation, serve: S, prefetch: &impl Prefetch, allowance: u32) {
    let mut sched = RequestScheduler::new(serve);
    sched.set_credits(4);
    let mut out: Vec<u64> = Vec::new();
    let mut resident: HashSet<Page> = HashSet::new();
    let floor = floor_of(&observation());
    for turn in 0..2_000 {
        let at = turn as f64 * 16.0;
        pyramid.observe(
            Some(&observation()),
            None,
            core::iter::empty(),
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
        sched.declare(Category::poi, pyramid.wants(), at);
        sched.pump(at);
        let mut stops = Vec::new();
        sched.take_stops(|key| stops.push(key));
        out.retain(|key| !stops.contains(key));
        sched.take_started(|credit, _, _| out.push(credit.key()));
        assert!(out.len() <= 4, "no more out than the credits");
        let answered = out.len().div_ceil(2);
        for key in out.split_off(out.len() - answered).into_iter().rev() {
            let (_, subject) = sched.attempt_of(key).expect("an attempt out");
            let p = Page::of_subject(subject);
            pyramid.admit(p, |_| Ok(crowded(p))).expect("a page is made");
            sched.settle(key, Outcome::ok, None, Failure::default(), at);
        }
        assert!(pyramid.paging().frames_used() <= Paging::<PoiPage>::FRAMES, "turn {turn}: past the frames");
        resident.extend(pyramid.paging().held().map(|(p, _)| p));
        for &p in floor.iter().filter(|p| resident.contains(p)) {
            assert!(pyramid.paging().resident(p), "turn {turn}: the floor's {p:?} given up");
        }
    }
    panic!("the view never came exact");
}

#[test]
fn the_view_converges_exact_with_the_reach_prefetched_under_every_order() {
    let _serial = crate::serial();
    let reach = Reach { slack: 1.0, margin: 0.0 };
    for allowance in [0, 16, u32::MAX] {
        converge(Policy::standard(), &reach, allowance);
        converge(Hostile::Random(11), &reach, allowance);
        converge(Hostile::Lifo, &reach, allowance);
        converge(Hostile::Nearest, &reach, allowance);
    }
}
