// pxl-core/domain/scene/marker/place.test.rs

use super::*;
use pxl_spec::space::coord::zoom::{LEAF_CELL_ZOOM, LEAF_ZOOM};

/// A place read from the brick it stands in, on the lap of X `lap` rounds
/// from the world's own.
fn place_in(brick_zoom: Zoom, lap: i32) -> Place {
    let src = Quadrant::new(LEAF_ZOOM, [3, 5]);
    let ground = stands(src, [7, 9]);
    let [x, y, _] = idx_of(brick_zoom, &ground.mid());
    Place {
        brick: (brick_zoom, [x + lap * (1 << brick_zoom), y]),
        kind: 0,
        first_zoom: 0,
        name: None,
        ground,
        finest: LEAF_CELL_ZOOM,
        top: None,
        asked: None,
    }
}

/// A kind Carto shows at the finest zoom.
fn shown_kind() -> u16 {
    let zoom = carto_zoom(LEAF_CELL_ZOOM);
    (1..=ICON_ZOOM.len() as u16)
        .find(|&k| point_of(k, 0, None, zoom).is_some())
        .unwrap()
}

/// The entry's budget, spent so that a pass asks one item, or ample.
fn budget(spent: bool) {
    use crate::sys::host::set_clock;
    set_clock(0.0);
    sched::begin(1000.0);
    if spent {
        set_clock(1e6);
    }
}

/// Three places Carto shows, each standing on a surface it holds, its own
/// still to ask.
fn standing() -> Vec<Place> {
    let src = Quadrant::new(LEAF_ZOOM, [3, 5]);
    (0..3)
        .map(|k| Place {
            ground: stands(src, [7 + 40 * k, 9]),
            kind: shown_kind(),
            top: Some(3.0),
            ..place_in(12, 0)
        })
        .collect()
}

/// A place read again stands on what the same place stood on until its own
/// is asked.
#[test]
fn a_place_read_again_keeps_its_stand() {
    let held = Place {
        top: Some(3.0),
        asked: Some([0.0; 3]),
        ..place_in(12, 0)
    };
    let mut read = vec![held, place_in(12, 0)];
    hand_held(&mut read, 1);
    assert_eq!((read[1].top, read[1].asked), (Some(3.0), None));
    // Its own ask takes the place of what it was handed: nothing is known
    // under it here.
    budget(false);
    show(&mut read, [0.0, 0.0, 10.0], &mut Vec::new());
    assert_eq!((read[1].top, read[1].asked.is_some()), (None, true));
}

/// With the budget spent, a run asks one place's surface and the others
/// stand on the surface they were handed; later runs ask the rest, and then
/// none. Nothing is known under them here, so a place asked stands no more.
#[test]
fn surfaces_are_asked_under_the_budget() {
    let mut read = standing();
    let [x, y, _] = q3::unpack(&read[0].ground.mid());
    let eye = [x, y, 10.0];
    let asked = |read: &[Place]| read.iter().filter(|p| p.asked.is_some()).count();
    let mut out = Vec::new();
    budget(true);
    assert!(show(&mut read, eye, &mut out), "some are left");
    assert_eq!(asked(&read), 1);
    assert_eq!(out.len(), 2, "those still to ask stand on what they hold");
    assert!(show(&mut read, eye, &mut out));
    assert!(!show(&mut read, eye, &mut out), "the last is asked");
    assert_eq!(asked(&read), 3);
    assert!(!show(&mut read, eye, &mut out), "none is left to ask");
}

/// The asks of a place with nothing known under it end, the eye standing
/// still, however short the budget, and it stands on nothing.
#[test]
fn the_asks_of_a_place_over_nothing_known_end() {
    let mut read: Vec<Place> = standing()
        .into_iter()
        .map(|p| Place { top: None, ..p })
        .collect();
    let [x, y, _] = q3::unpack(&read[0].ground.mid());
    let eye = [x, y, 500.0];
    let mut out = Vec::new();
    budget(true);
    let runs = (0..8)
        .take_while(|_| show(&mut read, eye, &mut out))
        .count();
    assert!(runs < 3, "the asks end: {runs} runs left some");
    assert!(read.iter().all(|p| p.asked.is_some()));
    assert!(out.is_empty(), "nothing stands on nothing known");
}

/// A place's brick at any zoom lies over or under its own brick, on its
/// brick's lap of X: at its own zoom it is its brick, a finer one's ancestor
/// there is its brick, and a coarser one is its brick's ancestor.
#[test]
fn a_place_s_brick_at_a_zoom_is_in_line_with_its_own() {
    for lap in [-1, 0, 1] {
        let p = place_in(12, lap);
        let (zoom, column) = p.brick;
        assert_eq!(p.brick_at(zoom), p.brick);
        for z in zoom + 1..=zoom + 4 {
            let (_, finer) = p.brick_at(z);
            assert_eq!(ancestor_of(finer, z - zoom), column, "lap {lap}, zoom {z}");
        }
        for z in zoom - 4..zoom {
            let coarser = (z, ancestor_of(column, zoom - z));
            assert_eq!(p.brick_at(z), coarser, "lap {lap}, zoom {z}");
        }
    }
}
