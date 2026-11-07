// pxl-render/gpu/draw.test.rs

use super::*;
use crate::frame::shader::record::PIPELINES;
use pxl_spec::space::coord::q3;
use pxl_spec::space::coord::world::{nearest, ROUND};

/// The eye-relative translation the records held before they held a `Position`: the
/// f64 difference narrowed to f32, what a `Position` from the eye is held to.
fn translation(at: V3, eye: V3) -> [f32; 3] {
    core::array::from_fn(|k| (at[k] - eye[k]) as f32)
}

/// A fixed sequence of numbers in [0, 1).
struct Noise(u64);

impl Noise {
    fn next(&mut self) -> f64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 11) as f64 / (1u64 << 53) as f64
    }

    /// In [-half, half).
    fn about(&mut self, half: f64) -> f64 {
        (self.next() * 2.0 - 1.0) * half
    }
}

/// How many f32s lie from `a` to `b`.
fn ulps(a: f32, b: f32) -> u32 {
    let key = |v: f32| {
        let i = v.to_bits() as i32;
        if i < 0 {
            i32::MIN - i
        } else {
            i
        }
    };
    key(a).abs_diff(key(b))
}

/// Units on Q3.
fn on_q3(units: f64) -> f64 {
    q3::to_real(from_real(units))
}

/// The world's X round in units, half each side of 0.
const HALF_ROUND: f64 = ROUND / 2.0;
/// The draw distance's 2^17 units (pxl-core control/param.rs `MAX_DIST`).
const DRAW_DIST: f64 = (1 << 17) as f64;

/// An eye on Q3 anywhere over the world and a point `reach` from it.
fn pair(n: &mut Noise, reach: f64) -> (V3, V3) {
    let eye = [
        on_q3(n.about(HALF_ROUND)),
        on_q3(n.about(HALF_ROUND - DRAW_DIST)),
        on_q3(n.about(30_000.0)),
    ];
    let at = [0, 1, 2].map(|k| eye[k] + n.about(reach));
    (eye, at)
}

/// The remainder lies in [0, 1/256) on either side of 0, and the position
/// is its Q3 and its remainder, to the remainder's f32.
#[test]
fn split_by_floor() {
    let mut n = Noise(0x9e37_79b9_7f4a_7c15);
    for _ in 0..100_000 {
        let v = [0, 1, 2].map(|_| n.about(HALF_ROUND));
        let p = Position::of(v);
        for k in 0..3 {
            assert!(p.r[k] >= 0.0 && p.r[k] < 1.0 / 256.0, "{v:?}: {:?}", p.r);
            let back = q3::to_real(i64::from(p.q[k])) + f64::from(p.r[k]);
            assert!((back - v[k]).abs() <= 1.0 / 256.0 * f64::EPSILON, "{v:?}");
        }
    }
    let p = Position::of([-1.0 / 512.0, -3.0, -0.0]);
    assert_eq!(p.q, [-1, -768, 0]);
    assert_eq!(p.r, [1.0 / 512.0, 0.0, 0.0]);
}

/// On Q3, a record's position from the eye is today's translation's, bit
/// for bit: near the eye, out to the draw distance, at negative
/// coordinates, 100,000 units under the eye, and across X's
/// seam, where today's needs the copy nearest the eye.
#[test]
fn on_q3_as_translated() {
    let mut n = Noise(0x2545_f491_4f6c_dd1d);
    for reach in [4.0, 1000.0, DRAW_DIST] {
        for _ in 0..100_000 {
            let (eye, at) = pair(&mut n, reach);
            let at = at.map(on_q3);
            let p = Position::of(at);
            assert_eq!(p.r, [0.0; 3]);
            let got = p.from_eye(q3_of(eye));
            let want = translation([nearest(at[0], eye[0]), at[1], at[2]], eye);
            assert_eq!(
                got.map(f32::to_bits),
                want.map(f32::to_bits),
                "{at:?} from {eye:?}"
            );
        }
    }
    let eye = [HALF_ROUND - 0.5, -1000.0, 12.0];
    for at in [
        [-HALF_ROUND + 3.25, -1001.5, 11.0],
        [HALF_ROUND + 3.25, -1001.5, 11.0],
        [eye[0], eye[1], eye[2] - 100_000.0],
    ] {
        let got = Position::of(at).from_eye(q3_of(eye));
        let want = translation([nearest(at[0], eye[0]), at[1], at[2]], eye);
        assert_eq!(got.map(f32::to_bits), want.map(f32::to_bits), "{at:?}");
    }
    // A copy a lap away is the same record.
    assert_eq!(
        Position::of([eye[0] + ROUND, 2.0, 3.0]),
        Position::of([eye[0], 2.0, 3.0])
    );
}

/// Finer than Q3, a record's position from the eye is within an ulp of
/// today's translation, or of the remainder's own f32 where a step's
/// difference cancels against it (`Position`), near and far.
#[test]
fn finer_within_an_ulp() {
    let mut n = Noise(0xd1b5_4a32_d192_ed03);
    let (mut asked, mut off) = (0, 0);
    for reach in [1.0 / 128.0, 4.0, 1000.0, DRAW_DIST] {
        for _ in 0..100_000 {
            let (eye, at) = pair(&mut n, reach);
            let got = Position::of(at).from_eye(q3_of(eye));
            let want = translation([nearest(at[0], eye[0]), at[1], at[2]], eye);
            for k in 0..3 {
                let ulp = f64::from(f32::from_bits(want[k].abs().to_bits() + 1) - want[k].abs());
                let gap = (f64::from(got[k]) - f64::from(want[k])).abs();
                assert!(
                    ulps(got[k], want[k]) <= 1 || gap <= ulp + R_HALF_ULP,
                    "{at:?} from {eye:?}: {got:?}, {want:?}"
                );
                asked += 1;
                off += usize::from(got[k] != want[k]);
            }
        }
    }
    assert!(off * 10 < asked, "{off} of {asked} off");
}

/// Half an f32's ulp at the remainder's largest, 1/256: how far its own
/// rounding moves it.
const R_HALF_ULP: f64 = 1.0 / (1u64 << 33) as f64;

/// A draw set as it was is no change, its i32 words read bit for bit, and its stand is handed
/// again; a draw that changed is one with what it was, one drawn first one with nothing, and
/// one no longer set one let go, its slot drawn anew under another name.
#[test]
fn a_draw_set_as_it_was_is_no_change() {
    let _l = test_lists();
    let mut d = DrawList::default();
    let (marker, ghost, debug) = (DrawId::default(), DrawId::default(), DrawId::default());
    // An i32 whose bits read as a NaN.
    let at = Position::of([q3::to_real(-1), 5.5, 1.0e-3]);
    let box_at = Stand::of([-1.0, 5.0, 0.0], [1.0, 6.0, 2.0]);
    let turn = |d: &mut DrawList, stand: Stand, debug_at: Option<f64>| {
        d.draw_as(&marker, PIPELINE_MARKER, 3, 2, 0, at, None);
        d.draw_in(
            &ghost,
            PIPELINE_GHOST_BOX,
            4,
            1,
            0,
            at,
            0.5,
            1.0,
            0.25,
            Some(stand),
        );
        if let Some(x) = debug_at {
            d.draw_as(&debug, PIPELINE_DEBUG, 5, 1, 0, Position::of([x; 3]), None);
        }
    };
    turn(&mut d, box_at, None);
    let l = d.take();
    assert_eq!(l.changes.len(), 2, "both drawn first");
    assert!(l.changes.iter().all(|c| c.was.is_none()));
    turn(&mut d, box_at, None);
    let l = d.take();
    assert!(l.changes.is_empty() && !l.reordered);
    assert_eq!(l.stands, &[box_at]);
    let moved = Stand::of([-1.0, 5.0, 0.0], [1.0, 6.0, 3.0]);
    turn(&mut d, moved, None);
    let l = d.take();
    assert!(l.changes.is_empty(), "a stand is no draw's");
    assert_eq!(l.stands, &[moved], "a stand moved");
    turn(&mut d, box_at, Some(0.0));
    let l = d.take();
    assert_eq!(l.changes.len(), 1);
    let slot = l.changes[0].slot;
    assert!(l.changes[0].was.is_none() && l.draws.now(slot).is_some());
    turn(&mut d, box_at, Some(1.0));
    let l = d.take();
    let was = l.changes[0]
        .was
        .map(|(r, u)| (r[0], Position::in_uniform(&u)));
    assert_eq!(
        (l.changes.len(), was),
        (1, Some((5, Position::of([0.0; 3]))))
    );
    turn(&mut d, box_at, None);
    let l = d.take();
    assert_eq!(l.changes.len(), 1);
    assert!(
        l.changes[0].was.is_some() && l.draws.now(slot).is_none(),
        "let go"
    );
    assert_eq!(l.draws.of(PIPELINE_DEBUG).len(), 0);
    turn(&mut d, box_at, Some(1.0));
    let l = d.take();
    assert_eq!(l.changes.len(), 1);
    assert!(l.changes[0].was.is_none(), "drawn anew");
}

/// Each pipeline's draws are drawn in the order the turn set them, a draw set in another
/// order said so, and a pipeline changed moves the draw into the other's.
#[test]
fn draws_keep_the_order_they_are_set_in() {
    let _l = test_lists();
    let mut d = DrawList::default();
    let ids: Vec<DrawId> = (0..3).map(|_| DrawId::default()).collect();
    let set = |d: &mut DrawList, order: &[usize], pipeline: [u32; 3]| {
        for &k in order {
            d.draw_as(
                &ids[k],
                pipeline[k],
                k as u32 + 1,
                0,
                0,
                Position::NONE,
                None,
            );
        }
    };
    let handles = |l: &Lists, p: u32| l.draws.of(p).map(|r| r[0]).collect::<Vec<_>>();
    let m = PIPELINE_MARKER;
    set(&mut d, &[0, 1, 2], [m; 3]);
    let l = d.take();
    assert_eq!(handles(&l, m), [1, 2, 3]);
    set(&mut d, &[2, 0, 1], [m; 3]);
    let l = d.take();
    assert_eq!(handles(&l, m), [3, 1, 2]);
    assert!(l.changes.is_empty() && l.reordered);
    set(&mut d, &[2, 0, 1], [m, PIPELINE_DEBUG, m]);
    let l = d.take();
    assert_eq!(
        (handles(&l, m), handles(&l, PIPELINE_DEBUG)),
        (vec![3, 1], vec![2])
    );
    assert_eq!(l.changes.len(), 1);
}

/// A draw set twice in a turn is refused.
#[test]
#[should_panic]
fn a_draw_set_twice_is_refused() {
    let _l = test_lists();
    let mut d = DrawList::default();
    let id = DrawId::default();
    for _ in 0..2 {
        d.draw_as(&id, PIPELINE_MARKER, 1, 0, 0, Position::NONE, None);
    }
}

/// The draws that read the shadow map are the lit ones: decor, its ghost and impostor, the
/// characters, the ghost box and the beacon; the zone, the markers and the debug lines read none.
#[test]
fn lit_draws_read_the_map() {
    for p in [
        PIPELINE_DECOR,
        PIPELINE_GHOST_BOX,
        PIPELINE_GHOST_DECOR,
        PIPELINE_SKINNED,
        PIPELINE_LOCATION_BEACON,
        PIPELINE_IMPOSTOR,
    ] {
        assert!(reads_shadow_map(p), "{p}");
    }
    for p in [PIPELINE_LOCATION_ZONE, PIPELINE_MARKER, PIPELINE_DEBUG] {
        assert!(!reads_shadow_map(p), "{p}");
    }
    assert!((0..PIPELINES).any(|p| !reads_shadow_map(p)));
}

/// A draw that reads the map stands its box; past `light::STANDS` the boxes are taken into the
/// last, which holds every one.
#[test]
fn every_lit_draw_stands_its_box() {
    let mut d = DrawList::default();
    let n = light::STANDS + 5;
    let unit = |v: usize| (v as i64 * ONE) as i32;
    let ids: Vec<DrawId> = (0..=n).map(|_| DrawId::default()).collect();
    for (k, id) in ids.iter().enumerate().take(n) {
        let v = k as f64;
        d.draw_as(
            id,
            PIPELINE_LOCATION_BEACON,
            0,
            0,
            0,
            Position::of([v; 3]),
            Some(Stand::of([v; 3], [v + 1.0; 3])),
        );
    }
    d.draw_as(&ids[n], PIPELINE_MARKER, 0, 0, 0, Position::NONE, None);
    assert_eq!(d.stands.len(), light::STANDS);
    let last = d.stands[light::STANDS - 1];
    assert_eq!(last, Stand([[unit(light::STANDS - 1); 3], [unit(n); 3]]));
    for (k, b) in d.stands[..light::STANDS - 1].iter().enumerate() {
        assert_eq!(*b, Stand([[unit(k); 3], [unit(k + 1); 3]]));
    }
}

/// Boxes taken into one across X's seam hold both the short way round, not the round's
/// width.
#[test]
fn boxes_held_across_the_seam() {
    let mut s = Stand([[i32::MAX - 10, 0, 0], [i32::MAX - 5, 4, 4]]);
    s.hold(&Stand([[i32::MIN + 2, -3, 1], [i32::MIN + 9, 2, 8]]));
    assert_eq!(s, Stand([[i32::MAX - 10, -3, 0], [i32::MIN + 9, 4, 8]]));
    let [lo, hi] = s.from_eye([i32::MAX - 10, 0, 0]);
    assert_eq!(lo, [0.0, -3.0 / 256.0, 0.0]);
    assert_eq!(hi, [20.0 / 256.0, 4.0 / 256.0, 8.0 / 256.0]);
}

/// A cloud's draw at its place on the field's map, moved back by the drift as the shaders
/// move it (`drifted`, instance.wgsl `draw_translation`), stands from the eye where the place
/// as the drift leaves it does, within an ulp and the remainder's own rounding, across X's
/// seam too; one that does not drift is not moved.
#[test]
fn a_drifted_cloud_stands_where_the_drift_leaves_it() {
    let mut n = Noise(0x3c6e_f372_fe94_f82b);
    for _ in 0..100_000 {
        let at = [n.about(ROUND), n.about(4.0e6), 0.0];
        let off = [n.next() * 1.0e5, n.next() * 4.0e4, 0.0];
        let eye = [
            at[0] - off[0] + n.about(DRAW_DIST),
            at[1] - off[1] + n.about(DRAW_DIST),
            0.0,
        ];
        let p = Position::of(at);
        let mut u = [0.0f32; DRAW_FLOATS];
        u[draw::Q..draw::Q + 3].copy_from_slice(&p.q_words());
        u[draw::R..draw::R + 3].copy_from_slice(&p.r);
        u[draw::DRIFTS] = 1.0;
        drifted(&mut u, Position::of(off));
        assert_eq!(u[draw::DRIFTS], 0.0);
        let got = Position::in_uniform(&u).from_eye(q3_of(eye));
        let want = Position::of([at[0] - off[0], at[1] - off[1], 0.0]).from_eye(q3_of(eye));
        for k in 0..2 {
            let slack = 2.0 * f32::EPSILON * want[k].abs() + 4.0 / 256.0 * f32::EPSILON;
            assert!(
                (got[k] - want[k]).abs() <= slack,
                "{at:?} off {off:?}: {got:?} {want:?}"
            );
        }
    }
    let mut u = [1.0f32; DRAW_FLOATS];
    u[draw::DRIFTS] = 0.0;
    drifted(&mut u, Position::of([3.0, 4.0, 0.0]));
    assert_eq!(u, {
        let mut w = [1.0f32; DRAW_FLOATS];
        w[draw::DRIFTS] = 0.0;
        w
    });
}

/// A ball's box holds the ball decor_ball.wgsl draws a receiver by.
#[test]
fn ball_holds_decor_ball() {
    let r = f64::from(TALLEST + WIDEST);
    let z = 3.0 + f64::from(TALLEST) * 0.5;
    assert_eq!(
        Stand::ball([1.0, 2.0, 3.0]),
        Stand::of([1.0 - r, 2.0 - r, z - r], [1.0 + r, 2.0 + r, z + r])
    );
}

/// A box is free of the eye, and what the light's passes read of it holds the box asked,
/// taken against the eye in f64, and stands off it by no more than a step and an ulp: near
/// the eye, out to the draw distance, at negative coordinates and across X's seam.
#[test]
fn a_stand_from_the_eye_holds_its_box() {
    let mut n = Noise(0x6a09_e667_f3bc_c909);
    for reach in [4.0, 1000.0, DRAW_DIST] {
        for _ in 0..100_000 {
            let (eye, lo) = pair(&mut n, reach);
            let hi = lo.map(|v| v + n.next() * 40.0);
            let [got_lo, got_hi] = Stand::of(lo, hi).from_eye(q3_of(eye));
            for k in 0..3 {
                let at = |v: f64| if k == 0 { nearest(v, eye[0]) } else { v } - eye[k];
                let (want_lo, want_hi) = (at(lo[k]), at(hi[k]));
                let slack = |v: f64| 1.0 / 256.0 + 2.0 * f64::from(f32::EPSILON) * v.abs();
                assert!(
                    f64::from(got_lo[k]) <= want_lo
                        && want_lo - f64::from(got_lo[k]) <= slack(want_lo),
                    "{lo:?} from {eye:?}: {got_lo:?}"
                );
                assert!(
                    f64::from(got_hi[k]) >= want_hi
                        && f64::from(got_hi[k]) - want_hi <= slack(want_hi),
                    "{hi:?} from {eye:?}: {got_hi:?}"
                );
            }
        }
    }
}

/// A draw that reads the map and gives no box is refused, so none skips standing.
#[test]
#[should_panic]
fn a_lit_draw_without_its_box_is_refused() {
    let mut d = DrawList::default();
    d.draw_as(
        &DrawId::default(),
        PIPELINE_LOCATION_BEACON,
        0,
        0,
        0,
        Position::NONE,
        None,
    );
}
