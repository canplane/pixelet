// pxl-core/domain/scene/character.test.rs

use super::*;
use crate::domain::scene::model::registry::test::swaying;
use crate::sys::host::set_clock;
use pxl_render::gpu::draw::{lists_sz, set_lists};

/// How many display pixels one unit spans at one unit from the eye.
const PX_PER_UNIT: f64 = 1000.0;

/// Ticks a character playing the swaying clip, alone at `dist` from the eye,
/// for two seconds at 60 Hz. It returns, for each tick, whether the tick
/// asked for a frame and whether the clip stepped.
fn idle(dist: f64) -> Vec<(bool, bool)> {
    const LIVE: usize = 16;
    set_lists(Vec::leak(vec![0; lists_sz(LIVE) as usize / 4]), LIVE);
    let mut draw = DrawList::default();
    let mut models = ModelTable::default();
    let model = models.add(&mut draw, swaying());
    let mut character = Character::new(&mut draw, &models, model, Group::Person, 0).unwrap();
    character.play(&models, 0, 0.0);
    let frame = Frame {
        camera: [dist, 0.0, 0.0],
        px_per_unit: PX_PER_UNIT,
        delta_s: 1.0 / 60.0,
        ..Frame::default()
    };
    character.transform = Some(Transform {
        at: [0.0; 3],
        rot: 0.0,
        scale: 1.0,
    });
    animation::take();
    let ticks = (1..=120)
        .map(|k| {
            set_clock(f64::from(k) * 1000.0 / 60.0);
            let asks = character.tick(&mut draw, &models, &frame, false);
            (asks, animation::take())
        })
        .collect();
    character.free(&mut draw);
    ticks
}

/// The number of steps the clip takes over one loop from its start, where a
/// step is a pixel's motion at `dist` (`Model::next_step`).
fn clip_steps(dist: f64) -> usize {
    let m = swaying();
    let far = (dist / PX_PER_UNIT) as f32;
    let (mut t, mut n) = (0.0, 0);
    while t < 1.0 {
        t = m.next_step(0, t, far);
        n += 1;
    }
    n
}

/// Near the eye, the swaying clip moves a pixel within a frame, so it steps
/// every tick except where it slows to turn back. Far from the eye, it steps
/// when the clip's own steps fall, at most a frame late, and stands still
/// between them. Either way it asks for no frame: the frame is woken for the
/// next step, and only a step notes the animation that draws it.
#[test]
fn idle_asks_nothing_between_steps() {
    let near = idle(3.0);
    assert!(
        near.iter().all(|&(asks, _)| !asks),
        "a near step asked a frame"
    );
    // At tick k the clip has run k + 1 frames, and the sway turns back every
    // half second, starting at a quarter second.
    let by_turn = |k: usize| (((k + 1) as f64 / 60.0) % 0.5 - 0.25).abs() <= 2.0 / 60.0;
    let still: Vec<usize> = (0..near.len()).filter(|&k| !near[k].1).collect();
    assert!(
        !still.is_empty() && still.iter().all(|&k| by_turn(k)),
        "near, the ticks {still:?} did not step, where they stand by no turn"
    );
    let far = idle(150.0);
    assert!(
        far.iter().all(|&(asks, _)| !asks),
        "a far step asked a frame"
    );
    let stepped = far.iter().filter(|&&(_, s)| s).count();
    let most = 2 * clip_steps(150.0) + 1;
    assert!(
        stepped > 2 && stepped <= most,
        "far, it stepped {stepped} of 120 ticks, its clip {most} steps over two loops at most"
    );
}

#[test]
fn a_body_is_drawn_clear_of_the_wall_beside_it() {
    use crate::domain::scene::character::companion::clear_of_walls;
    let step_east = |[x, _]: [f64; 2]| x >= 10.0;
    assert_eq!(clear_of_walls([9.5, 3.5], 0.25, 1.0, step_east), [9.5, 3.5]);
    assert_eq!(
        clear_of_walls([9.9, 3.5], 0.25, 1.0, step_east),
        [9.75, 3.5]
    );
    assert_eq!(
        clear_of_walls([9.99, 3.5], 0.25, 1.0, step_east),
        [9.75, 3.5]
    );
    let step_south = |[_, y]: [f64; 2]| y < 3.0;
    assert_eq!(
        clear_of_walls([5.5, 3.1], 0.25, 1.0, step_south),
        [5.5, 3.25]
    );
}

#[test]
fn a_body_is_held_off_a_wall_by_the_width_of_a_cell() {
    use crate::domain::layer::object::store::Store;
    use crate::domain::query::change::Memo;
    use crate::domain::query::test::{flat_at_five, placed, write};
    use crate::domain::scene::character::companion::{Camera, drawn_at};
    use pxl_spec::asset::{AssetId, cube};
    use pxl_spec::bake::constant::OBJECT_COARSEST;
    use pxl_spec::space::coord::world;
    let [x, y] = flat_at_five();
    let mut store = Store::default();
    let wall = cube::Object::Red as AssetId;
    write(&mut store, [x + 1.0, y, 5.0], OBJECT_COARSEST, wall);
    placed(store);
    let at = [x + 0.9, y + 0.5];
    let camera = Camera {
        pivot: at,
        floor: Some(5.0),
        azimuth: 0.0,
        dragging: false,
        steered: false,
        following: false,
        eye: world::point(at[0], at[1] - 3.0, 8.0),
    };
    let mut memo = Memo::NONE;
    let [drawn_x, drawn_y, _] = drawn_at(&mut memo, at, 5.0, 0.25, &camera);
    assert_eq!([drawn_x, drawn_y], [x + 0.75, y + 0.5]);
}

/// Whether a character alone at `dist` from the eye is drawn after a tick,
/// and whether a line hangs over its head.
fn drawn(dist: f64) -> (bool, bool) {
    const LIVE: usize = 16;
    set_lists(Vec::leak(vec![0; lists_sz(LIVE) as usize / 4]), LIVE);
    let mut draw = DrawList::default();
    let mut models = ModelTable::default();
    let model = models.add(&mut draw, swaying());
    let mut character = Character::new(&mut draw, &models, model, Group::Person, 0).unwrap();
    let frame = Frame {
        camera: [dist, 0.0, 0.0],
        px_per_unit: PX_PER_UNIT,
        ..Frame::default()
    };
    character.transform = Some(Transform {
        at: [0.0; 3],
        rot: 0.0,
        scale: 1.0,
    });
    character.tick(&mut draw, &models, &frame, false);
    let drawn = (character.visible, character.over_head().is_some());
    character.free(&mut draw);
    drawn
}

/// A character is drawn out to where the view stops drawing cells a zoom
/// coarser than the leaves', where a decor of its cell stops being drawn, and
/// not from there on, however large its model; and a line hangs over its
/// head only while it is drawn.
#[test]
fn a_character_is_drawn_where_it_stands() {
    use pxl_spec::space::coord::zoom::LEAF_ZOOM;
    use pxl_spec::space::layer::lattice::{GROUND, draw_dist};
    let edge = draw_dist(GROUND, LEAF_ZOOM - 1);
    assert_eq!(drawn(edge * 0.99), (true, true));
    assert_eq!(drawn(edge), (false, false));
}

/// A companion's step is walked only where the body stands at both its
/// ends for the camera's eye, a height not known taken where it stands.
#[test]
fn a_step_is_walked_where_the_walker_stands_at_both_its_ends() {
    use crate::domain::layer::object::store::Store;
    use crate::domain::query::test::{flat_at_five, placed};
    use crate::domain::query::{is_unknown, over, unknown};
    use crate::domain::scene::character::companion::{Camera, walk_to};
    use pxl_spec::space::coord::world;
    use pxl_spec::space::coord::zoom::LEAF_ZOOM;
    use pxl_spec::space::layer::lattice::{GROUND, draw_dist};
    let [x, y] = flat_at_five();
    let world = placed(Store::default());
    let edge = draw_dist(GROUND, LEAF_ZOOM - 1);
    let camera = |eye: [f64; 3]| Camera {
        pivot: [x + 0.5, y + 0.5],
        floor: Some(5.0),
        azimuth: 0.0,
        dragging: false,
        steered: false,
        following: false,
        eye: world::point(eye[0], eye[1], eye[2]),
    };
    let from = world::point(x + 0.5, y + 0.5, over(5.0));
    let beside = camera([x + 0.5, y - 3.0, 8.0]);
    let near = [x + 3.5, y + 0.5];
    assert_eq!(
        walk_to(world, &from, near, &beside),
        world::point(near[0], near[1], over(5.0)),
        "walked"
    );
    // The far end lies past where the body stands.
    let far = [x + 0.5 + edge, y + 0.5];
    assert!(is_unknown(&walk_to(world, &from, far, &beside)));
    // A body of unknown height is measured where it stands, not at its
    // unknown height's mark.
    assert_eq!(
        walk_to(world, &unknown(&from), near, &beside),
        world::point(near[0], near[1], over(5.0)),
        "walked from a height not known"
    );
}
