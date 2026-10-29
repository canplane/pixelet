// pxl-kernel/domain/scene/region.rs

//! What the scene animates, as each turn draws it (`drawn`): each draw its
//! sphere in the world's doubles and its state, so what changed is found by
//! comparing the turn's draws with the last's (`take`), where a draw moved,
//! changed or went, and where it was. The renderer draws again alone the
//! part of the screen and of the sun's map they cover, where nothing else
//! moved (pxl-render `FrameInput::moved`). Whether anything it animates
//! stepped this turn, waking the turn at its own steps, is marked beside
//! (`animated`).

use pxl_render::frame::draw::DrawList;
use pxl_spec::space::coord::v3::V3;
use xpute_guest::global::Global;

/// A draw as the turn listed it: its own key, its sphere, whether it
/// casts a shadow that moves with it, and its state.
#[derive(Clone, Copy)]
struct Drawn {
    key: u32,
    at: V3,
    radius: f64,
    casts: bool,
    state: u64,
}

#[derive(Default)]
struct Moved {
    now: Vec<Drawn>,
    last: Vec<Drawn>,
    animated: bool,
    /// What `take` last found changed, which the renderer reads.
    changed: Vec<[f32; 5]>,
    /// The keys of the turn's draws, which the renderer reads as the draws
    /// a region stands for.
    keys: Vec<u32>,
}

static MOVED: Global<Moved> = Global::new();

fn moved() -> &'static mut Moved {
    MOVED.get(Moved::default)
}

/// A draw this turn, `key` its own among the turn's: `radius` about `at`,
/// in the world's doubles, casting a shadow where `casts`, in `state`, a
/// digest of all that sets how it is drawn.
pub fn drawn(key: u32, at: V3, radius: f64, casts: bool, state: u64) {
    moved().now.push(Drawn {
        key,
        at,
        radius,
        casts,
        state,
    });
}

/// What the scene animates stepped this turn: what wakes the turn at its
/// own steps, so no frame is asked after it.
pub fn animated() {
    moved().animated = true;
}

/// A digest of `words`, for a draw's state.
pub fn digest(words: &[f64]) -> u64 {
    words.iter().fold(0xcbf2_9ce4_8422_2325, |h, w| {
        (h ^ w.to_bits()).wrapping_mul(0x0100_0000_01b3)
    })
}

/// What changed between the last turn's draws and this one's, where each
/// was and is: a sphere each, `[x, y, z]` in units from `eye`, given in the
/// world's doubles, its radius, and 1 where it casts a shadow that moves
/// with it (pxl-render `FrameInput::moved`); the keys of the turn's draws
/// (`FrameInput::bounded`). And whether what is animated stepped. The
/// turn's draws become the last.
pub fn take(eye: V3) -> (&'static [[f32; 5]], &'static [u32], bool) {
    let m = moved();
    let animated = core::mem::take(&mut m.animated);
    let sphere = |d: &Drawn| {
        let p = DrawList::translation(d.at, eye);
        [
            p[0],
            p[1],
            p[2],
            d.radius as f32,
            f32::from(u8::from(d.casts)),
        ]
    };
    m.changed.clear();
    for d in &m.now {
        match m.last.iter().find(|l| l.key == d.key) {
            Some(l) if l.state == d.state => {}
            Some(l) => m.changed.extend([sphere(d), sphere(l)]),
            None => m.changed.push(sphere(d)),
        }
    }
    for l in &m.last {
        if !m.now.iter().any(|d| d.key == l.key) {
            m.changed.push(sphere(l));
        }
    }
    m.keys.clear();
    m.keys.extend(m.now.iter().map(|d| d.key));
    m.last = core::mem::take(&mut m.now);
    let m: &'static Moved = m;
    (&m.changed, &m.keys, animated)
}
