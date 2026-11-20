// pxl-core/domain/smooth.test.rs

use super::*;

/// A flight of the tests' own, whatever the camera's is: van Wijk and Nuij's
/// own curve, at which a far way's top is as wide as the way is long.
const F: Flight = Flight {
    curve: 1.42,
    cruise: 6.0,
    rate: 20.0,
    cap: 131_072.0,
};

fn view(x: f64, w: f64) -> View {
    View { at: [x, 0.0], w }
}

/// Whether `at` is `to` to a part in a million of its width, where whoever
/// flies would end the flight.
fn there(at: View, to: View) -> bool {
    let off = libm::hypot(to.at[0] - at.at[0], to.at[1] - at.at[1]);
    off <= to.w * 1e-6 && (at.w / to.w - 1.0).abs() <= 1e-6
}

/// Flies from `from` to `to` at 60 steps a second, until it is there, and
/// returns every view on the way.
fn flown(from: View, to: View) -> Vec<View> {
    let (mut speed, mut at, mut seen) = (0.0, from, vec![from]);
    for _ in 0..100_000 {
        at = fly(&mut speed, at, to, &F, 1.0 / 60.0);
        seen.push(at);
        assert!(speed <= F.cruise, "never faster than the cruise: {speed}");
        if there(at, to) {
            return seen;
        }
    }
    panic!("the flight from {from:?} to {to:?} does not end");
}

/// Between two views near each other the way is a pan: the view barely
/// widens. Between two far apart it widens to about the ground between
/// them, and comes back to the width asked.
#[test]
fn a_near_way_is_a_pan_and_a_far_one_rises() {
    let near = flown(view(0.0, 16.0), view(4.0, 16.0));
    let widest = near.iter().map(|v| v.w).fold(0.0, f64::max);
    assert!(widest < 16.5, "a pan widens little: {widest}");
    let far = flown(view(0.0, 16.0), view(100_000.0, 16.0));
    let widest = far.iter().map(|v| v.w).fold(0.0, f64::max);
    assert!(
        (50_000.0..200_000.0).contains(&widest),
        "a far way rises to about the ground it crosses: {widest}"
    );
}

/// A flight never passes its end: along the ground it only nears it,
/// whatever the two views.
#[test]
fn a_flight_nears_its_end_and_arrives() {
    for (from, to) in [
        (view(0.0, 16.0), view(4.0, 16.0)),
        (view(0.0, 16.0), view(5_000.0, 300.0)),
        (view(0.0, 2_000.0), view(-70_000.0, 2.0)),
        (view(3.0, 16.0), view(3.0, 900.0)),
    ] {
        let seen = flown(from, to);
        let left = |v: &View| (to.at[0] - v.at[0]).abs();
        assert!(
            seen.windows(2).all(|p| left(&p[1]) <= left(&p[0]) + 1e-9),
            "from {from:?} to {to:?}"
        );
    }
}

/// Two views about one point differ by a zoom alone: the point stays, and
/// the width goes from one to the other without passing either.
#[test]
fn a_zoom_keeps_its_point() {
    for (w0, w1) in [(16.0, 900.0), (900.0, 16.0)] {
        let seen = flown(view(3.0, w0), view(3.0, w1));
        assert!(seen.iter().all(|v| v.at == [3.0, 0.0]));
        let (lo, hi) = (w0.min(w1), w0.max(w1));
        assert!(
            seen.iter()
                .all(|v| v.w >= lo * (1.0 - 1e-9) && v.w <= hi * (1.0 + 1e-9))
        );
    }
}

/// A far flight is flown at the cruise, so its time goes with the length of
/// its way, which grows with the logarithm of the ground crossed: ten times
/// the ground takes a little longer, not ten times as long.
#[test]
fn a_far_flight_takes_the_logarithm_of_its_ground() {
    let steps = |x: f64| flown(view(0.0, 16.0), view(x, 16.0)).len();
    let (a, b) = (steps(10_000.0), steps(100_000.0));
    assert!(b > a && b < 2 * a, "{a} then {b}");
}

/// A way too far to fly under the cap is crossed at the cap, a pan: the view
/// gets no wider, and half the world round is flown in a time of the same
/// order as a tenth of it.
#[test]
fn a_way_past_the_cap_is_crossed_at_it() {
    let seen = flown(view(0.0, 16.0), view(8_000_000.0, 16.0));
    assert!(seen.iter().all(|v| v.w <= F.cap));
    assert!(seen.iter().any(|v| v.w == F.cap), "it reaches the cap");
    let short = flown(view(0.0, 16.0), view(800_000.0, 16.0)).len();
    assert!(seen.len() < 10 * short, "{} then {}", short, seen.len());
    assert!(seen.len() < 60 * 60, "under a minute: {}", seen.len());
}
