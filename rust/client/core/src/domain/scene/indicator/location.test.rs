// pxl-core/domain/scene/indicator/location.test.rs

use super::*;
use crate::domain::layer::object::store::{self, Store, Write};
use crate::domain::query::change;
use crate::domain::query::test::{flat_at_five, placed, write};
use crate::domain::scene::model::draw_list;
use crate::sys::host::set_clock;
use pxl_render::gpu::draw::{lists_sz, set_lists};
use pxl_spec::space::coord::q3;
use pxl_spec::space::layer::octant::Octant;

/// A rough ground, level over each cell. It steps up and down by fractions of
/// a cell, and here and there a column is unknown.
fn rough(at: &Q3) -> Option<f64> {
    let [x, y] = cell_of(at);
    let h = (x * 7 + y * 13).rem_euclid(11);
    (h != 4).then(|| 3.0 + f64::from(h) * 0.375 - f64::from((x - y).rem_euclid(3)))
}

/// The ground's cell a point lies in.
fn cell_of(at: &Q3) -> [i32; 2] {
    let cell = units(GROUND.leaf_cell_zoom());
    [0, 1].map(|k| (q3::to_real(i64::from(at[k])) / cell).floor() as i32)
}

/// The zone laid about the origin over a ground whose cell `at` has its top
/// `rise(at)` cells above the zone's center, each of its cells with the faces
/// its box shows and the lines it draws.
fn laid(rise: impl Fn([i32; 2]) -> i32) -> Vec<([i32; 2], u32, u32)> {
    let cell = units(GROUND.leaf_cell_zoom());
    let center = [0.0, 0.0, 3.0 * cell];
    let instances = lay_zone(center, |at| {
        Some(center[2] + f64::from(rise(cell_of(at))) * cell)
    });
    zone_cells()
        .zip(instances.chunks(ZONE_INSTANCE_FLOATS))
        .map(|(at, v)| (at, v[3] as u32, v[4] as u32))
        .collect()
}

/// The lines a box draws along its top's edges toward the cells beyond the
/// zone.
fn top_lines(at: [i32; 2]) -> u32 {
    (0..4)
        .filter(|&k| !in_zone(beside(at, k)))
        .map(|k| line_bit(TOP, k))
        .sum()
}

#[test]
fn zone_cells_once() {
    let cell = units(GROUND.leaf_cell_zoom());
    for center in [[0.0, 0.0, 3.0], [10.0, -7.0, 5.0], [-123.0, 456.0, -2.0]] {
        let center = center.map(|v| v * cell);
        let mut asked = 0;
        lay_zone(center, |at| {
            asked += 1;
            rough(at)
        });
        let mut cells: Vec<[i32; 2]> = zone_cells()
            .flat_map(|[x, y]| {
                (-1..=1).flat_map(move |dx| (-1..=1).map(move |dy| [x + dx, y + dy]))
            })
            .collect();
        cells.sort_unstable();
        cells.dedup();
        assert_eq!(asked, cells.len(), "a cell asked more than once");
        assert!(
            zone_cells().any(|at| {
                let at = [0, 1].map(|k| center[k] + f64::from(at[k]) * cell);
                rough(&world::point(at[0], at[1], center[2])).is_none()
            }),
            "a column not known lies in the zone about {center:?}"
        );
    }
}

/// On level ground the zone shows its tops alone, and draws its outline along
/// their edges toward the cells beyond it.
#[test]
fn level_ground_lines_the_tops() {
    for (at, faces, lines) in laid(|_| 0) {
        assert_eq!(faces, 1 << TOP, "{at:?} shows a side");
        assert_eq!(lines, top_lines(at), "{at:?}");
    }
}

/// Where the ground beyond the zone is a cell lower, the zone shows the sides
/// that drop to it, and its outline runs along their feet, not along the
/// tops' edges above them.
#[test]
fn a_plateau_is_lined_at_its_foot() {
    for (at, faces, lines) in laid(|at| i32::from(in_zone(at))) {
        let out = (0..4).filter(|&k| !in_zone(beside(at, k)));
        assert_eq!(
            faces,
            out.clone().fold(1 << TOP, |f, k| f | 1 << k),
            "{at:?}"
        );
        assert_eq!(lines, out.map(|k| line_bit(k, BOTTOM)).sum(), "{at:?}");
    }
}

/// A step inside the zone shows its side, which draws a line only up its
/// edges where the zone ends beside it.
#[test]
fn a_step_inside_draws_no_line_across_it() {
    let up = |[x, _]: [i32; 2]| i32::from(x >= 1);
    for (at, faces, lines) in laid(up) {
        let [x, y] = at;
        if x != 1 {
            assert_eq!((faces, lines), (1 << TOP, top_lines(at)), "{at:?}");
            continue;
        }
        // The side toward -x shows, and its upright edges toward ±y carry a
        // line where the cell beside lies beyond the zone.
        let ends = [(2, [1, y + 1]), (3, [1, y - 1])]
            .into_iter()
            .filter(|&(_, next)| !in_zone(next))
            .map(|(j, _)| line_bit(1, j))
            .sum::<u32>();
        assert_eq!(faces, 1 << TOP | 1 << 1, "{at:?}");
        assert_eq!(lines, top_lines(at) | ends, "{at:?}");
    }
}

/// Over any ground, a line lies only on a face the box shows, along the top's
/// edge or a side's foot only toward a cell beyond the zone.
#[test]
fn lines_lie_on_the_zone_s_border() {
    let cell = units(GROUND.leaf_cell_zoom());
    let instances = lay_zone([0.0, 0.0, 3.0 * cell], rough);
    for (at, v) in zone_cells().zip(instances.chunks(ZONE_INSTANCE_FLOATS)) {
        let (faces, lines) = (v[3] as u32, v[4] as u32);
        for f in 0..5 {
            let mine = (lines >> (4 * f)) & 15;
            assert!(
                mine == 0 || faces & 1 << f != 0,
                "{at:?} draws on hidden face {f}"
            );
        }
        for k in 0..4 {
            if in_zone(beside(at, k)) {
                assert_eq!(
                    lines & line_bit(TOP, k),
                    0,
                    "{at:?} lines its top toward {k}"
                );
                assert_eq!(
                    lines & line_bit(k, BOTTOM),
                    0,
                    "{at:?} lines its side {k}'s foot"
                );
            }
        }
    }
}

#[test]
fn beacon_holds_on_stall() {
    // The zone is drawn here, so its cells are laid into the draw lists.
    const LIVE: usize = 4;
    set_lists(Vec::leak(vec![0; lists_sz(LIVE) as usize / 4]), LIVE);
    let mut location = LocationIndicator {
        zone: Some(Zone {
            center: [0.0; 3],
            beacon_to: [0.0; 2],
            beacon_at: None,
            run: 0.0,
            clock: StepClock::default(),
            visible: false,
            zone_drawn: false,
        }),
        ..Default::default()
    };
    // The eye stands at the zone's center, so the zone is seen and the bob
    // spans a camera cell there.
    set_clock(0.0);
    location.tick(
        draw_list(),
        &Frame {
            delta_s: 0.05,
            ..Frame::default()
        },
    );
    // A minute passes, as in a hidden tab. The frame's step is capped as
    // frame.rs caps it (`MAX_STEP_S`), so the bob must not leap.
    set_clock(60_000.0);
    location.tick(
        draw_list(),
        &Frame {
            delta_s: 0.1,
            ..Frame::default()
        },
    );
    let time = location.zone.as_ref().map_or(f64::NAN, |z| z.clock.time());
    assert!(time > 0.0 && time < 0.2, "the bob leapt to {time} s");
}

/// The zone stands on the fix's leaf cell, so a move within the cell grounds
/// nothing again and keeps the zone, while the beacon follows the fix; a move
/// into the next cell grounds the zone there. A cube written into the fix's
/// cell without a change noted would raise a zone grounded again.
#[test]
fn a_move_within_a_cell_grounds_nothing_again() {
    let [x, y] = flat_at_five();
    placed(Store::default());
    let cell = units(GROUND.leaf_cell_zoom());
    let [x, y, _] = v3::floor(&[x, y, 0.0], cell);
    let mut location = LocationIndicator::default();
    location.step(Some(world::point(x + 0.25 * cell, y + 0.25 * cell, 0.0)));
    let center = location.zone.as_ref().map(|z| z.center).unwrap();
    let mut store = Store::default();
    write(
        &mut store,
        [x, y, center[2] + 0.5 * cell],
        GROUND.leaf_cell_zoom(),
        1,
    );
    placed(store);
    let within = world::point(x + 0.75 * cell, y + 0.75 * cell, 0.0);
    location.step(Some(within));
    let zone = location.zone.as_ref().unwrap();
    assert_eq!(zone.center, center);
    assert_eq!(zone.beacon_to, [0.75 * cell, 0.75 * cell]);
    let mut fresh = LocationIndicator::default();
    fresh.step(Some(within));
    let raised = fresh.zone.as_ref().map(|z| z.center).unwrap();
    assert!(
        raised[2] > center[2],
        "the cube raises a zone grounded again"
    );
    location.step(Some(world::point(x + 1.25 * cell, y + 0.25 * cell, 0.0)));
    let next = location.zone.as_ref().map(|z| z.center).unwrap();
    assert_eq!([next[0], next[1]], [center[0] + cell, center[1]]);
}

/// The zone's cells are laid only while the zone is drawn, and kept while it
/// is not: a step grounds its center alone, a frame far from the zone lays
/// nothing, a frame that draws it lays its cells, and a zone that goes far
/// and comes back at the same cell asks nothing again (a cube written under
/// it without a change noted would show in cells laid again), a fix gone and
/// come back lays again, and so does a change noted under the zone.
#[test]
fn the_cells_are_laid_only_while_the_zone_is_drawn() {
    const LIVE: usize = 4;
    set_lists(Vec::leak(vec![0; lists_sz(LIVE) as usize / 4]), LIVE);
    let [x, y] = flat_at_five();
    placed(Store::default());
    set_clock(0.0);
    let mut location = LocationIndicator::default();
    let fix = world::point(x, y, 0.0);
    location.step(Some(fix));
    let center = location.zone.as_ref().map(|z| z.center).unwrap();
    assert!(location.instances.is_empty(), "a step lays no cell");
    // The eye far over the zone: it spans less than a camera cell there.
    let far = Frame {
        camera: [center[0], center[1], center[2] + 1.0e4],
        ..Frame::default()
    };
    location.tick(draw_list(), &far);
    assert!(
        location.instances.is_empty(),
        "nor does a frame far from it"
    );
    // The eye at the zone's center.
    let near = Frame {
        camera: center,
        ..Frame::default()
    };
    location.tick(draw_list(), &near);
    let first = location.instances.clone();
    assert!(
        !first.is_empty(),
        "a frame that draws the zone lays its cells"
    );
    let cell = units(GROUND.leaf_cell_zoom());
    let mut store = Store::default();
    write(
        &mut store,
        [center[0] + cell, center[1], center[2] + 0.5 * cell],
        GROUND.leaf_cell_zoom(),
        1,
    );
    placed(store);
    for frame in [&far, &near] {
        location.step(Some(fix));
        location.tick(draw_list(), frame);
    }
    assert_eq!(location.instances, first, "back in view, nothing is asked");
    // With the fix gone nothing is kept, so the fix come back lays again.
    location.step(None);
    location.step(Some(fix));
    location.tick(draw_list(), &near);
    assert_ne!(location.instances, first, "a fix come back lays again");
    // A change noted under the zone, as a write into the store notes it,
    // lays the cells again in the frame that takes it.
    let second = location.instances.clone();
    let under = world::point(center[0] - cell, center[1], center[2] + 0.5 * cell);
    assert!(store::apply([Write::at(
        Octant::holding(GROUND.leaf_cell_zoom(), &under),
        1
    )]));
    change::take();
    location.step(Some(fix));
    location.tick(draw_list(), &near);
    assert_ne!(location.instances, second, "a noted change lays again");
}
