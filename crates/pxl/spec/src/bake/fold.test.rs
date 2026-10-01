// pxl-spec/bake/fold.test.rs

use super::*;

use crate::bake::format::GroundChannelKey;
use crate::cell::cube::cube_types;

const SPAN: usize = PAGE_CELLS as usize;
const PADDED: usize = ELE_PADDED_SPAN as usize;

#[test]
fn an_area_keeps_what_fills_half_its_four_and_ties_alternate_corners() {
    assert_eq!(majority([7, 7, 7, 1], 0), 7);
    assert_eq!(majority([7, 7, 1, 2], 0), 7);
    assert_eq!(majority([1, 7, 2, 7], 1), 7);
    // No half, or two halves: the tie child, the first at an odd level and
    // the last at an even one.
    assert_eq!(majority([7, 1, 2, 3], 1), 7);
    assert_eq!(majority([7, 1, 2, 3], 2), 3);
    assert_eq!(majority([7, 7, 1, 1], 2), 1);
}

/// A page whose transport channel is `f` of the cell, every other channel
/// void: the fold votes a channel at a time, so one carries the whole test.
/// The values are numbers the channel gives out, which is what a cell can
/// hold (bake/pack.rs).
fn plane(f: impl Fn(usize, usize) -> u8) -> Ground {
    Ground {
        cells: (0..SPAN * SPAN).map(|i| with_channel(0, GroundChannelKey::TRANSPORT, f(i % SPAN, i / SPAN))).collect::<Vec<_>>().into(),
    }
}

/// The parent's transport numbers.
fn fold(children: [&Ground; 4], level: u32) -> Vec<u8> {
    let out = fold_ground(children, level);
    (0..SPAN * SPAN).map(|i| out.channel(i, GroundChannelKey::TRANSPORT)).collect()
}

#[test]
fn a_uniform_page_folds_to_itself() {
    let p = plane(|_, _| cube_types::transport::rail_subway);
    assert!(fold([&p, &p, &p, &p], 1).iter().all(|&v| v == cube_types::transport::rail_subway));
}

#[test]
fn a_one_cell_line_is_a_tie_and_a_dot_goes() {
    // A one-cell road down x = 10: every parent cell it touches holds it in
    // two of four children — a tie, taken by the corner child, which on level
    // 1 is the south-west one and so sees the road only where x is even.
    let road = plane(|x, _| if x == 10 { cube_types::transport::road_primary } else { 0 });
    let empty = plane(|_, _| 0);
    let out = fold([&road, &empty, &empty, &empty], 1);
    assert_eq!(out[5], cube_types::transport::road_primary);
    let odd = plane(|x, _| if x == 11 { cube_types::transport::road_primary } else { 0 });
    let out = fold([&odd, &empty, &empty, &empty], 1);
    assert_eq!(out[5], 0);
    // A single cell is one of four and always goes.
    let dot = plane(|x, y| if (x, y) == (10, 10) { 3 } else { 0 });
    assert!(fold([&dot, &empty, &empty, &empty], 1).iter().all(|&v| v == 0));
}

#[test]
fn a_feature_two_cells_wide_survives_a_fold() {
    let road = plane(|x, _| if x == 10 || x == 11 { 3 } else { 0 });
    let empty = plane(|_, _| 0);
    let out = fold([&road, &empty, &empty, &empty], 1);
    assert!((0..SPAN / 2).all(|py| out[py * SPAN + 5] == 3));
}

#[test]
fn ties_alternate_their_corner_by_level() {
    // Columns tie every block 2:2 with the two corners on opposite sides, so
    // the parent is whichever corner the level names: (0, 0) on odd levels,
    // (1, 1) on even ones.
    let columns = plane(|x, _| if x % 2 == 0 { cube_types::transport::road_default } else { cube_types::transport::road_motorway });
    assert!(fold([&columns, &columns, &columns, &columns], 1).iter().all(|&v| v == cube_types::transport::road_default));
    assert!(fold([&columns, &columns, &columns, &columns], 2).iter().all(|&v| v == cube_types::transport::road_motorway));
}

#[test]
fn each_child_lands_in_its_own_quarter() {
    let q = [plane(|_, _| 1), plane(|_, _| 2), plane(|_, _| 3), plane(|_, _| 4)]; // road_default..road_secondary
    let out = fold([&q[0], &q[1], &q[2], &q[3]], 1);
    let half = SPAN / 2;
    assert_eq!(out[0], 1);
    assert_eq!(out[half], 2);
    assert_eq!(out[half * SPAN], 3);
    assert_eq!(out[half * SPAN + half], 4);
}

/// A padded elevation grid whose sample at inner index (i, j) is `f` of that
/// sample's index across the whole world, page `q` being `q · PAGE_ELE_SPAN`
/// samples along.
fn grid(qx: i64, qy: i64, f: impl Fn(i64, i64) -> i64) -> Vec<i16> {
    let (inner, pad) = (PAGE_ELE_SPAN as i64, ELE_PAD as i64);
    (0..PADDED * PADDED)
        .map(|s| {
            let (ix, iy) = ((s % PADDED) as i64 - pad, (s / PADDED) as i64 - pad);
            f(qx * inner + ix, qy * inner + iy) as i16
        })
        .collect()
}

fn fold_grids(f: impl Fn(i64, i64) -> i64 + Copy) -> Vec<i16> {
    fold_grids_held(f, |_| true)
}

/// The same fold with only the pages `held` says are held, as the client
/// folds from what it has.
fn fold_grids_held(f: impl Fn(i64, i64) -> i64 + Copy, held: impl Fn(usize) -> bool) -> Vec<i16> {
    let grids: Vec<Vec<i16>> = (0..16).map(|k| grid(k % 4 - 1, k / 4 - 1, f)).collect();
    let refs: [Option<&[i16]>; 16] = std::array::from_fn(|k| held(k).then(|| grids[k].as_slice()));
    let mut out = vec![0; PADDED * PADDED];
    fold_ele(refs, &mut out);
    out
}

/// What the client loses by folding a page from the four under it rather
/// than the sixteen a bake has: the page itself and the padding sample next
/// to it are the bake's, to the bit, and the outermost padding sample — the
/// fourth tap of a bicubic read at the page's own rim — is the block's edge
/// extended instead of the neighbor's.
#[test]
fn elevation_folded_from_the_four_children_differs_only_in_its_outermost_padding() {
    let f = |x: i64, y: i64| 7 * x - 3 * y + 11;
    let whole = fold_grids(f);
    let four = fold_grids_held(f, |k| [5, 6, 9, 10].contains(&k));
    let rim = |s: usize| s < ELE_PAD as usize - 1 || s >= PADDED - (ELE_PAD as usize - 1);
    let mut outer = 0;
    for sy in 0..PADDED {
        for sx in 0..PADDED {
            let at = sy * PADDED + sx;
            if rim(sx) || rim(sy) {
                outer += (whole[at] != four[at]) as u32;
            } else {
                assert_eq!(whole[at], four[at], "({sx}, {sy}) is the bake's");
            }
        }
    }
    assert!(outer > 0, "a ramp does run off the block's edge");
}

#[test]
fn elevation_keeps_the_center_sample_convention() {
    // Child sample m sits at m + 0.5; the parent's sample i covers children 2i
    // and 2i + 1, whose centers average to 2i + 1 — the parent's own center in
    // child samples. So a ramp of height 2m folds to 4i + 1 exactly.
    let out = fold_grids(|mx, _| 2 * mx);
    let pad = ELE_PAD as i64;
    for (sx, &v) in out.iter().take(PADDED).enumerate() {
        let i = sx as i64 - pad;
        assert_eq!(v as i64, 4 * i + 1, "sample {i}");
    }
}

#[test]
fn elevation_reads_the_skirt_from_the_neighbors() {
    // The parent's outermost padding samples lie four child samples past the
    // block, which only the neighboring pages hold. Heights that differ by
    // page show which page each came from.
    let out = fold_grids(|mx, my| 100 * mx.div_euclid(PAGE_ELE_SPAN as i64) + my.div_euclid(PAGE_ELE_SPAN as i64));
    assert_eq!(out[0], -101);
    assert_eq!(out[PADDED - 1], 200 - 1);
    assert_eq!(out[(PADDED - 1) * PADDED], -100 + 2);
}

#[test]
fn elevation_rounds_its_halves_to_even() {
    // Four integers average to a half a quarter of the time; rounding half up
    // would drift every fold upward.
    let lows = fold_grids(|mx, _| mx.rem_euclid(2));
    assert!(lows.iter().all(|&v| v == 0), "0, 1, 0, 1 average to 0.5 and round to 0");
    let highs = fold_grids(|mx, _| 1 + mx.rem_euclid(2) * 2);
    assert!(highs.iter().all(|&v| v == 2), "1, 3, 1, 3 average to 2");
    let halves = fold_grids(|mx, _| 2 + mx.rem_euclid(2));
    assert!(halves.iter().all(|&v| v == 2), "2, 3, 2, 3 average to 2.5 and round to 2");
}

#[test]
fn a_volume_keeps_what_fills_half_its_eight_and_loses_what_fills_less() {
    let (a, b, empty) = (5, 9, 0);
    // A lone cube is an eighth: it goes.
    assert_eq!(majority([a, empty, empty, empty, empty, empty, empty, empty], 1), empty);
    // A slab — the lower four — is half: it stays.
    assert_eq!(majority([a, a, a, a, empty, empty, empty, empty], 1), a);
    // Five of eight win whatever the tie child holds.
    assert_eq!(majority([empty, a, a, a, a, a, empty, empty], 2), a);
    // Two halves tie, and the tie alternates by level between the diagonal's ends.
    let halves = [a, b, a, b, b, a, b, a];
    assert_eq!(majority(halves, 1), halves[0]);
    assert_eq!(majority(halves, 2), halves[7]);
    assert_eq!(majority(halves, -1), halves[0], "a level below the cell alternates the same way");
}

// ---- the index lane ----

/// A small generator, so a page's ground and relief are rough everywhere and
/// the same every run.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn below(&mut self, n: u64) -> i64 {
        (self.next() % n) as i64
    }
}

/// A leaf of rough relief — steps and spikes, which the interpolation
/// overshoots most — over ground part water, which sits a step lower.
fn rough_leaf(rng: &mut Rng) -> Terrain {
    let ele = (0..PADDED * PADDED).map(|_| if rng.below(5) == 0 { rng.below(4000) - 500 } else { rng.below(40) } as i16).collect();
    let cells = (0..SPAN * SPAN)
        .map(|_| {
            let land = if rng.below(3) == 0 { cube_types::land::bare } else { cube_types::land::green_light };
            let water = if rng.below(4) == 0 { cube_types::water::river } else { 0 };
            with_channel(with_channel(0, GroundChannelKey::LAND, land), GroundChannelKey::WATER, water)
        })
        .collect::<Vec<_>>();
    Terrain::leaf(ele, Ground { cells: cells.into() })
}

/// The open ocean, as the bake makes it: flat water at sea level, no page.
fn zero_page() -> Terrain {
    let cells = vec![with_channel(0, GroundChannelKey::LAND, cube_types::land::sea); SPAN * SPAN];
    let mut t = Terrain::leaf(vec![0; PADDED * PADDED], Ground { cells: cells.into() });
    t.index.occupancy = Occupancy::ZERO;
    t
}

/// Two levels folded over sixteen leaves under `top`, a few of them left as
/// the ocean, as the bake folds them: every page by level, the zero page where
/// a leaf has no object.
fn pyramid(top: LevelPage) -> (std::collections::HashMap<LevelPage, Terrain>, Terrain) {
    let (mut rng, zero) = (Rng(0x9e37_79b9_7f4a_7c15), zero_page());
    let mut held = std::collections::HashMap::new();
    for (k, leaf) in top.children().iter().flat_map(|c| c.children()).enumerate() {
        if ![3, 6, 7].contains(&k) {
            held.insert(leaf, rough_leaf(&mut rng));
        }
    }
    for mid in top.children() {
        let folded = Terrain::fold_page(mid, |p| Some(held.get(&p).unwrap_or(&zero))).unwrap();
        held.insert(mid, folded);
    }
    let folded = Terrain::fold_page(top, |p| Some(held.get(&p).unwrap_or(&zero))).unwrap();
    held.insert(top, folded);
    (held, zero)
}

fn point(x: f64, y: f64) -> crate::space::pvf3::pvf3::Pvf3 {
    use crate::space::pvf3::pvf3;
    let mut at = pvf3::Pvf3::default();
    pvf3::gather(&mut at, pvf3::from_real(x), pvf3::from_real(y), 0);
    at
}

/// The zero page is what every page without an object reads at every level,
/// which holds only if four of it fold to it: its index too.
#[test]
fn the_zero_page_folds_to_itself() {
    let zero = zero_page();
    assert_eq!(Terrain::fold_page(LevelPage::new(1, 0), |_| Some(&zero)).as_ref(), Some(&zero));
}

#[test]
fn a_parents_occupancy_is_which_of_its_children_are_pages() {
    let top = LevelPage::new(2, crate::space::pvf3::ivec::gather(-1, 3, 0));
    let (held, _) = pyramid(top);
    for parent in [top].into_iter().chain(top.children()) {
        let o = held[&parent].index.occupancy;
        let pages = parent.children().map(|c| held.get(&c).is_some_and(|t| t.index.occupancy.is_page()));
        assert!((0..4).all(|q| o.has_child(q) == pages[q]), "{parent:?}: {o:?} against {pages:?}");
        assert_eq!(o.is_page(), pages.contains(&true));
    }
}

#[test]
fn a_parents_extent_holds_each_childs() {
    let top = LevelPage::new(2, crate::space::pvf3::ivec::gather(-1, 3, 0));
    let (held, zero) = pyramid(top);
    for parent in [top].into_iter().chain(top.children()) {
        let outer = &held[&parent].index.extent();
        for (q, child) in parent.children().iter().enumerate() {
            let inner = &held.get(child).unwrap_or(&zero).index.extent();
            for (index, &[lo, hi]) in inner.iter().enumerate() {
                let [plo, phi] = outer[EXTENT.parent_cell(q, index as u32) as usize];
                assert!(plo <= lo && hi <= phi, "{parent:?} child {q} cell {index}: [{lo}, {hi}] outside [{plo}, {phi}]");
            }
        }
    }
}

/// Wherever a page's ground is drawn — at a cell's center or anywhere between
/// — the height drawn lies inside its own extent there and inside the extent
/// of every page over it.
#[test]
fn the_extent_holds_every_height_drawn_under_it() {
    let top = LevelPage::new(2, crate::space::pvf3::ivec::gather(-1, 3, 0));
    let (held, _) = pyramid(top);
    let mut checked = 0u32;
    for (&page, t) in &held {
        let (x0, y0, _) = page.rect();
        let side = GROUND.scale(page.level).side();
        for ty in (0..SPAN).step_by(3) {
            for tx in (0..SPAN).step_by(3) {
                for f in [0.0, 0.5, 0.73] {
                    let (fx, fy) = (tx as f64 + f, ty as f64 + (1.0 - f) * 0.9);
                    let cell = GROUND.cell_index(tx as u32, fy.floor() as u32) as usize;
                    let h = super::super::surface::bicubic(&t.ele, fx, fy) as i32 - super::super::ground::cell_recess(t.ground.cells[cell]) as i32;
                    let at = point(x0 + fx * side, y0 + fy * side);
                    let mut over = page;
                    while over.level <= top.level {
                        let c = EXTENT.cell_of(over.level, &at);
                        assert_eq!(c.page(), over);
                        let [lo, hi] = held[&over].index.extent()[c.index() as usize];
                        assert!(lo as i32 <= h && h <= hi as i32, "{page:?} at ({fx}, {fy}): {h} outside {over:?}'s [{lo}, {hi}]");
                        checked += 1;
                        over = over.parent();
                    }
                }
            }
        }
    }
    assert!(checked > 100_000);
}
