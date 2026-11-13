// pxl-core/domain/scene/character.test.rs

use std::sync::atomic::{AtomicU64, Ordering};

use super::*;
use crate::domain::scene::model::registry::test::swaying;
use pxl_render::gpu::draw::{lists_bytes, set_lists};
use xpute_guest::clock;

/// The wall clock the ticks read, in milliseconds, held as the f64's bits.
static WALL_MS: AtomicU64 = AtomicU64::new(0);

fn wall_ms() -> f64 {
    f64::from_bits(WALL_MS.load(Ordering::Relaxed))
}

/// How many display pixels one unit spans at one unit from the eye.
const PX_PER_UNIT: f64 = 1000.0;

/// Ticks a character playing the swaying clip, alone at `dist` from the eye,
/// for two seconds at 60 Hz. It returns, for each tick, whether the tick
/// asked for a frame and whether the clip stepped.
fn idle(dist: f64) -> Vec<(bool, bool)> {
    const LIVE: usize = 16;
    set_lists(Vec::leak(vec![0; lists_bytes(LIVE) as usize / 4]), LIVE);
    let mut draw = DrawList::default();
    let mut models = ModelTable::default();
    let model = models.add(&mut draw, swaying());
    let mut character = Character::new(&mut draw, &models, model, Group::person, 0).unwrap();
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
    clock::set(wall_ms);
    animation::take();
    let ticks = (1..=120)
        .map(|k| {
            WALL_MS.store((f64::from(k) * 1000.0 / 60.0).to_bits(), Ordering::Relaxed);
            let asks = character.tick(&mut draw, &models, &frame, false);
            (asks, animation::take())
        })
        .collect();
    clock::set(|| 0.0);
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
    crate::on_core(|| {
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
    });
}

#[test]
fn a_body_is_drawn_clear_of_the_wall_beside_it() {
    crate::on_core(|| {
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
    });
}

#[test]
fn a_body_is_held_off_a_wall_by_the_width_of_a_cell() {
    crate::on_core(|| {
        use crate::domain::layer::object::store::Store;
        use crate::domain::query::change::Memo;
        use crate::domain::query::test::{flat_at_five, placed, write};
        use crate::domain::scene::character::companion::{drawn_at, Camera};
        use pxl_spec::asset::{cube, AssetId};
        use pxl_spec::bake::constant::OBJECT_COARSEST;
        use pxl_spec::space::coord::world;
        let [x, y] = flat_at_five();
        let mut store = Store::default();
        let wall = cube::Object::red as AssetId;
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
    });
}
