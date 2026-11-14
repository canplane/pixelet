// pxl-core/domain/scene/indicator/location.test.rs

use super::*;
use crate::sys::host::set_clock;
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
