// pxl-kernel/world/pyramid.test.rs

use super::*;
use domain::{domain_for, Eye};
use pxl_spec::bake::poi_page::{Place, PoiPage, POI_LATTICE};
use pxl_spec::space::hierarchy::share;
use pxl_spec::space::pvf3::{ivec, Pvf3};
use pxl_spec::space::pyramid::GROUND;

/// How high the eye stands over flat ground: where the rule wants level 2
/// under it.
const HEIGHT: f64 = 1000.0;

fn anchor() -> IVec {
    ivec::gather(30_000, -17_000, 0)
}

fn the_domain() -> Domain {
    let eye = Eye {
        at: [ivec::X(anchor()) as f64, ivec::Y(anchor()) as f64, HEIGHT],
        foot: anchor(),
    };
    domain_for(&eye, HEIGHT, None)
}

/// The chunks the rule draws of a domain where everything is resident.
fn drawn_by_rule(d: &Domain) -> Vec<Chunk> {
    let mut out = Vec::new();
    let mut open = d.roots().to_vec();
    while let Some(c) = open.pop() {
        if d.splits(c) {
            open.extend(c.children());
        } else {
            out.push(c);
        }
    }
    out
}

/// The ground a domain draws, as the square around it and its area.
fn ground_of(d: &Domain) -> ((f64, f64, f64), f64) {
    let g: Vec<_> = d.roots().iter().map(|c| c.rect()).collect();
    let (x0, y0) = g.iter().fold((f64::MAX, f64::MAX), |(x, y), r| (x.min(r.0), y.min(r.1)));
    let x1 = g.iter().map(|r| r.0 + r.2).fold(f64::MIN, f64::max);
    let y1 = g.iter().map(|r| r.1 + r.2).fold(f64::MIN, f64::max);
    ((x0, y0, (x1 - x0).max(y1 - y0)), g.iter().map(|r| r.2 * r.2).sum())
}

fn area(c: Chunk) -> f64 {
    c.rect().2 * c.rect().2
}

fn meet(a: Chunk, b: Chunk) -> bool {
    share(a.node(), b.node())
}

/// Whether no two pieces share ground. Pieces of the pyramid meet only where
/// one is the other or lies under it.
fn disjoint(pieces: &[Chunk]) -> bool {
    let set: std::collections::HashSet<Chunk> = pieces.iter().copied().collect();
    set.len() == pieces.len() && pieces.iter().all(|c| !c.ancestors().any(|a| set.contains(&a)))
}

fn inside(c: Chunk, at: IVec) -> bool {
    let (x0, y0, side) = c.rect();
    let (x, y) = (ivec::X(at) as f64, ivec::Y(at) as f64);
    x >= x0 && x < x0 + side && y >= y0 && y < y0 + side
}

/// Places scattered over the domain's ground, and the pages counting them:
/// every page the domain wants and the four under each, empty or not, which
/// is every page a cover of it can draw from.
fn places() -> (Vec<IVec>, Vec<(LevelPage, PoiPage)>) {
    let d = the_domain();
    let ((rx, ry, side), _) = ground_of(&d);
    let ground = d.roots();
    let mut seed = 0x2545_f491u64;
    let mut next = |span: f64| {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((seed >> 33) as f64 / (1u64 << 31) as f64 * span) as i64
    };
    // On the ground the domain draws, which is a disc of chunks and not the
    // square around it.
    let at: Vec<IVec> = (0..800)
        .map(|_| ivec::gather(rx as i64 + next(side), ry as i64 + next(side), 0))
        .filter(|&a| ground.iter().any(|&c| inside(c, a)))
        .collect();
    let mut wanted: Vec<LevelPage> = d.pages().map(|(p, _)| p).collect();
    wanted.extend(d.pages().filter(|(p, _)| p.level > 0).flat_map(|(p, _)| p.children()));
    wanted.sort_unstable();
    wanted.dedup();
    let pages = wanted
        .into_iter()
        .map(|p| {
            let places = at
                .iter()
                .map(|&a| Pvf3 { i: a, ..Default::default() })
                .filter(|a| POI_LATTICE.cell_of(p.level, a).page() == p)
                .map(|a| (a, Place { name: "", kind: 0 }));
            (p, PoiPage::of_places(p.level, places))
        })
        .collect();
    (at, pages)
}

/// The places a drawn chunk counts: those of its source page — its own, or
/// the one over it that settles it — whose cell stands on the chunk's ground.
fn count(pyramid: &Pyramid<PoiPage>, c: Chunk) -> u32 {
    let src = pyramid.source(c.page()).expect("a drawn chunk has a page that draws it");
    let page = pyramid.get(src).expect("a source is resident");
    page.cells
        .iter()
        .filter(|x| Chunk::of(c.level(), &POI_LATTICE.cell_at(src, x.cell).inner_at(GROUND, 0, x.at).mid()) == c)
        .map(|x| x.count)
        .sum()
}

/// The cover is the domain's ground cut once: its drawn pieces and its gaps
/// meet nowhere, together they are all of it, and every place on it is
/// counted exactly once — whatever is resident, at whatever levels.
#[test]
fn the_cover_and_its_gaps_partition_the_domain_and_count_every_place_once() {
    // The domain reads the terrain's and the objects' heights, which are
    // globals.
    let _serial = crate::global::serial();
    let (at, pages) = places();
    let d = the_domain();
    let (_, whole) = ground_of(&d);
    let mut seed = 7u64;
    for trial in 0..12 {
        let mut pyramid = Pyramid::<PoiPage>::default();
        for (p, page) in &pages {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            // Denser as the trials go on, from nearly nothing to nearly all.
            if (seed >> 40) % 12 < trial {
                pyramid.set(*p, page.clone());
            }
        }
        let cover = pyramid.cover_of(&d);
        let pieces: Vec<Chunk> = cover.drawn.iter().copied().chain(cover.gaps.iter().copied()).collect();
        assert!(disjoint(&pieces), "trial {trial}: two pieces meet");
        let total: f64 = pieces.iter().map(|&c| area(c)).sum();
        assert_eq!(total, whole, "trial {trial}: the pieces are the domain's ground");
        let drawn: u32 = cover.drawn.iter().map(|&c| count(&pyramid, c)).sum();
        let in_gaps = at.iter().filter(|&&a| cover.gaps.iter().any(|&g| inside(g, a))).count() as u32;
        assert_eq!(drawn + in_gaps, at.len() as u32, "trial {trial}: every place once");
    }
}

/// A coarse page with some of the finer pages under it draws the coarse
/// one, and not the fine ones beside it: a sibling group is drawn whole or
/// not at all.
#[test]
fn a_parent_gives_way_only_to_all_of_its_children() {
    // The domain reads the terrain's and the objects' heights, which are
    // globals.
    let _serial = crate::global::serial();
    let d = the_domain();
    let parent = d.roots().iter().copied().find(|&c| d.splits(c)).expect("the top level holds ground wanted finer");
    let under = parent.children();
    let mut pyramid = Pyramid::<PoiPage>::default();
    pyramid.set(parent.page(), PoiPage::default());
    pyramid.set(under[0].page(), PoiPage::default());
    let drawn: Vec<Chunk> = pyramid.cover_of(&d).drawn;
    let here = |c: Chunk| drawn.contains(&c);
    // The children lie in one page, so a finer page resident is all four of
    // them; they give way only where each is drawn whole, which a split
    // child with nothing under it is not.
    assert!(here(parent) || under.iter().all(|&k| here(k) || k.children().iter().all(|&g| here(g))));
    assert!(!(here(parent) && under.iter().any(|&k| here(k))), "never a parent and its child");
}

/// Where nothing over a wanted chunk is resident, the pages under it stand
/// in however far down they are.
#[test]
fn a_stand_in_reaches_as_far_down_as_there_are_pages() {
    // The domain reads the terrain's and the objects' heights, which are
    // globals.
    let _serial = crate::global::serial();
    let d = the_domain();
    let c = drawn_by_rule(&d).into_iter().find(|c| c.level() >= 2).expect("ground drawn two levels over z16");
    let grand: Vec<Chunk> = c.children().iter().flat_map(|k| k.children()).collect();
    let mut pyramid = Pyramid::<PoiPage>::default();
    for g in &grand {
        pyramid.set(g.page(), PoiPage::default());
    }
    let cover = pyramid.cover_of(&d);
    assert!(grand.iter().all(|g| cover.drawn.contains(g)), "two levels down, all sixteen");
    assert!(!cover.gaps.iter().any(|&g| meet(g, c)));
}

/// The terrain's mesh draws its cover balanced: no two chunks that meet
/// more than a level apart, never more than a level finer than the view
/// wants, and still the domain's ground exactly once — what it cannot draw
/// is a gap.
#[test]
fn the_terrain_draws_its_cover_balanced_and_still_whole() {
    // The domain reads the terrain's and the objects' heights, which are
    // globals.
    let _serial = crate::global::serial();
    let (_, pages) = places();
    let d = the_domain();
    let (_, whole) = ground_of(&d);
    let mut seed = 11u64;
    for trial in 0..12 {
        let mut pyramid = Pyramid::<PoiPage>::default();
        for (p, page) in &pages {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            if (seed >> 40) % 12 < trial {
                pyramid.set(*p, page.clone());
            }
        }
        let cover = crate::layer::terrain::present::mesh(pyramid.cover_of(&d), &d, |c| pyramid.get(c.page()).is_some());
        for &c in &cover.drawn {
            let want = d.wanted_over(c);
            assert!(c.level() + 1 >= want.level(), "trial {trial}: {c:?} finer than the view wants");
            for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                let next = c.beside(dx, dy);
                if let Some(o) = std::iter::once(next).chain(next.ancestors()).find(|a| cover.drawn.contains(a)) {
                    assert!(o.level() < c.level() + 2, "trial {trial}: {c:?} meets {o:?}");
                }
            }
        }
        let pieces: Vec<Chunk> = cover.drawn.iter().chain(&cover.gaps).copied().collect();
        assert!(disjoint(&pieces), "trial {trial}: two pieces meet");
        assert_eq!(pieces.iter().map(|&c| area(c)).sum::<f64>(), whole, "trial {trial}");
    }
}

/// What the terrain leaves open is what it asks for: a page under a gap the
/// presentation made, and nothing resident or foldable, is coverage.
#[test]
fn a_gap_the_mesh_leaves_is_fetched_as_coverage() {
    // The domain reads the terrain's and the objects' heights, which are
    // globals.
    let _serial = crate::global::serial();
    let (_, pages) = places();
    let d = the_domain();
    let mut pyramid = Pyramid::<PoiPage>::default();
    for (p, page) in pages.iter().filter(|(p, _)| p.level == 2 || (p.level == 4 && ivec::X(p.page) % 2 == 0)) {
        pyramid.set(*p, page.clone());
    }
    pyramid.cover = crate::layer::terrain::present::mesh(pyramid.cover_of(&d), &d, |c| pyramid.get(c.page()).is_some());
    assert!(!pyramid.cover.gaps.is_empty(), "a residency with ground left open");
    for g in pyramid.cover.gaps.clone() {
        if pyramid.get(g.page()).is_none() && !pyramid.children_resident(g.page()) {
            assert_eq!(pyramid.resolve(g.page()), Resolved::Missing, "{g:?}");
        }
    }
}

/// The pinned page keeps its record through every page a view wants
/// arriving, every one of them placed.
#[test]
fn the_pinned_page_keeps_its_record_and_every_wanted_page_still_fits() {
    let _serial = crate::global::serial();
    let mut pyramid = Pyramid::<PoiPage>::default();
    let far = |i: u32| LevelPage::new(0, ivec::gather(1_000_000 + i64::from(i), 0, 0));
    for i in 0..RECORDS {
        let record = pyramid.record_for(far(i), Some(anchor())).expect("a free record");
        pyramid.take(far(i), record, PoiPage::default());
    }
    pyramid.pin(Some(far(0)));
    let d = the_domain();
    let wanted: Vec<LevelPage> = d.pages().map(|(p, _)| p).collect();
    pyramid.domain = Some(d);
    for p in wanted {
        let record = pyramid.record_for(p, Some(anchor())).expect("room for every page the view wants");
        pyramid.take(p, record, PoiPage::default());
    }
    assert!(pyramid.get(far(0)).is_some());
}

/// A count folds from its four children and reads nothing around them.
#[test]
fn a_count_folds_from_its_four_children_alone() {
    let p = LevelPage::new(3, ivec::gather(5, -2, 0));
    assert_eq!(PoiPage::sources(p), p.children().to_vec());
    let k = p.children()[0];
    let child = PoiPage::of_places(
        k.level,
        [
            (POI_LATTICE.cell_base(k, 0, 0), Place { name: "", kind: 0 }),
            (POI_LATTICE.cell_base(k, 1, 0), Place { name: "", kind: 0 }),
        ],
    );
    let folded = PoiPage::fold_page(p, |q| p.children().contains(&q).then_some(&child)).expect("the four are enough");
    assert_eq!(folded.cells.iter().map(|c| c.count).sum::<u32>(), 8);
}

/// A stand-in too deep for the terrain opens its own branch a level under
/// the ground it stands in for, and the stand-ins beside it stay drawn.
#[test]
fn a_stand_in_too_deep_opens_its_branch_and_not_its_neighbours() {
    // The domain reads the terrain's and the objects' heights, which are
    // globals.
    let _serial = crate::global::serial();
    let d = the_domain();
    let t = drawn_by_rule(&d).into_iter().find(|c| c.level() >= 3).expect("ground drawn three levels over z16");
    let [a, b, deep, gap] = t.children();
    let cover = Cover {
        drawn: [a, b].into_iter().chain(deep.children().iter().flat_map(|k| k.children())).collect(),
        gaps: vec![gap],
    };
    let shown = crate::layer::terrain::present::mesh(cover, &d, |_| false);
    assert!(shown.drawn.contains(&a) && shown.drawn.contains(&b), "the stand-ins beside it stay");
    assert!(shown.gaps.contains(&deep) && shown.gaps.contains(&gap), "its own branch opens, and nothing wider");
    assert!(!shown.gaps.contains(&t));
}

/// A page with a crowd over each of its children, so it settles none of them.
fn crowded(p: LevelPage) -> PoiPage {
    let at = |k: LevelPage| POI_LATTICE.cell_base(k, 0, 0);
    PoiPage::of_places(
        p.level,
        p.children().into_iter().flat_map(|k| [(at(k), Place { name: "a", kind: 0 }), (at(k), Place { name: "b", kind: 0 })]),
    )
}

/// Where a page says all of the ground under it, the tree stops there: the
/// roots are drawn from their own pages, nothing under them is a gap, and
/// nothing under them is asked for.
#[test]
fn ground_a_page_settles_is_drawn_from_it_and_never_asked_for() {
    let _serial = crate::global::serial();
    let d = the_domain();
    let mut pyramid = Pyramid::<PoiPage>::default();
    for r in d.roots() {
        pyramid.set(r.page(), PoiPage::default());
    }
    let cover = pyramid.cover_of(&d);
    assert!(cover.gaps.is_empty(), "{} gaps", cover.gaps.len());
    assert!(disjoint(&cover.drawn));
    for &c in &cover.drawn {
        assert!(pyramid.source(c.page()).is_some_and(|s| s.level == TOP_LEVEL), "{c:?} is drawn from the page at the top");
    }
    for (p, _) in d.pages().filter(|(p, _)| p.level < TOP_LEVEL) {
        assert!(pyramid.settled(p), "{p:?} is settled, so never asked for");
    }
}

/// Where the finer pages have not come, the pages over them stand in: with
/// only the wanted pages from some level up resident, every root's ground
/// is drawn, once, and nothing is a gap.
#[test]
fn ground_whose_page_has_not_come_is_stood_in_for_by_the_page_over_it() {
    let _serial = crate::global::serial();
    let d = the_domain();
    let mut pyramid = Pyramid::<PoiPage>::default();
    for (p, _) in d.pages().filter(|(p, _)| p.level >= 5) {
        pyramid.set(p, crowded(p));
    }
    let cover = pyramid.cover_of(&d);
    assert!(cover.gaps.is_empty(), "{} gaps", cover.gaps.len());
    assert!(disjoint(&cover.drawn));
    assert!(cover.drawn.iter().all(|c| c.level() >= 5));
    let (_, whole) = ground_of(&d);
    assert_eq!(cover.drawn.iter().map(|&c| area(c)).sum::<f64>(), whole);
}
