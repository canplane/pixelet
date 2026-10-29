// pxl-kernel/domain/scene/region.rs

//! What the scene animates, as each turn draws it (`drawn`): each draw its
//! sphere in the world's doubles and its state, so the part of the screen
//! that changed is found by comparing the turn's draws with the last's
//! (`take`), where a draw moved, changed or went, and where it was. The
//! renderer draws that part again alone where nothing else moved (pxl-render
//! `FrameInput::region`). Whether anything it animates stepped this turn,
//! waking the turn at its own steps, is marked beside (`animated`).

use pxl_render::frame::draw::DrawList;
use pxl_spec::space::coord::mat4::Mat4;
use pxl_spec::space::coord::v3::{self, V3};
use xpute_guest::global::Global;

/// How far an actor's shadow reaches along the ground, in its radii over
/// the cotangent of the sun's height.
const SHADOW_RADII: f64 = 2.0;

/// The sun's least height, its sine, a shadow's reach is bounded at: under
/// it a shadow falls too far to bound, and the frame is drawn whole.
const SHADOW_LEAST_SUN: f64 = 0.25;

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

/// A sphere's shadow toward the ground, away from the sun `sun`, a unit
/// vector: a sphere about where it falls.
fn shadow(d: &Drawn, sun: V3) -> (V3, f64) {
    let reach = SHADOW_RADII * d.radius / sun[2];
    (
        [
            d.at[0] - sun[0] * reach,
            d.at[1] - sun[1] * reach,
            d.at[2] - sun[2] * reach,
        ],
        d.radius + reach / 2.0,
    )
}

/// The clip `[x0, y0, x1, y1]` covering what changed between the last
/// turn's draws and this one's, where each was and is, and their shadows
/// from the sun `sun`, through `view_proj`, the eye's frame about `eye`, in
/// the world's doubles; none where nothing changed, a shadow that changed
/// falls too far, or any of it stands about or behind the eye. And whether
/// what is animated stepped. The turn's draws become the last.
pub fn take(view_proj: &Mat4, eye: V3, sun: V3) -> (Option<[f32; 4]>, bool) {
    let m = moved();
    let animated = core::mem::take(&mut m.animated);
    let mut changed: Vec<Drawn> = Vec::new();
    for d in &m.now {
        match m.last.iter().find(|l| l.key == d.key) {
            Some(l) if l.state == d.state => {}
            Some(l) => changed.extend([*d, *l]),
            None => changed.push(*d),
        }
    }
    for l in &m.last {
        if !m.now.iter().any(|d| d.key == l.key) {
            changed.push(*l);
        }
    }
    m.last = core::mem::take(&mut m.now);
    if changed.is_empty() {
        return (None, animated);
    }
    if changed.iter().any(|d| d.casts) && sun[2] < SHADOW_LEAST_SUN {
        return (None, animated);
    }
    let spheres = changed.iter().flat_map(|d| {
        let cast = d.casts.then(|| shadow(d, sun));
        [Some((d.at, d.radius)), cast].into_iter().flatten()
    });
    let mut clip: Option<[f64; 4]> = None;
    for (at, r) in spheres {
        let p = DrawList::translation(at, eye).map(f64::from);
        if v3::len(&p) <= r {
            return (None, animated);
        }
        for k in 0..8 {
            let c: V3 = core::array::from_fn(|a| p[a] + if k >> a & 1 == 1 { r } else { -r });
            let w =
                view_proj[3] * c[0] + view_proj[7] * c[1] + view_proj[11] * c[2] + view_proj[15];
            if w.is_nan() || w <= 0.0 {
                return (None, animated);
            }
            let x =
                (view_proj[0] * c[0] + view_proj[4] * c[1] + view_proj[8] * c[2] + view_proj[12])
                    / w;
            let y =
                (view_proj[1] * c[0] + view_proj[5] * c[1] + view_proj[9] * c[2] + view_proj[13])
                    / w;
            let b = clip.get_or_insert([x, y, x, y]);
            *b = [b[0].min(x), b[1].min(y), b[2].max(x), b[3].max(y)];
        }
    }
    (clip.map(|b| b.map(|v| v.clamp(-1.0, 1.0) as f32)), animated)
}
