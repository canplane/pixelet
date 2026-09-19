// pxl-spec/space/lattice/grid_index.test.rs

use super::*;

type Rect = [f64; 4];

/// Mapbox's reference viewport and cell, so the numbers here can be checked
/// against the source this follows: 600x600 at 30 px is 400 cells.
fn grid(capacity: u32, cell: f64) -> GridIndex {
    GridIndex::new(600, 600, capacity, cell, 8)
}

fn overlaps(a: &Rect, b: &Rect) -> bool {
    a[0] <= b[2] && a[2] >= b[0] && a[1] <= b[3] && a[3] >= b[1]
}

/// Brute force, as the oracle: greedy first-fit over the same sequence.
fn greedy(rects: &[Rect]) -> Vec<bool> {
    let mut kept: Vec<Rect> = Vec::new();
    rects
        .iter()
        .map(|r| {
            if kept.iter().any(|k| overlaps(r, k)) {
                return false;
            }
            kept.push(*r);
            true
        })
        .collect()
}

/// Deterministic, so a failure is reproducible without recording a seed.
fn scatter(n: usize, size: f64, span: f64) -> Vec<Rect> {
    let mut out: Vec<Rect> = Vec::new();
    let mut s: i32 = 0x6d2b79f5;
    let mut next = |max: f64| {
        s ^= s.wrapping_shl(13);
        s ^= ((s as u32) >> 17) as i32;
        s ^= s.wrapping_shl(5);
        ((s as u32) as f64 / 4_294_967_296.0) * max
    };
    for _ in 0..n {
        let (x, y) = (next(span), next(span));
        let (w, h) = (4.0 + next(size), 4.0 + next(size / 3.0));
        out.push([x, y, x + w, y + h]);
    }
    out
}

fn place(g: &mut GridIndex, r: &Rect) -> Placement {
    g.place(r[0], r[1], r[2], r[3])
}

/// The whole contract, and the only thing a caller can rely on: the answer is
/// exactly what a naive first-fit would have produced. The grid is an
/// acceleration structure, so any divergence from brute force is a bug in the
/// acceleration rather than a different policy.
#[test]
fn grid_index_placement_matches_brute_force_greedy_exactly() {
    for size in [10.0, 40.0, 120.0] {
        let rects = scatter(400, size, 600.0);
        let mut g = grid(512, 30.0);
        g.begin(600.0, 600.0);

        let got: Vec<bool> = rects.iter().map(|r| place(&mut g, r) == Placement::Placed).collect();
        assert_eq!(got, greedy(&rects), "size {size}");
    }
}

/// The failure a cell grid invites: two rectangles that overlap but share no
/// cell corner, or that share a cell without overlapping. Both are written
/// out rather than scattered, because a random test hits them rarely and a
/// hand-built one hits them every run.
#[test]
fn grid_index_overlap_is_decided_by_the_rectangles_not_by_their_cells() {
    let mut g = grid(512, 30.0);
    g.begin(600.0, 600.0);

    // Same cell, no overlap: both must be placed.
    assert_eq!(g.place(1.0, 1.0, 5.0, 5.0), Placement::Placed);
    assert_eq!(g.place(10.0, 10.0, 14.0, 14.0), Placement::Placed);

    // Different cells, real overlap: a rectangle spanning the boundary must
    // be caught by the neighbour it reaches into.
    assert_eq!(g.place(100.0, 100.0, 110.0, 110.0), Placement::Placed);
    assert!(matches!(g.place(105.0, 105.0, 200.0, 200.0), Placement::Blocked(_)));

    // Touching edges count as overlap, which is the conservative choice for
    // labels and has to be consistent with the oracle.
    assert_eq!(g.place(300.0, 300.0, 310.0, 310.0), Placement::Placed);
    assert!(matches!(g.place(310.0, 310.0, 320.0, 320.0), Placement::Blocked(_)));
}

/// A rectangle wider than the grid's own cell must register in every cell it
/// covers, or a later candidate landing in the middle of it would find the
/// space free.
#[test]
fn grid_index_a_rectangle_spanning_many_cells_blocks_all_of_them() {
    let mut g = grid(512, 30.0);
    g.begin(600.0, 600.0);

    assert_eq!(g.place(0.0, 0.0, 300.0, 300.0), Placement::Placed);
    for [x, y] in [[10.0, 10.0], [150.0, 150.0], [290.0, 290.0], [0.0, 299.0]] {
        assert!(matches!(g.place(x, y, x + 2.0, y + 2.0), Placement::Blocked(_)), "{x},{y} should be blocked");
    }
    assert_eq!(g.place(320.0, 320.0, 322.0, 322.0), Placement::Placed);
}

/// Order is the priority rule, so it has to actually decide. The same set in
/// the opposite order must keep the other member of a conflicting pair.
#[test]
fn grid_index_the_caller_s_order_decides_which_of_two_conflicting_rectangles_survives() {
    let a: Rect = [50.0, 50.0, 90.0, 70.0];
    let b: Rect = [60.0, 55.0, 100.0, 75.0];

    let mut first = grid(512, 30.0);
    first.begin(600.0, 600.0);
    assert_eq!(place(&mut first, &a), Placement::Placed);
    assert!(matches!(place(&mut first, &b), Placement::Blocked(_)));

    let mut second = grid(512, 30.0);
    second.begin(600.0, 600.0);
    assert_eq!(place(&mut second, &b), Placement::Placed);
    assert!(matches!(place(&mut second, &a), Placement::Blocked(_)));
}

/// The stamp is what removes the clearing pass, so a stale cell leaking into
/// a later generation is what it risks. A thousand generations of a shifting
/// layout is where that shows.
#[test]
fn grid_index_a_thousand_generations_leave_no_trace_of_the_ones_before() {
    let mut g = grid(512, 30.0);
    let rects = scatter(200, 30.0, 600.0);

    for step in 0..1000 {
        let shifted: Vec<Rect> = rects
            .iter()
            .map(|r| [r[0] + (step % 7) as f64, r[1] - (step % 5) as f64, r[2] + (step % 7) as f64, r[3] - (step % 5) as f64])
            .collect();
        g.begin(600.0, 600.0);
        let got: Vec<bool> = shifted.iter().map(|r| place(&mut g, r) == Placement::Placed).collect();
        assert_eq!(got, greedy(&shifted), "step {step}");
    }
}

/// A resize must reuse the arrays it was constructed with.
#[test]
fn grid_index_a_smaller_viewport_uses_fewer_cells_without_reallocating() {
    let mut g = grid(512, 30.0);
    let owned = |g: &GridIndex| {
        [
            g.rects.as_ptr() as usize,
            g.heads.as_ptr() as usize,
            g.stamps.as_ptr() as usize,
            g.node_rect.as_ptr() as usize,
            g.node_next.as_ptr() as usize,
        ]
    };

    g.begin(600.0, 600.0);
    let before = owned(&g);
    assert_eq!(g.place(10.0, 10.0, 20.0, 20.0), Placement::Placed);

    g.begin(320.0, 240.0);
    assert_eq!(g.place(10.0, 10.0, 20.0, 20.0), Placement::Placed);
    assert!(matches!(g.place(15.0, 15.0, 25.0, 25.0), Placement::Blocked(_)));

    let after = owned(&g);
    assert_eq!(after.len(), before.len());
    for i in 0..before.len() {
        assert!(after[i] == before[i], "array {i} was replaced");
    }
}

/// Capacity is the design here too. A full grid has to refuse rather than
/// grow, and say so, because a caller that silently under-reports occupancy
/// would draw labels on top of each other.
#[test]
fn grid_index_capacity_and_the_node_pool_are_refused_not_grown() {
    let mut g = GridIndex::new(600, 600, 4, 30.0, 2);
    g.begin(600.0, 600.0);

    // Four slots, two nodes each: a one-cell rectangle uses one node, so the
    // slots run out first.
    for i in 0..4 {
        let x = (i * 60) as f64;
        assert_eq!(g.place(x, 0.0, x + 5.0, 5.0), Placement::Placed, "slot {i}");
    }
    assert_eq!(g.place(400.0, 0.0, 405.0, 5.0), Placement::NoRoom);
    assert_eq!(g.stats().placed, 4);
    assert_eq!(g.stats().dropped, 1);

    // A rectangle needing more nodes than the pool has left is refused even
    // with a slot free.
    let mut h = GridIndex::new(600, 600, 8, 30.0, 1);
    h.begin(600.0, 600.0);
    assert_eq!(h.place(0.0, 0.0, 5.0, 5.0), Placement::Placed);
    assert_eq!(h.place(200.0, 200.0, 400.0, 400.0), Placement::NoRoom);
    assert!(h.stats().dropped >= 1);
}

/// Off-screen is not this structure's decision. It knows about pixels, and a
/// rectangle outside them collides with nothing — culling belongs to whatever
/// runs before.
#[test]
fn grid_index_a_rectangle_entirely_off_screen_collides_with_nothing() {
    let mut g = grid(512, 30.0);
    g.begin(600.0, 600.0);

    assert_eq!(g.place(10.0, 10.0, 50.0, 50.0), Placement::Placed);
    assert!(g.hit_test(10.0, 10.0, 50.0, 50.0));
    assert!(!g.hit_test(-200.0, -200.0, -100.0, -100.0));
    assert!(!g.hit_test(900.0, 900.0, 1000.0, 1000.0));
}

/// The density case, as with the spatial hash: far more candidates than room.
/// The point is not how many survive but that the answer stays exact when
/// almost everything collides.
#[test]
fn grid_index_ten_thousand_candidates_for_a_screen_that_fits_far_fewer() {
    let rects = scatter(10000, 20.0, 600.0);
    let mut g = GridIndex::new(600, 600, 10000, 30.0, 8);
    g.begin(600.0, 600.0);

    let got: Vec<bool> = rects.iter().map(|r| place(&mut g, r) == Placement::Placed).collect();
    assert_eq!(got, greedy(&rects));

    let kept = got.iter().filter(|&&b| b).count();
    assert!(kept > 0 && kept < 10000, "expected a real cut, kept {kept}");
    assert_eq!(g.stats().dropped, 0);
}

/// The attribution absorption rests on: a rejected rectangle has to name the
/// one that took its space, and that name has to be the caller's own ordinal,
/// or a caller cannot turn "refused" into "counted somewhere".
#[test]
fn grid_index_a_blocked_rectangle_reports_which_placement_blocked_it() {
    let mut g = grid(512, 30.0);
    g.begin(600.0, 600.0);

    assert_eq!(g.place(0.0, 0.0, 20.0, 20.0), Placement::Placed); // id 0
    assert_eq!(g.place(100.0, 100.0, 120.0, 120.0), Placement::Placed); // id 1
    assert_eq!(g.place(200.0, 200.0, 220.0, 220.0), Placement::Placed); // id 2

    assert_eq!(g.place(10.0, 10.0, 15.0, 15.0), Placement::Blocked(0));
    assert_eq!(g.place(110.0, 110.0, 115.0, 115.0), Placement::Blocked(1));
    assert_eq!(g.place(205.0, 205.0, 215.0, 215.0), Placement::Blocked(2));

    // A rejection places nothing, so the next free rectangle is still id 3.
    assert_eq!(g.place(400.0, 400.0, 420.0, 420.0), Placement::Placed);
    assert_eq!(g.place(405.0, 405.0, 410.0, 410.0), Placement::Blocked(3));
}
