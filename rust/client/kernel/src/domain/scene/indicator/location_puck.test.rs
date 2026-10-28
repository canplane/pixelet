// pxl-kernel/domain/scene/indicator/location_puck.test.rs

use super::*;
use pxl_spec::space::coord::q3;

/// The zone laid by asking the ground for every box and again for each of
/// its four sides, what `lay_zone` must lay the same bytes as.
fn laid_asking_each(center_pos: V3, mut ground: impl FnMut(&Q3) -> Option<f64>) -> Vec<f32> {
    let mut lane = Vec::new();
    let cell = units(GROUND.cell());
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
        lane.extend([
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
    lane
}

/// A rough ground over the cell a point is in: steps up and down, a fraction
/// of a cell high, and a column here and there not known.
fn rough(at: &Q3) -> Option<f64> {
    let cell = units(GROUND.cell());
    let [x, y] = [0, 1].map(|k| (q3::to_real(i64::from(at[k])) / cell).floor() as i64);
    let h = (x * 7 + y * 13).rem_euclid(11);
    (h != 4).then(|| 3.0 + h as f64 * 0.375 - (x - y).rem_euclid(3) as f64)
}

#[test]
fn the_zone_asks_each_cell_once_and_lays_the_same_bytes() {
    let cell = units(GROUND.cell());
    for center in [[0.0, 0.0, 3.0], [10.0, -7.0, 5.0], [-123.0, 456.0, -2.0]] {
        let center = center.map(|v| v * cell);
        let (mut asked, mut rise, mut lane) = (0, Vec::new(), Vec::new());
        lay_zone(
            center,
            |at| {
                asked += 1;
                rough(at)
            },
            &mut rise,
            &mut lane,
        );
        let before = laid_asking_each(center, rough);
        assert_eq!(
            lane.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
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
    }
}

#[test]
fn a_stepped_clock_steps_once_a_step_has_passed_and_wakes_for_the_next() {
    let (mut time, mut stepped) = (0.0, None);
    // The first turn starts it: nothing has passed.
    assert_eq!(
        step_clock(&mut time, &mut stepped, 1000.0, Some(50.0)),
        Some(1050.0)
    );
    assert_eq!(time, 0.0);
    // Short of a step it stands, and the wake stays where it was.
    assert_eq!(
        step_clock(&mut time, &mut stepped, 1030.0, Some(50.0)),
        Some(1050.0)
    );
    assert_eq!(time, 0.0);
    // Past it, it takes all that passed.
    assert_eq!(
        step_clock(&mut time, &mut stepped, 1080.0, Some(50.0)),
        Some(1130.0)
    );
    assert_eq!(time, 0.08);
}

#[test]
fn a_held_clock_takes_nothing_of_what_passed_while_held() {
    let (mut time, mut stepped) = (2.0, None);
    step_clock(&mut time, &mut stepped, 0.0, Some(50.0));
    assert_eq!(step_clock(&mut time, &mut stepped, 10.0, None), None);
    // A minute later it runs again: from there, with no jump.
    assert_eq!(
        step_clock(&mut time, &mut stepped, 60_010.0, Some(50.0)),
        Some(60_060.0)
    );
    assert_eq!(time, 2.0);
}
