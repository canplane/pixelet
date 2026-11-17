// pxl-core/domain/scene/indicator/location.test.rs

use super::*;
use crate::domain::layer::object::store::Store;
use crate::domain::query::test::{flat_at_five, placed, write};
use crate::sys::host::set_clock;
use pxl_render::gpu::draw::{lists_sz, set_lists};
use pxl_spec::space::coord::q3;

/// The zone laid the plain way, asking the ground again for every box and for
/// each of its four sides. `lay_zone` must lay the same bytes while asking
/// each cell once.
fn laid_asking_each(center_pos: V3, mut ground: impl FnMut(&Q3) -> Option<f64>) -> Vec<f32> {
    let mut instances = Vec::new();
    let cell = units(GROUND.leaf_cell_zoom());
    let mut box_at = |off: V3| {
        let v = v3::add(&center_pos, &off);
        let coord = world::point(v[0], v[1], v[2]);
        let delta_ele = ground(&coord).unwrap_or(center_pos[2]) - center_pos[2];
        v3::center(&[off[0], off[1], off[2] + delta_ele], cell)
    };
    for s in zone_samples() {
        let v = box_at(s.off);
        let mut side = |k: usize, d: [f64; 2]| {
            let open = box_at([s.off[0] + d[0] * cell, s.off[1] + d[1] * cell, s.off[2]])[2] < v[2];
            u32::from(open) << k
        };
        let open = side(0, [1.0, 0.0])
            | side(1, [-1.0, 0.0])
            | side(2, [0.0, 1.0])
            | side(3, [0.0, -1.0])
            | 1 << 4;
        instances.extend([
            v[0] as f32,
            v[1] as f32,
            v[2] as f32,
            s.borders[0] as f32,
            s.borders[1] as f32,
            s.borders[2] as f32,
            s.borders[3] as f32,
            open as f32,
        ]);
    }
    instances
}

/// A rough ground, level over each cell. It steps up and down by fractions of
/// a cell, and here and there a column is unknown.
fn rough(at: &Q3) -> Option<f64> {
    let cell = units(GROUND.leaf_cell_zoom());
    let [x, y] = [0, 1].map(|k| (q3::to_real(i64::from(at[k])) / cell).floor() as i64);
    let h = (x * 7 + y * 13).rem_euclid(11);
    (h != 4).then(|| 3.0 + h as f64 * 0.375 - (x - y).rem_euclid(3) as f64)
}

#[test]
fn zone_cells_once() {
    let cell = units(GROUND.leaf_cell_zoom());
    for center in [[0.0, 0.0, 3.0], [10.0, -7.0, 5.0], [-123.0, 456.0, -2.0]] {
        let center = center.map(|v| v * cell);
        let mut asked = 0;
        let instances = lay_zone(center, |at| {
            asked += 1;
            rough(at)
        });
        let before = laid_asking_each(center, rough);
        assert_eq!(
            instances.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
            before.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
            "about {center:?}"
        );
        let mut cells: Vec<[i32; 2]> = zone_samples()
            .flat_map(|s| {
                [[0, 0], [1, 0], [-1, 0], [0, 1], [0, -1]].map(|d| [s.at[0] + d[0], s.at[1] + d[1]])
            })
            .collect();
        cells.sort_unstable();
        cells.dedup();
        assert_eq!(asked, cells.len(), "a cell asked more than once");
        assert!(
            zone_samples().any(|s| {
                let at = [0, 1].map(|k| center[k] + f64::from(s.at[k]) * cell);
                rough(&world::point(at[0], at[1], center[2])).is_none()
            }),
            "a column not known lies in the zone about {center:?}"
        );
    }
}

#[test]
fn beacon_holds_on_stall() {
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
    location.tick(&Frame {
        delta_s: 0.05,
        ..Frame::default()
    });
    // A minute passes, as in a hidden tab. The frame's step is capped as
    // frame.rs caps it (`MAX_STEP_S`), so the bob must not leap.
    set_clock(60_000.0);
    location.tick(&Frame {
        delta_s: 0.1,
        ..Frame::default()
    });
    let time = location.zone.as_ref().map_or(f64::NAN, |z| z.clock.time());
    assert!(time > 0.0 && time < 0.2, "the bob leapt to {time} s");
}

/// The zone stands on the fix's leaf cell, so a move within the cell grounds
/// nothing again and keeps the zone, while the beacon follows the fix; a move
/// into the next cell grounds the zone there. A cube written into the fix's
/// cell without a change noted would raise a zone grounded again.
#[test]
fn a_move_within_a_cell_grounds_nothing_again() {
    const LIVE: usize = 4;
    set_lists(Vec::leak(vec![0; lists_sz(LIVE) as usize / 4]), LIVE);
    let [x, y] = flat_at_five();
    placed(Store::default());
    let cell = units(GROUND.leaf_cell_zoom());
    let [x, y, _] = v3::floor(&[x, y, 0.0], cell);
    let mut location = LocationIndicator::default();
    location.step(
        draw_list(),
        Some(world::point(x + 0.25 * cell, y + 0.25 * cell, 0.0)),
    );
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
    location.step(draw_list(), Some(within));
    let zone = location.zone.as_ref().unwrap();
    assert_eq!(zone.center, center);
    assert_eq!(zone.beacon_to, [0.75 * cell, 0.75 * cell]);
    let mut fresh = LocationIndicator::default();
    fresh.step(draw_list(), Some(within));
    let raised = fresh.zone.as_ref().map(|z| z.center).unwrap();
    assert!(
        raised[2] > center[2],
        "the cube raises a zone grounded again"
    );
    location.step(
        draw_list(),
        Some(world::point(x + 1.25 * cell, y + 0.25 * cell, 0.0)),
    );
    let next = location.zone.as_ref().map(|z| z.center).unwrap();
    assert_eq!([next[0], next[1]], [center[0] + cell, center[1]]);
}
