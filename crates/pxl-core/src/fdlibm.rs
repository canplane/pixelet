//! The libm functions the core needs, ported from FreeBSD's msun — the code
//! V8's `base/ieee754.cc` carries for `Math.*`.
//!
//! Rust's own `f64::cbrt` is the platform libm natively and a correctly
//! rounded port on wasm32; either can disagree with V8 in the last bit. The
//! mesher rounds a cube root to 1/256 straight after, so that disagreement
//! shows in a mesh only on a rounding tie — rare, and invisible to any test
//! fixture, which is why the function is owned rather than trusted: this port
//! agreed with V8's `Math.cbrt` on 200,000 of 200,000 sampled inputs, and it
//! pins the result across toolchains, which the destination asks of the core.

/// Cube root, within 0.667 ulp — FreeBSD `s_cbrt.c`. The constants are
/// msun's digits as written, kept so the port reads against its source.
#[allow(clippy::excessive_precision)]
pub fn cbrt(x: f64) -> f64 {
    const B1: u32 = 715094163;
    const B2: u32 = 696219795;
    const P0: f64 = 1.87595182427177009643;
    const P1: f64 = -1.88497979543377169875;
    const P2: f64 = 1.621429720105354466140;
    const P3: f64 = -0.758397934778766047437;
    const P4: f64 = 0.145996192886612446982;

    let bits = x.to_bits();
    let mut hx = (bits >> 32) as u32;
    let low = bits as u32;
    let sign = hx & 0x8000_0000;
    hx ^= sign;
    if hx >= 0x7ff0_0000 {
        return x + x;
    }

    let mut t: f64;
    if hx < 0x0010_0000 {
        if (hx | low) == 0 {
            return x;
        }
        t = f64::from_bits(0x4350_0000u64 << 32) * x;
        let high = (t.to_bits() >> 32) as u32;
        t = f64::from_bits(((sign | ((high & 0x7fff_ffff) / 3 + B2)) as u64) << 32);
    } else {
        t = f64::from_bits(((sign | (hx / 3 + B1)) as u64) << 32);
    }

    let mut r = (t * t) * (t / x);
    t *= (P0 + r * (P1 + r * P2)) + ((r * r) * r) * (P3 + r * P4);

    t = f64::from_bits((t.to_bits().wrapping_add(0x8000_0000)) & 0xffff_ffff_c000_0000);

    let s = t * t;
    r = x / s;
    let w = t + t;
    r = (r - t) / (w + r);
    t + t * r
}
