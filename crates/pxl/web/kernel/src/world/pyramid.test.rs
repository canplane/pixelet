// pxl-kernel/world/pyramid.test.rs

use std::collections::HashSet;

use super::*;
use crate::world::prefetch::NoPrefetch;
use pxl_spec::bake::poi_page::{Place, PoiPage, POI_LATTICE};
use pxl_spec::space::lod::{fault_radius, Tau};
use pxl_spec::space::observation::{ByBounds, Contents, Frustum, Unknown};
use pxl_spec::space::pvf3::ivec;
use pxl_spec::space::v3::Aabb;

const HEIGHT: f64 = 1000.0;

fn at(eye: [f64; 3], frustum: Option<Frustum>) -> Observation {
    Observation {
        eye,
        frustum,
        far: fault_radius(TOP_Z),
        top: TOP_Z,
        planet: f64::INFINITY,
        ground: 0.0,
    }
}

fn the_observation() -> Observation {
    at([30_000.0, -17_000.0, HEIGHT], None)
}

fn named(o: &Observation) -> Vec<Page> {
    let mut out = Vec::new();
    ByBounds { tau: Tau::FINEST }.requirements(o, &Unknown, &mut |r| PoiPage::pages(&r, &mut |p| out.push(p)));
    out.sort_unstable();
    out.dedup();
    out
}

struct Flat;

impl Bounds for Flat {
    fn contents(&self, _z: u32, region: Aabb) -> Contents {
        let ground = Aabb {
            min: [region.min[0], region.min[1], 0.0],
            max: [region.max[0], region.max[1], 0.0],
        };
        ground.meet(region).map_or(Contents::Empty, |bound| Contents::Holds { bound, error: [0.0; 3] })
    }
}

/// Asked for first: residency keeps an empty page only while something asks.
fn hold(pyramid: &mut Pyramid<PoiPage>, pages: impl IntoIterator<Item = (Page, PoiPage)>) {
    let pages: Vec<(Page, PoiPage)> = pages.into_iter().collect();
    let aside = Ask { why: Why::Fault, order: Order::LAST };
    pyramid.paging.ask(&pages.iter().map(|&(p, _)| (p, aside)).collect());
    for (p, page) in pages {
        pyramid.set(p, page);
    }
}

fn crowded(p: Page) -> PoiPage {
    if p.z == GROUND_Z {
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

/// Every named page not resident is asked for at the page that says its
/// ground; nothing held or settled is. Nothing of how the view is drawn is an input.
#[test]
fn the_demand_is_the_estimators_and_every_fault_in_it_is_asked_for() {
    let _serial = crate::serial();
    let o = the_observation();
    let mut pyramid = Pyramid::<PoiPage>::default();
    hold(&mut pyramid, named(&o).into_iter().filter(|p| p.z <= 13 || ivec::X(p.page) % 2 == 0).map(|p| (p, crowded(p))));
    pyramid.observe(Some(&o), None, core::iter::empty(), &Unknown, &ByBounds { tau: Tau::FINEST }, &NoPrefetch, Tau::FINEST, 0);
    assert_eq!(pyramid.demand(), named(&o).as_slice(), "the demand is the estimator's");
    let asked: HashSet<u64> = pyramid.wants().iter().map(|(s, _)| *s).collect();
    let mut faults = 0;
    for &p in pyramid.demand() {
        if pyramid.paging().source(p).is_some() {
            continue;
        }
        faults += 1;
        let held = pyramid.paging.resident(p) || pyramid.paging.settled(p);
        assert_eq!(asked.contains(&p.subject()), !held, "{p:?}");
    }
    assert!(faults > 0, "a residency with faults");
    for &(s, _) in pyramid.wants() {
        let p = Page::of_subject(s);
        assert!(!pyramid.paging.resident(p) && !pyramid.paging.settled(p), "{p:?} asked for and held or said");
    }
}

#[test]
fn ground_a_page_settles_is_never_asked_for() {
    let _serial = crate::serial();
    let o = the_observation();
    let mut pyramid = Pyramid::<PoiPage>::default();
    let mut top: Vec<Page> = named(&o).into_iter().map(|p| (TOP_Z..p.z).fold(p, |q, _| q.parent())).collect();
    top.sort_unstable();
    top.dedup();
    hold(&mut pyramid, top.into_iter().map(|p| (p, PoiPage::default())));
    pyramid.observe(Some(&o), None, core::iter::empty(), &Unknown, &ByBounds { tau: Tau::FINEST }, &NoPrefetch, Tau::FINEST, 0);
    for &p in pyramid.demand() {
        assert!(pyramid.paging().source(p).is_some_and(|s| s.z == TOP_Z), "{p:?} is said by the page at the top");
    }
    assert!(pyramid.wants().is_empty(), "{} pages asked for", pyramid.wants().len());
}

/// With the view looking east, every demanded page reaches east of a plane
/// through the eye, within the sphere the frustum is tested with.
#[test]
fn ground_outside_the_frustum_is_not_demanded() {
    let _serial = crate::serial();
    let [x, y, z] = [123_456.7, -98_765.4, 400.0];
    // One plane, x ≥ the eye's, and five that pass everything.
    let mut planes = [0.0; 24];
    planes[..4].copy_from_slice(&[1.0, 0.0, 0.0, -x]);
    for k in 1..6 {
        planes[k * 4 + 3] = 1.0;
    }
    let o = at([x, y, z], Some(Frustum { planes }));
    let mut pyramid = Pyramid::<PoiPage>::default();
    pyramid.observe(Some(&o), None, core::iter::empty(), &Flat, &ByBounds { tau: Tau::FINEST }, &NoPrefetch, Tau::FINEST, 0);
    let all = named(&at([x, y, z], None));
    assert!(pyramid.demand().len() < all.len(), "looking one way demands less than looking all around");
    for &p in pyramid.demand() {
        let (x0, _, side) = p.rect();
        assert!(x0 + side / 2.0 + side * std::f64::consts::SQRT_2 / 2.0 >= x, "{p:?} lies wholly behind the eye");
    }
}

#[test]
fn observing_a_moving_eye_allocates_nothing_once_it_has_held_the_most() {
    let _serial = crate::serial();
    let mut pyramid = Pyramid::<PoiPage>::default();
    let path = |k: u32| at([30_000.0 + 97.0 * k as f64, -17_000.0 + 41.0 * k as f64, HEIGHT + (k % 7) as f64 * 150.0], None);
    let walk = |pyramid: &mut Pyramid<PoiPage>| {
        for k in 0..40 {
            pyramid.observe(Some(&path(k)), None, core::iter::empty(), &Unknown, &ByBounds { tau: Tau::FINEST }, &NoPrefetch, Tau::FINEST, 0);
        }
    };
    let (first, ()) = crate::allocations::during(|| walk(&mut pyramid));
    assert!(first > 0, "nothing counted the first time over, so the count says nothing");
    let (again, ()) = crate::allocations::during(|| walk(&mut pyramid));
    assert_eq!(again, 0, "observing along the path again allocated");
}

/// What Cesium's preloadAncestors does, the demand does on its own: the line
/// over every demanded page is asked for, with no room for speculation, so a
/// page not yet come is drawn from the one just over it, never from the floor.
#[test]
fn the_line_over_every_demanded_page_is_asked_for() {
    let _serial = crate::serial();
    let o = the_observation();
    let mut pyramid = Pyramid::<PoiPage>::default();
    pyramid.observe(Some(&o), None, core::iter::empty(), &Unknown, &ByBounds { tau: Tau::FINEST }, &NoPrefetch, Tau::FINEST, 0);
    let asked: HashSet<Page> = pyramid.asks.keys().copied().collect();
    for &p in pyramid.demand() {
        for q in std::iter::successors(Some(p), |q| (q.z > TOP_Z).then(|| q.parent())).skip(1) {
            assert!(asked.contains(&q), "{q:?} over {p:?} is asked for");
        }
    }
}
