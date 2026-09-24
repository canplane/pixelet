// pxl-kernel/world/pyramid.test.rs

use super::*;
use pxl_spec::bake::poi_page::PoiPage;
use pxl_spec::space::pvf3::ivec;
use pxl_spec::space::pyramid::{cell_index, span_of, Cell, CHUNK_CELLS};
use window::ring_rect;

const WINDOW: std::ops::Range<u32> = 2..5;

fn anchor() -> IVec {
    ivec::gather(30_000, -17_000, 0)
}

fn area(c: Chunk) -> f64 {
    c.rect().2 * c.rect().2
}

fn meet(a: Chunk, b: Chunk) -> bool {
    let (a, b) = (a.rect(), b.rect());
    a.0 < b.0 + b.2 && b.0 < a.0 + a.2 && a.1 < b.1 + b.2 && b.1 < a.1 + a.2
}

fn inside(c: Chunk, at: IVec) -> bool {
    let (x0, y0, side) = c.rect();
    let (x, y) = (ivec::X(at) as f64, ivec::Y(at) as f64);
    x >= x0 && x < x0 + side && y >= y0 && y < y0 + side
}

/// Places scattered over the domain's ground, and every level's pages
/// counting them: a page of every level over that ground, empty or not.
fn places() -> (Vec<IVec>, Vec<(LevelPage, PoiPage)>) {
    let (rx, ry, side) = ring_rect(WINDOW.end - 1, anchor());
    let mut seed = 0x2545_f491u64;
    let mut next = |span: f64| {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((seed >> 33) as f64 / (1u64 << 31) as f64 * span) as i64
    };
    let at: Vec<IVec> = (0..400).map(|_| ivec::gather(rx as i64 + next(side), ry as i64 + next(side), 0)).collect();
    let mut pages = Vec::new();
    for level in 0..WINDOW.end {
        let span = span_of(level);
        let (x0, y0) = ((rx / span).floor() as i64, (ry / span).floor() as i64);
        let n = (side / span).ceil() as i64 + 1;
        for y in y0..y0 + n {
            for x in x0..x0 + n {
                let p = LevelPage::new(level, ivec::gather(x, y, 0));
                let cells = at.iter().map(|&a| Cell::of(level, a)).filter(|c| c.page() == p).map(|c| c.index());
                pages.push((p, PoiPage::of_places(cells)));
            }
        }
    }
    (at, pages)
}

fn count(pyramid: &Pyramid<PoiPage>, c: Chunk) -> u32 {
    let page = pyramid.get(c.page).expect("a drawn chunk's page is resident");
    (c.by..c.by + CHUNK_CELLS)
        .flat_map(|ty| (c.bx..c.bx + CHUNK_CELLS).map(move |tx| cell_index(tx, ty)))
        .map(|i| page.count(i))
        .sum()
}

/// The cover is the domain's ground cut once: its drawn pieces and its gaps
/// meet nowhere, together they are all of it, and every place on it is
/// counted exactly once — whatever is resident, at whatever levels.
#[test]
fn the_cover_and_its_gaps_partition_the_domain_and_count_every_place_once() {
    let (at, pages) = places();
    let d = domain(anchor(), WINDOW);
    let (_, _, side) = ring_rect(WINDOW.end - 1, anchor());
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
        let pieces: Vec<Chunk> = cover.drawn.iter().map(|(c, _)| *c).chain(cover.gaps.iter().copied()).collect();
        for (i, &a) in pieces.iter().enumerate() {
            assert!(pieces[i + 1..].iter().all(|&b| !meet(a, b)), "trial {trial}: {a:?} meets another piece");
        }
        let total: f64 = pieces.iter().map(|&c| area(c)).sum();
        assert_eq!(total, side * side, "trial {trial}: the pieces are the domain's ground");
        let drawn: u32 = cover.drawn.iter().map(|&(c, _)| count(&pyramid, c)).sum();
        let in_gaps = at.iter().filter(|&&a| cover.gaps.iter().any(|&g| inside(g, a))).count() as u32;
        assert_eq!(drawn + in_gaps, at.len() as u32, "trial {trial}: every place once");
    }
}

/// A coarse page with some of the finer pages under it draws the coarse
/// one, and not the fine ones beside it: a sibling group is drawn whole or
/// not at all.
#[test]
fn a_parent_gives_way_only_to_all_of_its_children() {
    let d = domain(anchor(), WINDOW);
    let parent = d.roots().into_iter().find(|&c| d.splits(c)).expect("the top ring holds ground wanted finer");
    let under = parent.children();
    let mut pyramid = Pyramid::<PoiPage>::default();
    pyramid.set(parent.page, PoiPage::default());
    pyramid.set(under[0].page, PoiPage::default());
    let drawn: Vec<Chunk> = pyramid.cover_of(&d).drawn.iter().map(|(c, _)| *c).collect();
    let here = |c: Chunk| drawn.contains(&c);
    // The children lie in one page, so a finer page resident is all four of
    // them; they give way only where each is drawn whole, which a split
    // child with nothing under it is not.
    assert!(here(parent) || under.iter().all(|&k| here(k) || k.children().iter().all(|&g| here(g))));
    assert!(!(here(parent) && under.iter().any(|&k| here(k))), "never a parent and its child");
}

/// The cover and the folds are functions of the resident set, and what
/// tells them it changed is one number every set and clear moves.
#[test]
fn every_arrival_and_departure_moves_the_resident_revision() {
    let mut pyramid = Pyramid::<PoiPage>::default();
    let p = LevelPage::new(0, ivec::gather(4, 9, 0));
    let before = pyramid.revision();
    pyramid.set(p, PoiPage::default());
    let arrived = pyramid.revision();
    assert_ne!(arrived, before);
    pyramid.clear(p);
    assert_ne!(pyramid.revision(), arrived);
}
