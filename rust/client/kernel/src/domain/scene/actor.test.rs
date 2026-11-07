// pxl-kernel/domain/scene/actor.test.rs

use std::sync::atomic::{AtomicU64, Ordering};

use super::*;
use crate::domain::scene::model::registry::test::swaying;
use crate::sys;
use pxl_render::frame::draw::{lists_bytes, set_lists};
use xpute_core::clock;

/// The wall clock the ticks read, in milliseconds, as its bits.
static WALL_MS: AtomicU64 = AtomicU64::new(0);

fn wall_ms() -> f64 {
    f64::from_bits(WALL_MS.load(Ordering::Relaxed))
}

/// The display's pixels a unit spans a unit from the eye.
const PX_PER_UNIT: f64 = 1000.0;

/// Two seconds of ticks at 60 Hz, the swaying clip standing alone `dist`
/// from the eye: for each, whether it asked a frame and whether it stepped.
fn idle(dist: f64) -> Vec<(bool, bool)> {
    const LIVE: usize = 16;
    let (at, _) = alloc(lists_bytes(LIVE)).unwrap();
    // SAFETY: a block of the heap's, the lists' alone for the test's run.
    unsafe { set_lists(at, LIVE) };
    let mut draw = DrawList::default();
    let mut models = Models::default();
    let model = models.add(&mut draw, swaying());
    let mut actor = Actor::new(&mut draw, &models, model).unwrap();
    actor.play(0, 0.0);
    let frame = Frame {
        camera: [dist, 0.0, 0.0],
        px_per_unit: PX_PER_UNIT,
        delta_s: 1.0 / 60.0,
        ..Frame::default()
    };
    let transform = Transform {
        at: [0.0; 3],
        rot_z: 0.0,
        scale: 1.0,
    };
    clock::set(wall_ms);
    animation::take();
    let ticks = (1..=120)
        .map(|k| {
            WALL_MS.store((f64::from(k) * 1000.0 / 60.0).to_bits(), Ordering::Relaxed);
            let asks = actor.tick(&mut draw, &models, &frame, &transform);
            (asks, animation::take())
        })
        .collect();
    clock::set(|| 0.0);
    actor.free(&mut draw);
    ticks
}

/// The clip's own steps over a loop, from its start, a pixel at `dist`
/// a step (`Model::next_step`).
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

/// Near, the swaying clip goes a pixel within a frame, so it steps every
/// tick but where it slows to turn back; far, it steps as its clip's steps
/// fall, a frame late at most, and stands between them. Either way it asks
/// no frame: the turn is woken for the next step, and only a step notes the
/// animation that draws it.
#[test]
fn idle_asks_nothing_between_steps() {
    let _g = sys::serial();
    let near = idle(3.0);
    assert!(
        near.iter().all(|&(asks, _)| !asks),
        "a near step asked a frame"
    );
    // The clip has run a frame for each tick so far, and the sway turns
    // back every half second from a quarter on.
    let by_turn = |k: usize| (((k + 1) as f64 / 60.0) % 0.5 - 0.25).abs() <= 2.0 / 60.0;
    let held: Vec<usize> = (0..near.len()).filter(|&k| !near[k].1).collect();
    assert!(
        !held.is_empty() && held.iter().all(|&k| by_turn(k)),
        "near, the ticks {held:?} did not step, where they stand by no turn"
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
