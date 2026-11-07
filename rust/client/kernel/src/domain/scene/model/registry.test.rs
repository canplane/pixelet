// pxl-kernel/domain/scene/model/registry.test.rs

use std::pin::pin;
use std::task::{Context, Poll, Waker};

use super::*;

/// Keys of the swaying clip over its second.
const SWAY_KEYS: usize = 40;

/// How far the swaying clip turns its joint either way, in radians.
const SWAY_RAD: f32 = 0.3;

/// The times the swaying clip's motion turns back: where its sine peaks.
const SWAY_TURNS: [f32; 2] = [0.25, 0.75];

/// One joint and four vertices on it, a unit or two from its origin, and
/// one clip of a second that sways the joint about Z by a sine baked a key
/// every fortieth of a second, as an exported idle clip is baked.
pub(crate) fn swaying() -> Model {
    let vertices: [[f32; 3]; 4] = [
        [1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [-1.0, 0.0, 1.0],
        [0.5, 0.5, 2.0],
    ];
    let cnt = vertices.len();
    let mut m = Model::new(cnt as u32, 3, 1).expect("the heap gives the model room");
    // SAFETY: the model's block, just taken, its columns a vertex count apart.
    let w = unsafe {
        core::slice::from_raw_parts_mut(
            mem::at::<u32>(m.at()),
            cnt * MODEL_VERTEX_WORDS as usize + 3,
        )
    };
    w.fill(0);
    for (v, p) in vertices.iter().enumerate() {
        for k in 0..3 {
            w[v * 3 + k] = p[k].to_bits();
        }
        w[12 * cnt + v * 4] = 1.0f32.to_bits();
    }
    let rest = [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0];
    let identity = [
        1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
    ];
    m.joint(0, -1, rest, identity, identity);
    let clip = m.clip(1.0);
    let times: Vec<f32> = (0..=SWAY_KEYS)
        .map(|k| k as f32 / SWAY_KEYS as f32)
        .collect();
    let vals = times
        .iter()
        .flat_map(|&t| {
            let a = SWAY_RAD * libm::sinf(2.0 * core::f32::consts::PI * t);
            [0.0, 0.0, libm::sinf(a / 2.0), libm::cosf(a / 2.0)]
        })
        .collect();
    m.ch(
        clip,
        Channel {
            joint: 0,
            path: R,
            step: false,
            times,
            vals,
        },
    );
    let mut sealing = pin!(async move {
        // SAFETY: every vertex and index of the block written above.
        unsafe { m.seal(1.0, || core::future::ready(())).await };
        m
    });
    let Poll::Ready(m) = sealing
        .as_mut()
        .poll(&mut Context::from_waker(Waker::noop()))
    else {
        unreachable!("a pause that is ready never parks");
    };
    m
}

/// Its vertices as its clip skins them at `time`.
fn drawn(m: &Model, time: f32) -> Vec<[f32; 3]> {
    let n = m.joints();
    let (mut local, mut global) = (vec![[0.0; 10]; n], vec![[0.0; 16]; n]);
    let mut palette = vec![0.0; n * 16];
    m.pose(Some((0, time)), &mut local);
    m.palette(&local, &mut global, &mut palette);
    m.skin(&palette, 0..m.vertex_cnt as usize).collect()
}

/// The farthest any vertex stands at `to` from where it stood at `from`.
fn gone(from: &[[f32; 3]], to: &[[f32; 3]]) -> f32 {
    from.iter()
        .zip(to)
        .map(|(a, b)| libm::sqrtf((0..3).map(|i| (b[i] - a[i]) * (b[i] - a[i])).sum()))
        .fold(0.0, f32::max)
}

/// The clip's steps from `start` to its end, `far` a step.
fn steps(m: &Model, start: f32, far: f32) -> Vec<f32> {
    let mut at = vec![start];
    while at[at.len() - 1] < 1.0 {
        let next = m.next_step(0, at[at.len() - 1], far);
        assert!(
            next > at[at.len() - 1],
            "a step from {} stood still",
            at[at.len() - 1]
        );
        at.push(next);
    }
    at
}

/// A step goes as far as a pixel `far` is and no farther: what is drawn
/// stands within it of the pose drawn at the step's start until the next,
/// past it by no more than a turn between two keys' arc is longer than its
/// chord, here under a ten-thousandth (the joint turns 0.047 rad between
/// keys at most), and a step that ends at no turn and not at the clip's
/// end has gone it, all but the arc's shortfall.
#[test]
fn a_step_goes_a_pixel() {
    let m = swaying();
    let far = 0.01;
    // How much farther than the chord a point may go: an arc's over its chord.
    let bound = far * 1.0001 + 1e-6;
    let at = steps(&m, 0.0, far);
    for w in at.windows(2) {
        let (from, to) = (w[0], w[1]);
        let was = drawn(&m, from);
        let farthest = (1..=32)
            .map(|k| gone(&was, &drawn(&m, from + (to - from) * k as f32 / 32.0)))
            .fold(0.0, f32::max);
        assert!(
            farthest <= bound,
            "from {from} to {to} the pose went {farthest}, past {bound}"
        );
        if !SWAY_TURNS.contains(&to) && to < 1.0 {
            assert!(
                gone(&was, &drawn(&m, to)) >= 0.99 * far,
                "from {from} to {to} the pose went short of a pixel"
            );
        }
    }
}

/// Far, where a pixel spans more than the clip moves, it steps at its turns
/// and its end alone: three steps a loop of forty keys.
#[test]
fn far_steps_at_turns() {
    let m = swaying();
    assert_eq!(
        steps(&m, 0.0, 1.0e3),
        [0.0, SWAY_TURNS[0], SWAY_TURNS[1], 1.0]
    );
}

/// The clip's turns are where its motion turns back and nowhere else, and
/// a step reaches each turn after where it starts, whatever a pixel spans.
#[test]
fn steps_keep_turns() {
    let m = swaying();
    assert_eq!(m.clips[0].turns, SWAY_TURNS);
    for far in [0.003, 0.02, 0.1, 0.5] {
        for start in [0.0, 0.013, 0.26, 0.749] {
            let at = steps(&m, start, far);
            for turn in SWAY_TURNS.into_iter().filter(|&t| t > start) {
                assert!(
                    at.contains(&turn),
                    "from {start}, {far} a step, the steps {at:?} pass the turn at {turn}"
                );
            }
        }
    }
}
