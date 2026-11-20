// pxl-core/domain/smooth.rs

//! The one exponential smoothing that everything easing uses. It nears its
//! target and never arrives: whoever eases ends it, by what the eye can
//! tell apart. And the way a view flies over the ground to another
//! (`fly`).

use xpute_kit::math::scalar::wrap;

/// The share of the remaining distance covered in `delta` seconds at rate
/// `k`, `1 - e^(-k·delta)`. It traces the same curve at any frame rate.
pub(crate) fn smoothing_factor(k: f64, delta: f64) -> f64 {
    1.0 - libm::exp(-k * delta)
}

/// Eases an angle toward `target` the short way round. The result lies in
/// [-π, π).
pub(crate) fn smooth_angle(cur: f64, target: f64, k: f64, delta: f64) -> f64 {
    let pi = core::f64::consts::PI;
    let left = wrap(target - cur, -pi, pi);
    let [moved] = smooth_vec([0.0], [left], k, delta);
    wrap(cur + moved, -pi, pi)
}

/// Eases every axis by the same share, so the path is a straight line.
pub(crate) fn smooth_vec<const N: usize>(
    cur: [f64; N],
    target: [f64; N],
    k: f64,
    delta: f64,
) -> [f64; N] {
    let a = smoothing_factor(k, delta);
    std::array::from_fn(|i| cur[i] + (target[i] - cur[i]) * a)
}

/// A view over the ground: the point it is about, in units, and how wide
/// it is there.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct View {
    pub at: [f64; 2],
    pub w: f64,
}

/// How a view flies: `curve` is how far a zoom counts against a pan, van
/// Wijk and Nuij's rho; `cruise` the most of the way it covers a second, in
/// the way's own length, where zooming e-fold is `1 / curve`; `rate` the
/// rate it settles at where its end is near; and `cap` the widest a view
/// gets on the way.
pub(crate) struct Flight {
    pub curve: f64,
    pub cruise: f64,
    pub rate: f64,
    pub cap: f64,
}

/// The length of the shortest way from the view `from` to the view `to`, and
/// the view `s` along it, for `s` no longer than that length. The way is van
/// Wijk and Nuij's ("Smooth and efficient zooming and panning", 2003): in
/// the space of a view's place and width, where a pan counts by how much of
/// the view it crosses and a zoom by its ratio, the shortest way between two
/// views is an arc that widens the view, crosses and narrows it again, flat
/// where the two are near and high where they are far. Where that arc would
/// widen the view past `cap`, a view as wide as `cap` crosses at that width
/// instead, a pan, until the arc from where it is stays under it.
fn way(from: View, to: View, curve: f64, cap: f64) -> (f64, impl Fn(f64) -> View) {
    let d = [to.at[0] - from.at[0], to.at[1] - from.at[1]];
    let u1 = libm::hypot(d[0], d[1]);
    let (w0, w1, c2) = (from.w, to.w, curve * curve);
    // Two views about one point differ by a zoom alone.
    let about = u1 <= w0 * 1e-12;
    let b =
        |w: f64, sign: f64| (w1 * w1 - w0 * w0 + sign * c2 * c2 * u1 * u1) / (2.0 * w * c2 * u1);
    let (r0, r1) = match about {
        true => (0.0, libm::log(w1 / w0)),
        false => (-libm::asinh(b(w0, 1.0)), -libm::asinh(b(w1, -1.0))),
    };
    let len = match about {
        true => r1.abs() / curve,
        false => (r1 - r0) / curve,
    };
    // The arc is widest where it turns, between its ends where it first
    // widens.
    let capped = !about && w0 >= cap && r0 < 0.0 && r1 > 0.0;
    let at = move |s: f64| match (about, capped) {
        (true, _) => View {
            at: from.at,
            w: w0 * libm::exp(r1.signum() * curve * s),
        },
        (false, true) => {
            let u = (s * cap / curve).min(u1);
            View {
                at: [from.at[0] + d[0] / u1 * u, from.at[1] + d[1] / u1 * u],
                w: cap,
            }
        }
        (false, false) => {
            let r = curve * s + r0;
            let u = w0 / c2 * (libm::cosh(r0) * libm::tanh(r) - libm::sinh(r0));
            View {
                at: [from.at[0] + d[0] / u1 * u, from.at[1] + d[1] / u1 * u],
                w: (w0 * libm::cosh(r0) / libm::cosh(r)).min(cap),
            }
        }
    };
    (len, at)
}

/// The view `delta` seconds on from `from` on its way to `to` (`way`), found
/// again from where it is each time, so a `to` that changes is followed with
/// no seam. `speed` is how fast it goes along the way and is carried from
/// step to step: it settles on its end as a critically damped spring does,
/// with no overshoot, and is held to `cruise`, so a far end is flown to at
/// one speed and a near one eased into. Like the smoothing it nears its end
/// and does not arrive: whoever flies ends it.
pub(crate) fn fly(speed: &mut f64, from: View, to: View, f: &Flight, delta: f64) -> View {
    let (len, at) = way(from, to, f.curve, f.cap);
    let want = (f.rate / 2.0 * len).min(f.cruise);
    [*speed] = smooth_vec([*speed], [want], 2.0 * f.rate, delta);
    let s = *speed * delta;
    match s < len {
        true => at(s),
        false => to,
    }
}

#[cfg(test)]
#[path = "smooth.test.rs"]
mod test;
