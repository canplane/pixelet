// pxl-kernel/world/pyramid/cover.test.rs

use super::*;
use crate::io::demand::{Ask, Why};
use crate::layer::environment::fog::reach_at;
use crate::world::observation::Order;
use pxl_spec::bake::poi_page::{Place, PoiPage, POI_LATTICE};
use pxl_spec::space::lod::{fault_radius, Tau};
use pxl_spec::space::pvf3::ivec::IVec;
use pxl_spec::space::pvf3::{ivec, Pvf3};
use pxl_spec::space::pyramid::{LevelPage, GROUND, TOP_LEVEL};

use crate::world::fixture::{self, eye, World};

/// Where the rule wants level 2 under the eye.
const HEIGHT: f64 = 1000.0;

fn anchor() -> IVec {
    ivec::gather(30_000, -17_000, 0)
}

const LAND: f64 = 10.5;

fn the_domain() -> Domain {
    let world = World::flat(LAND);
    let at = [ivec::X(anchor()) as f64, ivec::Y(anchor()) as f64, LAND + HEIGHT];
    fixture::domain(eye(at, Some([0.05, 0.02, -1.0]), reach_at(HEIGHT).min(fault_radius(TOP_LEVEL)), LAND), &[Tau::FINEST], &world)
}

fn read_by_rule(d: &Domain) -> Vec<LevelPage> {
    let mut out: Vec<LevelPage> = d
        .pieces()
        .iter()
        .flat_map(|c| std::iter::successors(Some(c.page()), |p| (p.level < TOP_LEVEL).then(|| p.parent())))
        .collect();
    out.sort_unstable();
    out.dedup();
    out
}

fn ground_of(d: &Domain) -> ((f64, f64, f64), f64) {
    let g: Vec<_> = d.roots().iter().map(|c| c.rect()).collect();
    let (x0, y0) = g.iter().fold((f64::MAX, f64::MAX), |(x, y), r| (x.min(r.0), y.min(r.1)));
    let x1 = g.iter().map(|r| r.0 + r.2).fold(f64::MIN, f64::max);
    let y1 = g.iter().map(|r| r.1 + r.2).fold(f64::MIN, f64::max);
    ((x0, y0, (x1 - x0).max(y1 - y0)), g.iter().map(|r| r.2 * r.2).sum())
}

/// Asked for first: residency keeps an `ABSENT` page only while something asks.
fn hold(pyramid: &mut Pyramid<PoiPage>, pages: impl IntoIterator<Item = (LevelPage, PoiPage)>) {
    let pages: Vec<(LevelPage, PoiPage)> = pages.into_iter().collect();
    let aside = Ask { why: Why::Fault, order: Order::LAST };
    pyramid.paging.ask(&pages.iter().map(|&(p, _)| (p, aside)).collect());
    for (p, page) in pages {
        pyramid.set(p, page);
    }
}

fn area(c: Chunk) -> f64 {
    c.rect().2 * c.rect().2
}

/// Pieces of the pyramid meet only where one is or lies under the other.
fn disjoint(pieces: &[Chunk]) -> bool {
    let set: std::collections::HashSet<Chunk> = pieces.iter().copied().collect();
    set.len() == pieces.len() && pieces.iter().all(|c| !c.ancestors().any(|a| set.contains(&a)))
}

fn inside(c: Chunk, at: IVec) -> bool {
    let (x0, y0, side) = c.rect();
    let (x, y) = (ivec::X(at) as f64, ivec::Y(at) as f64);
    x >= x0 && x < x0 + side && y >= y0 && y < y0 + side
}

/// Every page the domain reads and the four under each: every page a cover
/// can draw from.
fn places() -> (Vec<IVec>, Vec<(LevelPage, PoiPage)>) {
    let d = the_domain();
    let ((rx, ry, side), _) = ground_of(&d);
    let ground = d.roots();
    let mut seed = 0x2545_f491u64;
    let mut next = |span: f64| {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((seed >> 33) as f64 / (1u64 << 31) as f64 * span) as i64
    };
    // The domain's ground is a disc of chunks, not the square around it.
    let at: Vec<IVec> = (0..800)
        .map(|_| ivec::gather(rx as i64 + next(side), ry as i64 + next(side), 0))
        .filter(|&a| ground.iter().any(|&c| inside(c, a)))
        .collect();
    let mut wanted = read_by_rule(&d);
    wanted.extend(read_by_rule(&d).into_iter().filter(|p| p.level > 0).flat_map(|p| p.children()));
    wanted.sort_unstable();
    wanted.dedup();
    let pages = wanted
        .into_iter()
        .map(|p| {
            let places = at
                .iter()
                .map(|&a| Pvf3 { i: a, ..Default::default() })
                .filter(|a| POI_LATTICE.cell_of(p.level, a).page() == p)
                .map(|a| (a, Place { name: "p", kind: 0, id: a.i }));
            (p, PoiPage::of_places(p.level, places, true))
        })
        .collect();
    (at, pages)
}

/// None where only a non-settling page over the chunk is resident: it draws
/// the ground but does not say what stands there.
fn count(pyramid: &Pyramid<PoiPage>, c: Chunk) -> Option<u32> {
    let src = pyramid.paging().source(c.page())?;
    let page = pyramid.paging().get(src).expect("a source is resident");
    page.cells
        .iter()
        .filter(|x| {
            let cell = POI_LATTICE.cell_at(src, x.cell);
            let at = page.shown(x).next().map_or(cell.mid(), |(_, _, at)| cell.inner_at(GROUND, 0, at).mid());
            Chunk::of(c.level(), &at) == c
        })
        .map(|x| x.count)
        .sum::<u32>()
        .into()
}

/// Drawn pieces and gaps partition the domain, and every place is counted
/// exactly once, whatever is resident.
#[test]
fn the_cover_and_its_gaps_partition_the_domain_and_count_every_place_once() {
    let _serial = crate::global::serial();
    let (at, pages) = places();
    let d = the_domain();
    let (_, whole) = ground_of(&d);
    let mut seed = 7u64;
    for trial in 0..12 {
        let mut pyramid = Pyramid::<PoiPage>::default();
        let mut held = Vec::new();
        for (p, page) in &pages {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            if (seed >> 40) % 12 < trial {
                held.push((*p, page.clone()));
            }
        }
        hold(&mut pyramid, held);
        let cover = cover_of(&d, &pyramid);
        let pieces: Vec<Chunk> = cover.drawn.iter().copied().chain(cover.gaps.iter().copied()).collect();
        assert!(disjoint(&pieces), "trial {trial}: two pieces meet");
        let total: f64 = pieces.iter().map(|&c| area(c)).sum();
        assert_eq!(total, whole, "trial {trial}: the pieces are the domain's ground");
        let drawn: u32 = cover.drawn.iter().filter_map(|&c| count(&pyramid, c)).sum();
        let unsaid: Vec<Chunk> = cover.gaps.iter().copied().chain(cover.drawn.iter().copied().filter(|&c| count(&pyramid, c).is_none())).collect();
        let elsewhere = at.iter().filter(|&&a| unsaid.iter().any(|&g| inside(g, a))).count() as u32;
        assert_eq!(drawn + elsewhere, at.len() as u32, "trial {trial}: every place once");
    }
}

#[test]
fn the_cover_is_the_domain_whole_whatever_is_held() {
    let _serial = crate::global::serial();
    let (_, pages) = places();
    let d = the_domain();
    let (_, whole) = ground_of(&d);
    let mut seed = 11u64;
    for trial in 0..12 {
        let mut pyramid = Pyramid::<PoiPage>::default();
        let mut held = Vec::new();
        for (p, page) in &pages {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            if (seed >> 40) % 12 < trial {
                held.push((*p, page.clone()));
            }
        }
        hold(&mut pyramid, held);
        let cover = cover_of(&d, &pyramid);
        let pieces: Vec<Chunk> = cover.drawn.iter().chain(&cover.gaps).copied().collect();
        assert!(disjoint(&pieces), "trial {trial}: two pieces meet");
        assert_eq!(pieces.iter().map(|&c| area(c)).sum::<f64>(), whole, "trial {trial}");
    }
}

fn crowded(p: LevelPage) -> PoiPage {
    let at = |k: LevelPage| POI_LATTICE.cell_base(k, 0, 0);
    PoiPage::of_places(
        p.level,
        p.children()
            .into_iter()
            .flat_map(|k| [(at(k), Place { name: "a", kind: 0, id: 1 }), (at(k), Place { name: "b", kind: 0, id: 1 })]),
        true,
    )
}

#[test]
fn ground_a_page_settles_is_drawn_from_it_and_never_asked_for() {
    let _serial = crate::global::serial();
    let d = the_domain();
    let mut pyramid = Pyramid::<PoiPage>::default();
    hold(&mut pyramid, d.roots().iter().map(|r| (r.page(), PoiPage::default())));
    let cover = cover_of(&d, &pyramid);
    assert!(cover.gaps.is_empty(), "{} gaps", cover.gaps.len());
    assert!(disjoint(&cover.drawn));
    for &c in &cover.drawn {
        assert!(pyramid.paging().source(c.page()).is_some_and(|s| s.level == TOP_LEVEL), "{c:?} is drawn from the page at the top");
    }
    for p in read_by_rule(&d).into_iter().filter(|p| p.level < TOP_LEVEL) {
        assert!(pyramid.paging().settled(p), "{p:?} is settled, so never asked for");
    }
}

/// A piece never waits on its siblings: it is drawn from its page or the
/// nearest page over it.
#[test]
fn ground_whose_page_has_not_come_is_drawn_in_its_place_from_the_page_over_it() {
    let _serial = crate::global::serial();
    let d = the_domain();
    let mut pyramid = Pyramid::<PoiPage>::default();
    hold(&mut pyramid, read_by_rule(&d).into_iter().filter(|p| p.level >= 5).map(|p| (p, crowded(p))));
    let cover = cover_of(&d, &pyramid);
    assert!(cover.gaps.is_empty(), "{} gaps", cover.gaps.len());
    let mut wanted = d.pieces().to_vec();
    let mut drawn = cover.drawn.clone();
    wanted.sort_unstable();
    drawn.sort_unstable();
    assert_eq!(drawn, wanted, "the chunks the rule wants, each in its own place");
    for &c in &cover.drawn {
        let from = pyramid.paging().drawn_from(c.page()).expect("drawn from a page");
        assert!(from.level >= 5.max(c.level()), "{c:?} drawn from {from:?}");
    }
}
