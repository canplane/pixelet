// xpute-core/kit/fdlibm.rs

//! The libm functions the core needs, ported from FreeBSD's msun, so a result
//! is the same bits on every toolchain and target rather than whatever the
//! platform's libm returns.
//!
//! The core is where these numbers are decided, and JavaScript's `Math.*` is
//! the one that follows: V8 now calls LLVM libc's correctly rounded functions,
//! from which msun's differ by at most one ulp on a few inputs
//! (fdlibm.test.rs counts them). What cannot differ at all — a lattice
//! address, a mesh vertex after rounding — never rests on that last bit.
//!
//! `cbrt` is the exception that agrees exactly: this port matched V8's
//! `Math.cbrt` on 200,000 of 200,000 sampled inputs, and the mesher rounds a
//! cube root to 1/256 straight after, where a disagreement would show on a
//! rounding tie.

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

/// `Math.hypot(...values)`, as V8 computes it (builtins-math.tq): scaled by
/// the largest magnitude and summed with Kahan compensation, so the result
/// is the same bits whatever the order of magnitudes. Rust's `f64::hypot`
/// is the platform libm's two-argument one, which differs in the last bit.
pub fn hypot(values: &[f64]) -> f64 {
    let mut max = 0.0f64;
    let mut one_nan = false;
    for &v in values {
        let v = v.abs();
        if v.is_infinite() {
            return f64::INFINITY;
        }
        if v.is_nan() {
            one_nan = true;
        } else if v > max {
            max = v;
        }
    }
    if one_nan {
        return f64::NAN;
    }
    if max == 0.0 {
        return 0.0;
    }
    let mut sum = 0.0f64;
    let mut compensation = 0.0f64;
    for &v in values {
        let n = v.abs() / max;
        let summand = n * n - compensation;
        let preliminary = sum + summand;
        compensation = (preliminary - sum) - summand;
        sum = preliminary;
    }
    sum.sqrt() * max
}

/// `Math.sin`, `Math.cos`, `Math.tan`, `Math.atan`, `Math.atan2`,
/// `Math.exp`, `Math.log`: msun's, below.
pub fn sin(x: f64) -> f64 {
    msun::sin::sin(x)
}
pub fn cos(x: f64) -> f64 {
    msun::cos::cos(x)
}
pub fn tan(x: f64) -> f64 {
    msun::tan::tan(x)
}
pub fn atan(x: f64) -> f64 {
    msun::atan::atan(x)
}
pub fn atan2(y: f64, x: f64) -> f64 {
    msun::atan2::atan2(y, x)
}
pub fn exp(x: f64) -> f64 {
    msun::exp::exp(x)
}
pub fn log(x: f64) -> f64 {
    msun::log::log(x)
}

/// `Math.round`: the nearest integer, a half rounding up (toward +∞), and
/// -0 for -0.5 ≤ x < 0. Rust's `round` sends a half away from zero, and
/// `floor(x + 0.5)` is wrong at 0.49999999999999994, where the sum rounds
/// to 1.
pub fn round(x: f64) -> f64 {
    if !x.is_finite() || x == 0.0 {
        return x;
    }
    if x > 0.0 && x < 0.5 {
        return 0.0;
    }
    if (-0.5..0.0).contains(&x) {
        return -0.0;
    }
    let c = x.ceil();
    if c - 0.5 > x {
        c - 1.0
    } else {
        c
    }
}

#[allow(clippy::all)]
mod msun {

    // ---- msun's transcendental functions ----
    //
    // sin, cos, tan, atan, atan2, exp and log: FreeBSD msun's, as the
    // rust-lang/libm crate carries them (MIT; the Sun notice each file keeps is
    // kept below), vendored so the kernel's doubles are its own on every
    // toolchain and target rather than the platform's. They are not V8's: V8's
    // Math calls LLVM libc's correctly rounded functions now, and these differ
    // from those by at most one ulp on a small share of inputs. The kernel is
    // the source of truth for what a coordinate computes to; the shell's
    // engine, whichever it is, is not (issue/20260914-what-stays-in-javascript.md).

    #[allow(unused_macros)]
    macro_rules! force_eval {
        ($e:expr) => {
            unsafe { ::core::ptr::read_volatile(&$e) }
        };
    }

    #[allow(unused_macros)]
    macro_rules! div {
        ($a:expr, $b:expr) => {
            $a / $b
        };
    }

    #[allow(unused_macros)]
    macro_rules! i {
        ($array:expr, $index:expr) => {
            $array[$index]
        };
        ($array:expr, $index:expr, = , $rhs:expr) => {
            $array[$index] = $rhs;
        };
        ($array:expr, $index:expr, += , $rhs:expr) => {
            $array[$index] += $rhs;
        };
        ($array:expr, $index:expr, -= , $rhs:expr) => {
            $array[$index] -= $rhs;
        };
        ($array:expr, $index:expr, &= , $rhs:expr) => {
            $array[$index] &= $rhs;
        };
        ($array:expr, $index:expr, == , $rhs:expr) => {
            $array[$index] == $rhs
        };
    }

    fn floor(x: f64) -> f64 {
        x.floor()
    }

    /// x * 2^n, exact where the result is representable (musl's scalbn).
    fn scalbn(x: f64, mut n: i32) -> f64 {
        let x1p1023 = f64::from_bits(0x7fe0000000000000);
        let x1p53 = f64::from_bits(0x4340000000000000);
        let x1p_1022 = f64::from_bits(0x0010000000000000);
        let mut y = x;
        if n > 1023 {
            y *= x1p1023;
            n -= 1023;
            if n > 1023 {
                y *= x1p1023;
                n -= 1023;
                if n > 1023 {
                    n = 1023;
                }
            }
        } else if n < -1022 {
            y *= x1p_1022 * x1p53;
            n += 1022 - 53;
            if n < -1022 {
                y *= x1p_1022 * x1p53;
                n += 1022 - 53;
                if n < -1022 {
                    n = -1022;
                }
            }
        }
        y * f64::from_bits(((0x3ff + n) as u64) << 52)
    }

    #[allow(clippy::all, unused_imports, unused_unsafe, non_upper_case_globals)]
    pub mod k_sin {
        #[allow(unused_imports)]
        use super::*;
        // origin: FreeBSD /usr/src/lib/msun/src/k_sin.c
        //
        // ====================================================
        // Copyright (C) 1993 by Sun Microsystems, Inc. All rights reserved.
        //
        // Developed at SunSoft, a Sun Microsystems, Inc. business.
        // Permission to use, copy, modify, and distribute this
        // software is freely granted, provided that this notice
        // is preserved.
        // ====================================================

        const S1: f64 = -1.66666666666666324348e-01; /* 0xBFC55555, 0x55555549 */
        const S2: f64 = 8.33333333332248946124e-03; /* 0x3F811111, 0x1110F8A6 */
        const S3: f64 = -1.98412698298579493134e-04; /* 0xBF2A01A0, 0x19C161D5 */
        const S4: f64 = 2.75573137070700676789e-06; /* 0x3EC71DE3, 0x57B1FE7D */
        const S5: f64 = -2.50507602534068634195e-08; /* 0xBE5AE5E6, 0x8A2B9CEB */
        const S6: f64 = 1.58969099521155010221e-10; /* 0x3DE5D93A, 0x5ACFD57C */

        // kernel sin function on ~[-pi/4, pi/4] (except on -0), pi/4 ~ 0.7854
        // Input x is assumed to be bounded by ~pi/4 in magnitude.
        // Input y is the tail of x.
        // Input iy indicates whether y is 0. (if iy=0, y assume to be 0).
        //
        // Algorithm
        //      1. Since sin(-x) = -sin(x), we need only to consider positive x.
        //      2. Callers must return sin(-0) = -0 without calling here since our
        //         odd polynomial is not evaluated in a way that preserves -0.
        //         Callers may do the optimization sin(x) ~ x for tiny x.
        //      3. sin(x) is approximated by a polynomial of degree 13 on
        //         [0,pi/4]
        //                               3            13
        //              sin(x) ~ x + S1*x + ... + S6*x
        //         where
        //
        //      |sin(x)         2     4     6     8     10     12  |     -58
        //      |----- - (1+S1*x +S2*x +S3*x +S4*x +S5*x  +S6*x   )| <= 2
        //      |  x                                               |
        //
        //      4. sin(x+y) = sin(x) + sin'(x')*y
        //                  ~ sin(x) + (1-x*x/2)*y
        //         For better accuracy, let
        //                   3      2      2      2      2
        //              r = x *(S2+x *(S3+x *(S4+x *(S5+x *S6))))
        //         then                   3    2
        //              sin(x) = x + (S1*x + (x *(r-y/2)+y))
        pub fn k_sin(x: f64, y: f64, iy: i32) -> f64 {
            let z = x * x;
            let w = z * z;
            let r = S2 + z * (S3 + z * S4) + z * w * (S5 + z * S6);
            let v = z * x;
            if iy == 0 {
                x + v * (S1 + z * r)
            } else {
                x - ((z * (0.5 * y - v * r) - y) - v * S1)
            }
        }
    }

    #[allow(clippy::all, unused_imports, unused_unsafe, non_upper_case_globals)]
    pub mod k_cos {
        #[allow(unused_imports)]
        use super::*;
        // origin: FreeBSD /usr/src/lib/msun/src/k_cos.c
        //
        // ====================================================
        // Copyright (C) 1993 by Sun Microsystems, Inc. All rights reserved.
        //
        // Developed at SunSoft, a Sun Microsystems, Inc. business.
        // Permission to use, copy, modify, and distribute this
        // software is freely granted, provided that this notice
        // is preserved.
        // ====================================================

        const C1: f64 = 4.16666666666666019037e-02; /* 0x3FA55555, 0x5555554C */
        const C2: f64 = -1.38888888888741095749e-03; /* 0xBF56C16C, 0x16C15177 */
        const C3: f64 = 2.48015872894767294178e-05; /* 0x3EFA01A0, 0x19CB1590 */
        const C4: f64 = -2.75573143513906633035e-07; /* 0xBE927E4F, 0x809C52AD */
        const C5: f64 = 2.08757232129817482790e-09; /* 0x3E21EE9E, 0xBDB4B1C4 */
        const C6: f64 = -1.13596475577881948265e-11; /* 0xBDA8FAE9, 0xBE8838D4 */

        // kernel cos function on [-pi/4, pi/4], pi/4 ~ 0.785398164
        // Input x is assumed to be bounded by ~pi/4 in magnitude.
        // Input y is the tail of x.
        //
        // Algorithm
        //      1. Since cos(-x) = cos(x), we need only to consider positive x.
        //      2. if x < 2^-27 (hx<0x3e400000 0), return 1 with inexact if x!=0.
        //      3. cos(x) is approximated by a polynomial of degree 14 on
        //         [0,pi/4]
        //                                       4            14
        //              cos(x) ~ 1 - x*x/2 + C1*x + ... + C6*x
        //         where the remez error is
        //
        //      |              2     4     6     8     10    12     14 |     -58
        //      |cos(x)-(1-.5*x +C1*x +C2*x +C3*x +C4*x +C5*x  +C6*x  )| <= 2
        //      |                                                      |
        //
        //                     4     6     8     10    12     14
        //      4. let r = C1*x +C2*x +C3*x +C4*x +C5*x  +C6*x  , then
        //             cos(x) ~ 1 - x*x/2 + r
        //         since cos(x+y) ~ cos(x) - sin(x)*y
        //                        ~ cos(x) - x*y,
        //         a correction term is necessary in cos(x) and hence
        //              cos(x+y) = 1 - (x*x/2 - (r - x*y))
        //         For better accuracy, rearrange to
        //              cos(x+y) ~ w + (tmp + (r-x*y))
        //         where w = 1 - x*x/2 and tmp is a tiny correction term
        //         (1 - x*x/2 == w + tmp exactly in infinite precision).
        //         The exactness of w + tmp in infinite precision depends on w
        //         and tmp having the same precision as x.  If they have extra
        //         precision due to compiler bugs, then the extra precision is
        //         only good provided it is retained in all terms of the final
        //         expression for cos().  Retention happens in all cases tested
        //         under FreeBSD, so don't pessimize things by forcibly clipping
        //         any extra precision in w.
        pub fn k_cos(x: f64, y: f64) -> f64 {
            let z = x * x;
            let w = z * z;
            let r = z * (C1 + z * (C2 + z * C3)) + w * w * (C4 + z * (C5 + z * C6));
            let hz = 0.5 * z;
            let w = 1.0 - hz;
            w + (((1.0 - w) - hz) + (z * r - x * y))
        }
    }

    #[allow(clippy::all, unused_imports, unused_unsafe, non_upper_case_globals)]
    pub mod k_tan {
        #[allow(unused_imports)]
        use super::*;
        // origin: FreeBSD /usr/src/lib/msun/src/k_tan.c */
        //
        // ====================================================
        // Copyright 2004 Sun Microsystems, Inc.  All Rights Reserved.
        //
        // Permission to use, copy, modify, and distribute this
        // software is freely granted, provided that this notice
        // is preserved.
        // ====================================================

        // kernel tan function on ~[-pi/4, pi/4] (except on -0), pi/4 ~ 0.7854
        // Input x is assumed to be bounded by ~pi/4 in magnitude.
        // Input y is the tail of x.
        // Input odd indicates whether tan (if odd = 0) or -1/tan (if odd = 1) is returned.
        //
        // Algorithm
        //      1. Since tan(-x) = -tan(x), we need only to consider positive x.
        //      2. Callers must return tan(-0) = -0 without calling here since our
        //         odd polynomial is not evaluated in a way that preserves -0.
        //         Callers may do the optimization tan(x) ~ x for tiny x.
        //      3. tan(x) is approximated by a odd polynomial of degree 27 on
        //         [0,0.67434]
        //                               3             27
        //              tan(x) ~ x + T1*x + ... + T13*x
        //         where
        //
        //              |tan(x)         2     4            26   |     -59.2
        //              |----- - (1+T1*x +T2*x +.... +T13*x    )| <= 2
        //              |  x                                    |
        //
        //         Note: tan(x+y) = tan(x) + tan'(x)*y
        //                        ~ tan(x) + (1+x*x)*y
        //         Therefore, for better accuracy in computing tan(x+y), let
        //                   3      2      2       2       2
        //              r = x *(T2+x *(T3+x *(...+x *(T12+x *T13))))
        //         then
        //                                  3    2
        //              tan(x+y) = x + (T1*x + (x *(r+y)+y))
        //
        //      4. For x in [0.67434,pi/4],  let y = pi/4 - x, then
        //              tan(x) = tan(pi/4-y) = (1-tan(y))/(1+tan(y))
        //                     = 1 - 2*(tan(y) - (tan(y)^2)/(1+tan(y)))
        static T: [f64; 13] = [
            3.33333333333334091986e-01,  /* 3FD55555, 55555563 */
            1.33333333333201242699e-01,  /* 3FC11111, 1110FE7A */
            5.39682539762260521377e-02,  /* 3FABA1BA, 1BB341FE */
            2.18694882948595424599e-02,  /* 3F9664F4, 8406D637 */
            8.86323982359930005737e-03,  /* 3F8226E3, E96E8493 */
            3.59207910759131235356e-03,  /* 3F6D6D22, C9560328 */
            1.45620945432529025516e-03,  /* 3F57DBC8, FEE08315 */
            5.88041240820264096874e-04,  /* 3F4344D8, F2F26501 */
            2.46463134818469906812e-04,  /* 3F3026F7, 1A8D1068 */
            7.81794442939557092300e-05,  /* 3F147E88, A03792A6 */
            7.14072491382608190305e-05,  /* 3F12B80F, 32F0A7E9 */
            -1.85586374855275456654e-05, /* BEF375CB, DB605373 */
            2.59073051863633712884e-05,  /* 3EFB2A70, 74BF7AD4 */
        ];
        const PIO4: f64 = 7.85398163397448278999e-01; /* 3FE921FB, 54442D18 */
        const PIO4_LO: f64 = 3.06161699786838301793e-17; /* 3C81A626, 33145C07 */

        pub fn k_tan(mut x: f64, mut y: f64, odd: i32) -> f64 {
            let hx = (f64::to_bits(x) >> 32) as u32;
            let big = (hx & 0x7fffffff) >= 0x3FE59428; /* |x| >= 0.6744 */
            if big {
                let sign = hx >> 31;
                if sign != 0 {
                    x = -x;
                    y = -y;
                }
                x = (PIO4 - x) + (PIO4_LO - y);
                y = 0.0;
            }
            let z = x * x;
            let w = z * z;
            /*
             * Break x^5*(T[1]+x^2*T[2]+...) into
             * x^5(T[1]+x^4*T[3]+...+x^20*T[11]) +
             * x^5(x^2*(T[2]+x^4*T[4]+...+x^22*[T12]))
             */
            let r = T[1] + w * (T[3] + w * (T[5] + w * (T[7] + w * (T[9] + w * T[11]))));
            let v = z * (T[2] + w * (T[4] + w * (T[6] + w * (T[8] + w * (T[10] + w * T[12])))));
            let s = z * x;
            let r = y + z * (s * (r + v) + y) + s * T[0];
            let w = x + r;
            if big {
                let sign = hx >> 31;
                let s = 1.0 - 2.0 * odd as f64;
                let v = s - 2.0 * (x + (r - w * w / (w + s)));
                return if sign != 0 { -v } else { v };
            }
            if odd == 0 {
                return w;
            }
            /* -1.0/(x+r) has up to 2ulp error, so compute it accurately */
            let w0 = zero_low_word(w);
            let v = r - (w0 - x); /* w0+v = r+x */
            let a = -1.0 / w;
            let a0 = zero_low_word(a);
            a0 + a * (1.0 + a0 * w0 + a0 * v)
        }

        fn zero_low_word(x: f64) -> f64 {
            f64::from_bits(f64::to_bits(x) & 0xFFFF_FFFF_0000_0000)
        }
    }

    #[allow(clippy::all, unused_imports, unused_unsafe, non_upper_case_globals)]
    pub mod rem_pio2_large {
        #[allow(unused_imports)]
        use super::*;
        use super::{floor, scalbn};
        /* origin: FreeBSD /usr/src/lib/msun/src/k_rem_pio2.c */
        /*
         * ====================================================
         * Copyright (C) 1993 by Sun Microsystems, Inc. All rights reserved.
         *
         * Developed at SunSoft, a Sun Microsystems, Inc. business.
         * Permission to use, copy, modify, and distribute this
         * software is freely granted, provided that this notice
         * is preserved.
         * ====================================================
         */

        // initial value for jk
        const INIT_JK: [usize; 4] = [3, 4, 4, 6];

        // Table of constants for 2/pi, 396 Hex digits (476 decimal) of 2/pi
        //
        //              integer array, contains the (24*i)-th to (24*i+23)-th
        //              bit of 2/pi after binary point. The corresponding
        //              floating value is
        //
        //                      ipio2[i] * 2^(-24(i+1)).
        //
        // NB: This table must have at least (e0-3)/24 + jk terms.
        //     For quad precision (e0 <= 16360, jk = 6), this is 686.

        const IPIO2: [i32; 690] = [
            0xA2F983, 0x6E4E44, 0x1529FC, 0x2757D1, 0xF534DD, 0xC0DB62, 0x95993C, 0x439041, 0xFE5163, 0xABDEBB, 0xC561B7, 0x246E3A, 0x424DD2, 0xE00649, 0x2EEA09, 0xD1921C, 0xFE1DEB, 0x1CB129,
            0xA73EE8, 0x8235F5, 0x2EBB44, 0x84E99C, 0x7026B4, 0x5F7E41, 0x3991D6, 0x398353, 0x39F49C, 0x845F8B, 0xBDF928, 0x3B1FF8, 0x97FFDE, 0x05980F, 0xEF2F11, 0x8B5A0A, 0x6D1F6D, 0x367ECF,
            0x27CB09, 0xB74F46, 0x3F669E, 0x5FEA2D, 0x7527BA, 0xC7EBE5, 0xF17B3D, 0x0739F7, 0x8A5292, 0xEA6BFB, 0x5FB11F, 0x8D5D08, 0x560330, 0x46FC7B, 0x6BABF0, 0xCFBC20, 0x9AF436, 0x1DA9E3,
            0x91615E, 0xE61B08, 0x659985, 0x5F14A0, 0x68408D, 0xFFD880, 0x4D7327, 0x310606, 0x1556CA, 0x73A8C9, 0x60E27B, 0xC08C6B, 0x47C419, 0xC367CD, 0xDCE809, 0x2A8359, 0xC4768B, 0x961CA6,
            0xDDAF44, 0xD15719, 0x053EA5, 0xFF0705, 0x3F7E33, 0xE832C2, 0xDE4F98, 0x327DBB, 0xC33D26, 0xEF6B1E, 0x5EF89F, 0x3A1F35, 0xCAF27F, 0x1D87F1, 0x21907C, 0x7C246A, 0xFA6ED5, 0x772D30,
            0x433B15, 0xC614B5, 0x9D19C3, 0xC2C4AD, 0x414D2C, 0x5D000C, 0x467D86, 0x2D71E3, 0x9AC69B, 0x006233, 0x7CD2B4, 0x97A7B4, 0xD55537, 0xF63ED7, 0x1810A3, 0xFC764D, 0x2A9D64, 0xABD770,
            0xF87C63, 0x57B07A, 0xE71517, 0x5649C0, 0xD9D63B, 0x3884A7, 0xCB2324, 0x778AD6, 0x23545A, 0xB91F00, 0x1B0AF1, 0xDFCE19, 0xFF319F, 0x6A1E66, 0x615799, 0x47FBAC, 0xD87F7E, 0xB76522,
            0x89E832, 0x60BFE6, 0xCDC4EF, 0x09366C, 0xD43F5D, 0xD7DE16, 0xDE3B58, 0x929BDE, 0x2822D2, 0xE88628, 0x4D58E2, 0x32CAC6, 0x16E308, 0xCB7DE0, 0x50C017, 0xA71DF3, 0x5BE018, 0x34132E,
            0x621283, 0x014883, 0x5B8EF5, 0x7FB0AD, 0xF2E91E, 0x434A48, 0xD36710, 0xD8DDAA, 0x425FAE, 0xCE616A, 0xA4280A, 0xB499D3, 0xF2A606, 0x7F775C, 0x83C2A3, 0x883C61, 0x78738A, 0x5A8CAF,
            0xBDD76F, 0x63A62D, 0xCBBFF4, 0xEF818D, 0x67C126, 0x45CA55, 0x36D9CA, 0xD2A828, 0x8D61C2, 0x77C912, 0x142604, 0x9B4612, 0xC459C4, 0x44C5C8, 0x91B24D, 0xF31700, 0xAD43D4, 0xE54929,
            0x10D5FD, 0xFCBE00, 0xCC941E, 0xEECE70, 0xF53E13, 0x80F1EC, 0xC3E7B3, 0x28F8C7, 0x940593, 0x3E71C1, 0xB3092E, 0xF3450B, 0x9C1288, 0x7B20AB, 0x9FB52E, 0xC29247, 0x2F327B, 0x6D550C,
            0x90A772, 0x1FE76B, 0x96CB31, 0x4A1679, 0xE27941, 0x89DFF4, 0x9794E8, 0x84E6E2, 0x973199, 0x6BED88, 0x365F5F, 0x0EFDBB, 0xB49A48, 0x6CA467, 0x427271, 0x325D8D, 0xB8159F, 0x09E5BC,
            0x25318D, 0x3974F7, 0x1C0530, 0x010C0D, 0x68084B, 0x58EE2C, 0x90AA47, 0x02E774, 0x24D6BD, 0xA67DF7, 0x72486E, 0xEF169F, 0xA6948E, 0xF691B4, 0x5153D1, 0xF20ACF, 0x339820, 0x7E4BF5,
            0x6863B2, 0x5F3EDD, 0x035D40, 0x7F8985, 0x295255, 0xC06437, 0x10D86D, 0x324832, 0x754C5B, 0xD4714E, 0x6E5445, 0xC1090B, 0x69F52A, 0xD56614, 0x9D0727, 0x50045D, 0xDB3BB4, 0xC576EA,
            0x17F987, 0x7D6B49, 0xBA271D, 0x296996, 0xACCCC6, 0x5414AD, 0x6AE290, 0x89D988, 0x50722C, 0xBEA404, 0x940777, 0x7030F3, 0x27FC00, 0xA871EA, 0x49C266, 0x3DE064, 0x83DD97, 0x973FA3,
            0xFD9443, 0x8C860D, 0xDE4131, 0x9D3992, 0x8C70DD, 0xE7B717, 0x3BDF08, 0x2B3715, 0xA0805C, 0x93805A, 0x921110, 0xD8E80F, 0xAF806C, 0x4BFFDB, 0x0F9038, 0x761859, 0x15A562, 0xBBCB61,
            0xB989C7, 0xBD4010, 0x04F2D2, 0x277549, 0xF6B6EB, 0xBB22DB, 0xAA140A, 0x2F2689, 0x768364, 0x333B09, 0x1A940E, 0xAA3A51, 0xC2A31D, 0xAEEDAF, 0x12265C, 0x4DC26D, 0x9C7A2D, 0x9756C0,
            0x833F03, 0xF6F009, 0x8C402B, 0x99316D, 0x07B439, 0x15200C, 0x5BC3D8, 0xC492F5, 0x4BADC6, 0xA5CA4E, 0xCD37A7, 0x36A9E6, 0x9492AB, 0x6842DD, 0xDE6319, 0xEF8C76, 0x528B68, 0x37DBFC,
            0xABA1AE, 0x3115DF, 0xA1AE00, 0xDAFB0C, 0x664D64, 0xB705ED, 0x306529, 0xBF5657, 0x3AFF47, 0xB9F96A, 0xF3BE75, 0xDF9328, 0x3080AB, 0xF68C66, 0x15CB04, 0x0622FA, 0x1DE4D9, 0xA4B33D,
            0x8F1B57, 0x09CD36, 0xE9424E, 0xA4BE13, 0xB52333, 0x1AAAF0, 0xA8654F, 0xA5C1D2, 0x0F3F0B, 0xCD785B, 0x76F923, 0x048B7B, 0x721789, 0x53A6C6, 0xE26E6F, 0x00EBEF, 0x584A9B, 0xB7DAC4,
            0xBA66AA, 0xCFCF76, 0x1D02D1, 0x2DF1B1, 0xC1998C, 0x77ADC3, 0xDA4886, 0xA05DF7, 0xF480C6, 0x2FF0AC, 0x9AECDD, 0xBC5C3F, 0x6DDED0, 0x1FC790, 0xB6DB2A, 0x3A25A3, 0x9AAF00, 0x9353AD,
            0x0457B6, 0xB42D29, 0x7E804B, 0xA707DA, 0x0EAA76, 0xA1597B, 0x2A1216, 0x2DB7DC, 0xFDE5FA, 0xFEDB89, 0xFDBE89, 0x6C76E4, 0xFCA906, 0x70803E, 0x156E85, 0xFF87FD, 0x073E28, 0x336761,
            0x86182A, 0xEABD4D, 0xAFE7B3, 0x6E6D8F, 0x396795, 0x5BBF31, 0x48D784, 0x16DF30, 0x432DC7, 0x356125, 0xCE70C9, 0xB8CB30, 0xFD6CBF, 0xA200A4, 0xE46C05, 0xA0DD5A, 0x476F21, 0xD21262,
            0x845CB9, 0x496170, 0xE0566B, 0x015299, 0x375550, 0xB7D51E, 0xC4F133, 0x5F6E13, 0xE4305D, 0xA92E85, 0xC3B21D, 0x3632A1, 0xA4B708, 0xD4B1EA, 0x21F716, 0xE4698F, 0x77FF27, 0x80030C,
            0x2D408D, 0xA0CD4F, 0x99A520, 0xD3A2B3, 0x0A5D2F, 0x42F9B4, 0xCBDA11, 0xD0BE7D, 0xC1DB9B, 0xBD17AB, 0x81A2CA, 0x5C6A08, 0x17552E, 0x550027, 0xF0147F, 0x8607E1, 0x640B14, 0x8D4196,
            0xDEBE87, 0x2AFDDA, 0xB6256B, 0x34897B, 0xFEF305, 0x9EBFB9, 0x4F6A68, 0xA82A4A, 0x5AC44F, 0xBCF82D, 0x985AD7, 0x95C7F4, 0x8D4D0D, 0xA63A20, 0x5F57A4, 0xB13F14, 0x953880, 0x0120CC,
            0x86DD71, 0xB6DEC9, 0xF560BF, 0x11654D, 0x6B0701, 0xACB08C, 0xD0C0B2, 0x485551, 0x0EFB1E, 0xC37295, 0x3B06A3, 0x3540C0, 0x7BDC06, 0xCC45E0, 0xFA294E, 0xC8CAD6, 0x41F3E8, 0xDE647C,
            0xD8649B, 0x31BED9, 0xC397A4, 0xD45877, 0xC5E369, 0x13DAF0, 0x3C3ABA, 0x461846, 0x5F7555, 0xF5BDD2, 0xC6926E, 0x5D2EAC, 0xED440E, 0x423E1C, 0x87C461, 0xE9FD29, 0xF3D6E7, 0xCA7C22,
            0x35916F, 0xC5E008, 0x8DD7FF, 0xE26A6E, 0xC6FDB0, 0xC10893, 0x745D7C, 0xB2AD6B, 0x9D6ECD, 0x7B723E, 0x6A11C6, 0xA9CFF7, 0xDF7329, 0xBAC9B5, 0x5100B7, 0x0DB2E2, 0x24BA74, 0x607DE5,
            0x8AD874, 0x2C150D, 0x0C1881, 0x94667E, 0x162901, 0x767A9F, 0xBEFDFD, 0xEF4556, 0x367ED9, 0x13D9EC, 0xB9BA8B, 0xFC97C4, 0x27A831, 0xC36EF1, 0x36C594, 0x56A8D8, 0xB5A8B4, 0x0ECCCF,
            0x2D8912, 0x34576F, 0x89562C, 0xE3CE99, 0xB920D6, 0xAA5E6B, 0x9C2A3E, 0xCC5F11, 0x4A0BFD, 0xFBF4E1, 0x6D3B8E, 0x2C86E2, 0x84D4E9, 0xA9B4FC, 0xD1EEEF, 0xC9352E, 0x61392F, 0x442138,
            0xC8D91B, 0x0AFC81, 0x6A4AFB, 0xD81C2F, 0x84B453, 0x8C994E, 0xCC2254, 0xDC552A, 0xD6C6C0, 0x96190B, 0xB8701A, 0x649569, 0x605A26, 0xEE523F, 0x0F117F, 0x11B5F4, 0xF5CBFC, 0x2DBC34,
            0xEEBC34, 0xCC5DE8, 0x605EDD, 0x9B8E67, 0xEF3392, 0xB817C9, 0x9B5861, 0xBC57E1, 0xC68351, 0x103ED8, 0x4871DD, 0xDD1C2D, 0xA118AF, 0x462C21, 0xD7F359, 0x987AD9, 0xC0549E, 0xFA864F,
            0xFC0656, 0xAE79E5, 0x362289, 0x22AD38, 0xDC9367, 0xAAE855, 0x382682, 0x9BE7CA, 0xA40D51, 0xB13399, 0x0ED7A9, 0x480569, 0xF0B265, 0xA7887F, 0x974C88, 0x36D1F9, 0xB39221, 0x4A827B,
            0x21CF98, 0xDC9F40, 0x5547DC, 0x3A74E1, 0x42EB67, 0xDF9DFE, 0x5FD45E, 0xA4677B, 0x7AACBA, 0xA2F655, 0x23882B, 0x55BA41, 0x086E59, 0x862A21, 0x834739, 0xE6E389, 0xD49EE5, 0x40FB49,
            0xE956FF, 0xCA0F1C, 0x8A59C5, 0x2BFA94, 0xC5C1D3, 0xCFC50F, 0xAE5ADB, 0x86C547, 0x624385, 0x3B8621, 0x94792C, 0x876110, 0x7B4C2A, 0x1A2C80, 0x12BF43, 0x902688, 0x893C78, 0xE4C4A8,
            0x7BDBE5, 0xC23AC4, 0xEAF426, 0x8A67F7, 0xBF920D, 0x2BA365, 0xB1933D, 0x0B7CBD, 0xDC51A4, 0x63DD27, 0xDDE169, 0x19949A, 0x9529A8, 0x28CE68, 0xB4ED09, 0x209F44, 0xCA984E, 0x638270,
            0x237C7E, 0x32B90F, 0x8EF5A7, 0xE75614, 0x08F121, 0x2A9DB5, 0x4D7E6F, 0x5119A5, 0xABF9B5, 0xD6DF82, 0x61DD96, 0x023616, 0x9F3AC4, 0xA1A283, 0x6DED72, 0x7A8D39, 0xA9B882, 0x5C326B,
            0x5B2746, 0xED3400, 0x7700D2, 0x55F4FC, 0x4D5901, 0x8071E0,
        ];

        const PIO2: [f64; 8] = [
            1.57079625129699707031e+00, /* 0x3FF921FB, 0x40000000 */
            7.54978941586159635335e-08, /* 0x3E74442D, 0x00000000 */
            5.39030252995776476554e-15, /* 0x3CF84698, 0x80000000 */
            3.28200341580791294123e-22, /* 0x3B78CC51, 0x60000000 */
            1.27065575308067607349e-29, /* 0x39F01B83, 0x80000000 */
            1.22933308981111328932e-36, /* 0x387A2520, 0x40000000 */
            2.73370053816464559624e-44, /* 0x36E38222, 0x80000000 */
            2.16741683877804819444e-51, /* 0x3569F31D, 0x00000000 */
        ];

        // fn rem_pio2_large(x : &[f64], y : &mut [f64], e0 : i32, prec : usize) -> i32
        //
        // Input parameters:
        //      x[]     The input value (must be positive) is broken into nx
        //              pieces of 24-bit integers in double precision format.
        //              x[i] will be the i-th 24 bit of x. The scaled exponent
        //              of x[0] is given in input parameter e0 (i.e., x[0]*2^e0
        //              match x's up to 24 bits.
        //
        //              Example of breaking a double positive z into x[0]+x[1]+x[2]:
        //                      e0 = ilogb(z)-23
        //                      z  = scalbn(z,-e0)
        //              for i = 0,1,2
        //                      x[i] = floor(z)
        //                      z    = (z-x[i])*2**24
        //
        //      y[]     output result in an array of double precision numbers.
        //              The dimension of y[] is:
        //                      24-bit  precision       1
        //                      53-bit  precision       2
        //                      64-bit  precision       2
        //                      113-bit precision       3
        //              The actual value is the sum of them. Thus for 113-bit
        //              precison, one may have to do something like:
        //
        //              long double t,w,r_head, r_tail;
        //              t = (long double)y[2] + (long double)y[1];
        //              w = (long double)y[0];
        //              r_head = t+w;
        //              r_tail = w - (r_head - t);
        //
        //      e0      The exponent of x[0]. Must be <= 16360 or you need to
        //              expand the ipio2 table.
        //
        //      prec    an integer indicating the precision:
        //                      0       24  bits (single)
        //                      1       53  bits (double)
        //                      2       64  bits (extended)
        //                      3       113 bits (quad)
        //
        // Here is the description of some local variables:
        //
        //      jk      jk+1 is the initial number of terms of ipio2[] needed
        //              in the computation. The minimum and recommended value
        //              for jk is 3,4,4,6 for single, double, extended, and quad.
        //              jk+1 must be 2 larger than you might expect so that our
        //              recomputation test works. (Up to 24 bits in the integer
        //              part (the 24 bits of it that we compute) and 23 bits in
        //              the fraction part may be lost to cancelation before we
        //              recompute.)
        //
        //      jz      local integer variable indicating the number of
        //              terms of ipio2[] used.
        //
        //      jx      nx - 1
        //
        //      jv      index for pointing to the suitable ipio2[] for the
        //              computation. In general, we want
        //                      ( 2^e0*x[0] * ipio2[jv-1]*2^(-24jv) )/8
        //              is an integer. Thus
        //                      e0-3-24*jv >= 0 or (e0-3)/24 >= jv
        //              Hence jv = max(0,(e0-3)/24).
        //
        //      jp      jp+1 is the number of terms in PIo2[] needed, jp = jk.
        //
        //      q[]     double array with integral value, representing the
        //              24-bits chunk of the product of x and 2/pi.
        //
        //      q0      the corresponding exponent of q[0]. Note that the
        //              exponent for q[i] would be q0-24*i.
        //
        //      PIo2[]  double precision array, obtained by cutting pi/2
        //              into 24 bits chunks.
        //
        //      f[]     ipio2[] in floating point
        //
        //      iq[]    integer array by breaking up q[] in 24-bits chunk.
        //
        //      fq[]    final product of x*(2/pi) in fq[0],..,fq[jk]
        //
        //      ih      integer. If >0 it indicates q[] is >= 0.5, hence
        //              it also indicates the *sign* of the result.

        /// Return the last three digits of N with y = x - N*pi/2
        /// so that |y| < pi/2.
        ///
        /// The method is to compute the integer (mod 8) and fraction parts of
        /// (2/pi)*x without doing the full multiplication. In general we
        /// skip the part of the product that are known to be a huge integer (
        /// more accurately, = 0 mod 8 ). Thus the number of operations are
        /// independent of the exponent of the input.
        pub fn rem_pio2_large(x: &[f64], y: &mut [f64], e0: i32, prec: usize) -> i32 {
            // FIXME(rust-lang/rust#144518): Inline assembly would cause `no_panic` to fail
            // on the callers of this function. As a workaround, avoid inlining `floor` here
            // when implemented with assembly.

            let x1p24 = f64::from_bits(0x4170000000000000); // 0x1p24 === 2 ^ 24
            let x1p_24 = f64::from_bits(0x3e70000000000000); // 0x1p_24 === 2 ^ (-24)

            if cfg!(target_pointer_width = "64") {
                debug_assert!(e0 <= 16360);
            }

            let nx = x.len();

            let mut fw: f64;
            let mut n: i32;
            let mut ih: i32;
            let mut z: f64;
            let mut f: [f64; 20] = [0.; 20];
            let mut fq: [f64; 20] = [0.; 20];
            let mut q: [f64; 20] = [0.; 20];
            let mut iq: [i32; 20] = [0; 20];

            /* initialize jk*/
            let jk = i!(INIT_JK, prec);
            let jp = jk;

            /* determine jx,jv,q0, note that 3>q0 */
            let jx = nx - 1;
            let mut jv = div!(e0 - 3, 24);
            if jv < 0 {
                jv = 0;
            }
            let mut q0 = e0 - 24 * (jv + 1);
            let jv = jv as usize;

            /* set up f[0] to f[jx+jk] where f[jx+jk] = ipio2[jv+jk] */
            let mut j = (jv as i32) - (jx as i32);
            let m = jx + jk;
            for i in 0..=m {
                i!(f, i, =, if j < 0 {
                    0.
                } else {
                    i!(IPIO2, j as usize) as f64
                });
                j += 1;
            }

            /* compute q[0],q[1],...q[jk] */
            for i in 0..=jk {
                fw = 0f64;
                for j in 0..=jx {
                    fw += i!(x, j) * i!(f, jx + i - j);
                }
                i!(q, i, =, fw);
            }

            let mut jz = jk;

            'recompute: loop {
                /* distill q[] into iq[] reversingly */
                let mut i = 0i32;
                z = i!(q, jz);
                for j in (1..=jz).rev() {
                    fw = (x1p_24 * z) as i32 as f64;
                    i!(iq, i as usize, =, (z - x1p24 * fw) as i32);
                    z = i!(q, j - 1) + fw;
                    i += 1;
                }

                /* compute n */
                z = scalbn(z, q0); /* actual value of z */
                z -= 8.0 * floor(z * 0.125); /* trim off integer >= 8 */
                n = z as i32;
                z -= n as f64;
                ih = 0;
                if q0 > 0 {
                    /* need iq[jz-1] to determine n */
                    i = i!(iq, jz - 1) >> (24 - q0);
                    n += i;
                    i!(iq, jz - 1, -=, i << (24 - q0));
                    ih = i!(iq, jz - 1) >> (23 - q0);
                } else if q0 == 0 {
                    ih = i!(iq, jz - 1) >> 23;
                } else if z >= 0.5 {
                    ih = 2;
                }

                if ih > 0 {
                    /* q > 0.5 */
                    n += 1;
                    let mut carry = 0i32;
                    for i in 0..jz {
                        /* compute 1-q */
                        let j = i!(iq, i);
                        if carry == 0 {
                            if j != 0 {
                                carry = 1;
                                i!(iq, i, =, 0x1000000 - j);
                            }
                        } else {
                            i!(iq, i, =, 0xffffff - j);
                        }
                    }
                    if q0 > 0 {
                        /* rare case: chance is 1 in 12 */
                        match q0 {
                            1 => {
                                i!(iq, jz - 1, &=, 0x7fffff);
                            }
                            2 => {
                                i!(iq, jz - 1, &=, 0x3fffff);
                            }
                            _ => {}
                        }
                    }
                    if ih == 2 {
                        z = 1. - z;
                        if carry != 0 {
                            z -= scalbn(1., q0);
                        }
                    }
                }

                /* check if recomputation is needed */
                if z == 0. {
                    let mut j = 0;
                    for i in (jk..=jz - 1).rev() {
                        j |= i!(iq, i);
                    }
                    if j == 0 {
                        /* need recomputation */
                        let mut k = 1;
                        while i!(iq, jk - k, ==, 0) {
                            k += 1; /* k = no. of terms needed */
                        }

                        for i in (jz + 1)..=(jz + k) {
                            /* add q[jz+1] to q[jz+k] */
                            i!(f, jx + i, =, i!(IPIO2, jv + i) as f64);
                            fw = 0f64;
                            for j in 0..=jx {
                                fw += i!(x, j) * i!(f, jx + i - j);
                            }
                            i!(q, i, =, fw);
                        }
                        jz += k;
                        continue 'recompute;
                    }
                }

                break;
            }

            /* chop off zero terms */
            if z == 0. {
                jz -= 1;
                q0 -= 24;
                while i!(iq, jz) == 0 {
                    jz -= 1;
                    q0 -= 24;
                }
            } else {
                /* break z into 24-bit if necessary */
                z = scalbn(z, -q0);
                if z >= x1p24 {
                    fw = (x1p_24 * z) as i32 as f64;
                    i!(iq, jz, =, (z - x1p24 * fw) as i32);
                    jz += 1;
                    q0 += 24;
                    i!(iq, jz, =, fw as i32);
                } else {
                    i!(iq, jz, =, z as i32);
                }
            }

            /* convert integer "bit" chunk to floating-point value */
            fw = scalbn(1., q0);
            for i in (0..=jz).rev() {
                i!(q, i, =, fw * (i!(iq, i) as f64));
                fw *= x1p_24;
            }

            /* compute PIo2[0,...,jp]*q[jz,...,0] */
            for i in (0..=jz).rev() {
                fw = 0f64;
                let mut k = 0;
                while (k <= jp) && (k <= jz - i) {
                    fw += i!(PIO2, k) * i!(q, i + k);
                    k += 1;
                }
                i!(fq, jz - i, =, fw);
            }

            /* compress fq[] into y[] */
            match prec {
                0 => {
                    fw = 0f64;
                    for i in (0..=jz).rev() {
                        fw += i!(fq, i);
                    }
                    i!(y, 0, =, if ih == 0 { fw } else { -fw });
                }
                1 | 2 => {
                    fw = 0f64;
                    for i in (0..=jz).rev() {
                        fw += i!(fq, i);
                    }
                    i!(y, 0, =, if ih == 0 { fw } else { -fw });
                    fw = i!(fq, 0) - fw;
                    for i in 1..=jz {
                        fw += i!(fq, i);
                    }
                    i!(y, 1, =, if ih == 0 { fw } else { -fw });
                }
                3 => {
                    /* painful */
                    for i in (1..=jz).rev() {
                        fw = i!(fq, i - 1) + i!(fq, i);
                        i!(fq, i, +=, i!(fq, i - 1) - fw);
                        i!(fq, i - 1, =, fw);
                    }
                    for i in (2..=jz).rev() {
                        fw = i!(fq, i - 1) + i!(fq, i);
                        i!(fq, i, +=, i!(fq, i - 1) - fw);
                        i!(fq, i - 1, =, fw);
                    }
                    fw = 0f64;
                    for i in (2..=jz).rev() {
                        fw += i!(fq, i);
                    }
                    if ih == 0 {
                        i!(y, 0, =, i!(fq, 0));
                        i!(y, 1, =, i!(fq, 1));
                        i!(y, 2, =, fw);
                    } else {
                        i!(y, 0, =, -i!(fq, 0));
                        i!(y, 1, =, -i!(fq, 1));
                        i!(y, 2, =, -fw);
                    }
                }
                #[cfg(debug_assertions)]
                _ => unreachable!(),
                #[cfg(not(debug_assertions))]
                _ => {}
            }
            n & 7
        }
    }

    #[allow(clippy::all, unused_imports, unused_unsafe, non_upper_case_globals)]
    pub mod rem_pio2 {
        use super::rem_pio2_large::rem_pio2_large;
        use super::scalbn;
        #[allow(unused_imports)]
        use super::*;
        // origin: FreeBSD /usr/src/lib/msun/src/e_rem_pio2.c
        //
        // ====================================================
        // Copyright (C) 1993 by Sun Microsystems, Inc. All rights reserved.
        //
        // Developed at SunPro, a Sun Microsystems, Inc. business.
        // Permission to use, copy, modify, and distribute this
        // software is freely granted, provided that this notice
        // is preserved.
        // ====================================================
        //
        // Optimized by Bruce D. Evans. */
        // #if FLT_EVAL_METHOD==0 || FLT_EVAL_METHOD==1
        // #define EPS DBL_EPSILON
        const EPS: f64 = 2.2204460492503131e-16;
        // #elif FLT_EVAL_METHOD==2
        // #define EPS LDBL_EPSILON
        // #endif

        // TODO: Support FLT_EVAL_METHOD?

        const TO_INT: f64 = 1.5 / EPS;
        /// 53 bits of 2/pi
        const INV_PIO2: f64 = 6.36619772367581382433e-01; /* 0x3FE45F30, 0x6DC9C883 */
        /// first 33 bits of pi/2
        const PIO2_1: f64 = 1.57079632673412561417e+00; /* 0x3FF921FB, 0x54400000 */
        /// pi/2 - PIO2_1
        const PIO2_1T: f64 = 6.07710050650619224932e-11; /* 0x3DD0B461, 0x1A626331 */
        /// second 33 bits of pi/2
        const PIO2_2: f64 = 6.07710050630396597660e-11; /* 0x3DD0B461, 0x1A600000 */
        /// pi/2 - (PIO2_1+PIO2_2)
        const PIO2_2T: f64 = 2.02226624879595063154e-21; /* 0x3BA3198A, 0x2E037073 */
        /// third 33 bits of pi/2
        const PIO2_3: f64 = 2.02226624871116645580e-21; /* 0x3BA3198A, 0x2E000000 */
        /// pi/2 - (PIO2_1+PIO2_2+PIO2_3)
        const PIO2_3T: f64 = 8.47842766036889956997e-32; /* 0x397B839A, 0x252049C1 */

        // return the remainder of x rem pi/2 in y[0]+y[1]
        // use rem_pio2_large() for large x
        //
        // caller must handle the case when reduction is not needed: |x| ~<= pi/4 */
        pub fn rem_pio2(x: f64) -> (i32, f64, f64) {
            let x1p24 = f64::from_bits(0x4170000000000000);

            let sign = (f64::to_bits(x) >> 63) as i32;
            let ix = (f64::to_bits(x) >> 32) as u32 & 0x7fffffff;

            fn medium(x: f64, ix: u32) -> (i32, f64, f64) {
                /* rint(x/(pi/2)), Assume round-to-nearest. */
                let tmp = x * INV_PIO2 + TO_INT;
                // force rounding of tmp to it's storage format on x87 to avoid
                // excess precision issues.
                let f_n = tmp - TO_INT;
                let n = f_n as i32;
                let mut r = x - f_n * PIO2_1;
                let mut w = f_n * PIO2_1T; /* 1st round, good to 85 bits */
                let mut y0 = r - w;
                let ui = f64::to_bits(y0);
                let ey = (ui >> 52) as i32 & 0x7ff;
                let ex = (ix >> 20) as i32;
                if ex - ey > 16 {
                    /* 2nd round, good to 118 bits */
                    let t = r;
                    w = f_n * PIO2_2;
                    r = t - w;
                    w = f_n * PIO2_2T - ((t - r) - w);
                    y0 = r - w;
                    let ey = (f64::to_bits(y0) >> 52) as i32 & 0x7ff;
                    if ex - ey > 49 {
                        /* 3rd round, good to 151 bits, covers all cases */
                        let t = r;
                        w = f_n * PIO2_3;
                        r = t - w;
                        w = f_n * PIO2_3T - ((t - r) - w);
                        y0 = r - w;
                    }
                }
                let y1 = (r - y0) - w;
                (n, y0, y1)
            }

            if ix <= 0x400f6a7a {
                /* |x| ~<= 5pi/4 */
                if (ix & 0xfffff) == 0x921fb {
                    /* |x| ~= pi/2 or 2pi/2 */
                    return medium(x, ix); /* cancellation -- use medium case */
                }
                if ix <= 0x4002d97c {
                    /* |x| ~<= 3pi/4 */
                    if sign == 0 {
                        let z = x - PIO2_1; /* one round good to 85 bits */
                        let y0 = z - PIO2_1T;
                        let y1 = (z - y0) - PIO2_1T;
                        return (1, y0, y1);
                    } else {
                        let z = x + PIO2_1;
                        let y0 = z + PIO2_1T;
                        let y1 = (z - y0) + PIO2_1T;
                        return (-1, y0, y1);
                    }
                } else if sign == 0 {
                    let z = x - 2.0 * PIO2_1;
                    let y0 = z - 2.0 * PIO2_1T;
                    let y1 = (z - y0) - 2.0 * PIO2_1T;
                    return (2, y0, y1);
                } else {
                    let z = x + 2.0 * PIO2_1;
                    let y0 = z + 2.0 * PIO2_1T;
                    let y1 = (z - y0) + 2.0 * PIO2_1T;
                    return (-2, y0, y1);
                }
            }
            if ix <= 0x401c463b {
                /* |x| ~<= 9pi/4 */
                if ix <= 0x4015fdbc {
                    /* |x| ~<= 7pi/4 */
                    if ix == 0x4012d97c {
                        /* |x| ~= 3pi/2 */
                        return medium(x, ix);
                    }
                    if sign == 0 {
                        let z = x - 3.0 * PIO2_1;
                        let y0 = z - 3.0 * PIO2_1T;
                        let y1 = (z - y0) - 3.0 * PIO2_1T;
                        return (3, y0, y1);
                    } else {
                        let z = x + 3.0 * PIO2_1;
                        let y0 = z + 3.0 * PIO2_1T;
                        let y1 = (z - y0) + 3.0 * PIO2_1T;
                        return (-3, y0, y1);
                    }
                } else {
                    if ix == 0x401921fb {
                        /* |x| ~= 4pi/2 */
                        return medium(x, ix);
                    }
                    if sign == 0 {
                        let z = x - 4.0 * PIO2_1;
                        let y0 = z - 4.0 * PIO2_1T;
                        let y1 = (z - y0) - 4.0 * PIO2_1T;
                        return (4, y0, y1);
                    } else {
                        let z = x + 4.0 * PIO2_1;
                        let y0 = z + 4.0 * PIO2_1T;
                        let y1 = (z - y0) + 4.0 * PIO2_1T;
                        return (-4, y0, y1);
                    }
                }
            }
            if ix < 0x413921fb {
                /* |x| ~< 2^20*(pi/2), medium size */
                return medium(x, ix);
            }
            /*
             * all other (large) arguments
             */
            if ix >= 0x7ff00000 {
                /* x is inf or NaN */
                let y0 = x - x;
                let y1 = y0;
                return (0, y0, y1);
            }
            /* set z = scalbn(|x|,-ilogb(x)+23) */
            let mut ui = f64::to_bits(x);
            ui &= (!1) >> 12;
            ui |= (0x3ff + 23) << 52;
            let mut z = f64::from_bits(ui);
            let mut tx = [0.0; 3];
            for i in 0..2 {
                i!(tx,i, =, z as i32 as f64);
                z = (z - i!(tx, i)) * x1p24;
            }
            i!(tx,2, =, z);
            /* skip zero terms, first term is non-zero */
            let mut i = 2;
            while i != 0 && i!(tx, i) == 0.0 {
                i -= 1;
            }
            let mut ty = [0.0; 3];
            let n = rem_pio2_large(&tx[..=i], &mut ty, ((ix as i32) >> 20) - (0x3ff + 23), 1);
            if sign != 0 {
                return (-n, -i!(ty, 0), -i!(ty, 1));
            }
            (n, i!(ty, 0), i!(ty, 1))
        }
    }

    #[allow(clippy::all, unused_imports, unused_unsafe, non_upper_case_globals)]
    pub mod sin {
        use super::k_cos::k_cos;
        use super::k_sin::k_sin;
        use super::rem_pio2::rem_pio2;
        #[allow(unused_imports)]
        use super::*;
        // origin: FreeBSD /usr/src/lib/msun/src/s_sin.c */
        //
        // ====================================================
        // Copyright (C) 1993 by Sun Microsystems, Inc. All rights reserved.
        //
        // Developed at SunPro, a Sun Microsystems, Inc. business.
        // Permission to use, copy, modify, and distribute this
        // software is freely granted, provided that this notice
        // is preserved.
        // ====================================================

        // sin(x)
        // Return sine function of x.
        //
        // kernel function:
        //      k_sin            ... sine function on [-pi/4,pi/4]
        //      k_cos            ... cose function on [-pi/4,pi/4]
        //      rem_pio2         ... argument reduction routine
        //
        // Method.
        //      Let S,C and T denote the sin, cos and tan respectively on
        //      [-PI/4, +PI/4]. Reduce the argument x to y1+y2 = x-k*pi/2
        //      in [-pi/4 , +pi/4], and let n = k mod 4.
        //      We have
        //
        //          n        sin(x)      cos(x)        tan(x)
        //     ----------------------------------------------------------
        //          0          S           C             T
        //          1          C          -S            -1/T
        //          2         -S          -C             T
        //          3         -C           S            -1/T
        //     ----------------------------------------------------------
        //
        // Special cases:
        //      Let trig be any of sin, cos, or tan.
        //      trig(+-INF)  is NaN, with signals;
        //      trig(NaN)    is that NaN;
        //
        // Accuracy:
        //      TRIG(x) returns trig(x) nearly rounded

        /// The sine of `x` (f64).
        ///
        /// `x` is specified in radians.
        pub fn sin(x: f64) -> f64 {
            let x1p120 = f64::from_bits(0x4770000000000000); // 0x1p120f === 2 ^ 120

            /* High word of x. */
            let ix = (f64::to_bits(x) >> 32) as u32 & 0x7fffffff;

            /* |x| ~< pi/4 */
            if ix <= 0x3fe921fb {
                if ix < 0x3e500000 {
                    /* |x| < 2**-26 */
                    /* raise inexact if x != 0 and underflow if subnormal*/
                    if ix < 0x00100000 {
                        force_eval!(x / x1p120);
                    } else {
                        force_eval!(x + x1p120);
                    }
                    return x;
                }
                return k_sin(x, 0.0, 0);
            }

            /* sin(Inf or NaN) is NaN */
            if ix >= 0x7ff00000 {
                return x - x;
            }

            /* argument reduction needed */
            let (n, y0, y1) = rem_pio2(x);
            match n & 3 {
                0 => k_sin(y0, y1, 1),
                1 => k_cos(y0, y1),
                2 => -k_sin(y0, y1, 1),
                _ => -k_cos(y0, y1),
            }
        }
    }

    #[allow(clippy::all, unused_imports, unused_unsafe, non_upper_case_globals)]
    pub mod cos {
        use super::k_cos::k_cos;
        use super::k_sin::k_sin;
        use super::rem_pio2::rem_pio2;
        #[allow(unused_imports)]
        use super::*;
        // origin: FreeBSD /usr/src/lib/msun/src/s_cos.c */
        //
        // ====================================================
        // Copyright (C) 1993 by Sun Microsystems, Inc. All rights reserved.
        //
        // Developed at SunPro, a Sun Microsystems, Inc. business.
        // Permission to use, copy, modify, and distribute this
        // software is freely granted, provided that this notice
        // is preserved.
        // ====================================================

        // cos(x)
        // Return cosine function of x.
        //
        // kernel function:
        //      k_sin           ... sine function on [-pi/4,pi/4]
        //      k_cos           ... cosine function on [-pi/4,pi/4]
        //      rem_pio2        ... argument reduction routine
        //
        // Method.
        //      Let S,C and T denote the sin, cos and tan respectively on
        //      [-PI/4, +PI/4]. Reduce the argument x to y1+y2 = x-k*pi/2
        //      in [-pi/4 , +pi/4], and let n = k mod 4.
        //      We have
        //
        //          n        sin(x)      cos(x)        tan(x)
        //     ----------------------------------------------------------
        //          0          S           C             T
        //          1          C          -S            -1/T
        //          2         -S          -C             T
        //          3         -C           S            -1/T
        //     ----------------------------------------------------------
        //
        // Special cases:
        //      Let trig be any of sin, cos, or tan.
        //      trig(+-INF)  is NaN, with signals;
        //      trig(NaN)    is that NaN;
        //
        // Accuracy:
        //      TRIG(x) returns trig(x) nearly rounded
        //

        /// The cosine of `x` (f64).
        ///
        /// `x` is specified in radians.
        pub fn cos(x: f64) -> f64 {
            let ix = (f64::to_bits(x) >> 32) as u32 & 0x7fffffff;

            /* |x| ~< pi/4 */
            if ix <= 0x3fe921fb {
                if ix < 0x3e46a09e {
                    /* if x < 2**-27 * f64::sqrt(2) */
                    /* raise inexact if x != 0 */
                    if x as i32 == 0 {
                        return 1.0;
                    }
                }
                return k_cos(x, 0.0);
            }

            /* cos(Inf or NaN) is NaN */
            if ix >= 0x7ff00000 {
                return x - x;
            }

            /* argument reduction needed */
            let (n, y0, y1) = rem_pio2(x);
            match n & 3 {
                0 => k_cos(y0, y1),
                1 => -k_sin(y0, y1, 1),
                2 => -k_cos(y0, y1),
                _ => k_sin(y0, y1, 1),
            }
        }
    }

    #[allow(clippy::all, unused_imports, unused_unsafe, non_upper_case_globals)]
    pub mod tan {
        use super::k_tan::k_tan;
        use super::rem_pio2::rem_pio2;
        #[allow(unused_imports)]
        use super::*;
        // origin: FreeBSD /usr/src/lib/msun/src/s_tan.c */
        //
        // ====================================================
        // Copyright (C) 1993 by Sun Microsystems, Inc. All rights reserved.
        //
        // Developed at SunPro, a Sun Microsystems, Inc. business.
        // Permission to use, copy, modify, and distribute this
        // software is freely granted, provided that this notice
        // is preserved.
        // ====================================================

        // tan(x)
        // Return tangent function of x.
        //
        // kernel function:
        //      k_tan           ... tangent function on [-pi/4,pi/4]
        //      rem_pio2        ... argument reduction routine
        //
        // Method.
        //      Let S,C and T denote the sin, cos and tan respectively on
        //      [-PI/4, +PI/4]. Reduce the argument x to y1+y2 = x-k*pi/2
        //      in [-pi/4 , +pi/4], and let n = k mod 4.
        //      We have
        //
        //          n        sin(x)      cos(x)        tan(x)
        //     ----------------------------------------------------------
        //          0          S           C             T
        //          1          C          -S            -1/T
        //          2         -S          -C             T
        //          3         -C           S            -1/T
        //     ----------------------------------------------------------
        //
        // Special cases:
        //      Let trig be any of sin, cos, or tan.
        //      trig(+-INF)  is NaN, with signals;
        //      trig(NaN)    is that NaN;
        //
        // Accuracy:
        //      TRIG(x) returns trig(x) nearly rounded

        /// The tangent of `x` (f64).
        ///
        /// `x` is specified in radians.
        pub fn tan(x: f64) -> f64 {
            let x1p120 = f32::from_bits(0x7b800000); // 0x1p120f === 2 ^ 120

            let ix = (f64::to_bits(x) >> 32) as u32 & 0x7fffffff;
            /* |x| ~< pi/4 */
            if ix <= 0x3fe921fb {
                if ix < 0x3e400000 {
                    /* |x| < 2**-27 */
                    /* raise inexact if x!=0 and underflow if subnormal */
                    force_eval!(if ix < 0x00100000 { x / x1p120 as f64 } else { x + x1p120 as f64 });
                    return x;
                }
                return k_tan(x, 0.0, 0);
            }

            /* tan(Inf or NaN) is NaN */
            if ix >= 0x7ff00000 {
                return x - x;
            }

            /* argument reduction */
            let (n, y0, y1) = rem_pio2(x);
            k_tan(y0, y1, n & 1)
        }
    }

    #[allow(clippy::all, unused_imports, unused_unsafe, non_upper_case_globals)]
    pub mod atan {
        #[allow(unused_imports)]
        use super::*;
        /* origin: FreeBSD /usr/src/lib/msun/src/s_atan.c */
        /*
         * ====================================================
         * Copyright (C) 1993 by Sun Microsystems, Inc. All rights reserved.
         *
         * Developed at SunPro, a Sun Microsystems, Inc. business.
         * Permission to use, copy, modify, and distribute this
         * software is freely granted, provided that this notice
         * is preserved.
         * ====================================================
         */
        /* atan(x)
         * Method
         *   1. Reduce x to positive by atan(x) = -atan(-x).
         *   2. According to the integer k=4t+0.25 chopped, t=x, the argument
         *      is further reduced to one of the following intervals and the
         *      arctangent of t is evaluated by the corresponding formula:
         *
         *      [0,7/16]      atan(x) = t-t^3*(a1+t^2*(a2+...(a10+t^2*a11)...)
         *      [7/16,11/16]  atan(x) = atan(1/2) + atan( (t-0.5)/(1+t/2) )
         *      [11/16.19/16] atan(x) = atan( 1 ) + atan( (t-1)/(1+t) )
         *      [19/16,39/16] atan(x) = atan(3/2) + atan( (t-1.5)/(1+1.5t) )
         *      [39/16,INF]   atan(x) = atan(INF) + atan( -1/t )
         *
         * Constants:
         * The hexadecimal values are the intended ones for the following
         * constants. The decimal values may be used, provided that the
         * compiler will convert from decimal to binary accurately enough
         * to produce the hexadecimal values shown.
         */

        const ATANHI: [f64; 4] = [
            4.63647609000806093515e-01, /* atan(0.5)hi 0x3FDDAC67, 0x0561BB4F */
            7.85398163397448278999e-01, /* atan(1.0)hi 0x3FE921FB, 0x54442D18 */
            9.82793723247329054082e-01, /* atan(1.5)hi 0x3FEF730B, 0xD281F69B */
            1.57079632679489655800e+00, /* atan(inf)hi 0x3FF921FB, 0x54442D18 */
        ];

        const ATANLO: [f64; 4] = [
            2.26987774529616870924e-17, /* atan(0.5)lo 0x3C7A2B7F, 0x222F65E2 */
            3.06161699786838301793e-17, /* atan(1.0)lo 0x3C81A626, 0x33145C07 */
            1.39033110312309984516e-17, /* atan(1.5)lo 0x3C700788, 0x7AF0CBBD */
            6.12323399573676603587e-17, /* atan(inf)lo 0x3C91A626, 0x33145C07 */
        ];

        const AT: [f64; 11] = [
            3.33333333333329318027e-01,  /* 0x3FD55555, 0x5555550D */
            -1.99999999998764832476e-01, /* 0xBFC99999, 0x9998EBC4 */
            1.42857142725034663711e-01,  /* 0x3FC24924, 0x920083FF */
            -1.11111104054623557880e-01, /* 0xBFBC71C6, 0xFE231671 */
            9.09088713343650656196e-02,  /* 0x3FB745CD, 0xC54C206E */
            -7.69187620504482999495e-02, /* 0xBFB3B0F2, 0xAF749A6D */
            6.66107313738753120669e-02,  /* 0x3FB10D66, 0xA0D03D51 */
            -5.83357013379057348645e-02, /* 0xBFADDE2D, 0x52DEFD9A */
            4.97687799461593236017e-02,  /* 0x3FA97B4B, 0x24760DEB */
            -3.65315727442169155270e-02, /* 0xBFA2B444, 0x2C6A6C2F */
            1.62858201153657823623e-02,  /* 0x3F90AD3A, 0xE322DA11 */
        ];

        /// Arctangent (f64)
        ///
        /// Computes the inverse tangent (arc tangent) of the input value.
        /// Returns a value in radians, in the range of -pi/2 to pi/2.
        pub fn atan(x: f64) -> f64 {
            let mut x = x;
            let mut ix = (x.to_bits() >> 32) as u32;
            let sign = ix >> 31;
            ix &= 0x7fff_ffff;
            if ix >= 0x4410_0000 {
                if x.is_nan() {
                    return x;
                }

                let z = ATANHI[3] + f64::from_bits(0x0380_0000); // 0x1p-120f
                return if sign != 0 { -z } else { z };
            }

            let id = if ix < 0x3fdc_0000 {
                /* |x| < 0.4375 */
                if ix < 0x3e40_0000 {
                    /* |x| < 2^-27 */
                    if ix < 0x0010_0000 {
                        /* raise underflow for subnormal x */
                        force_eval!(x as f32);
                    }

                    return x;
                }

                -1
            } else {
                x = f64::abs(x);
                if ix < 0x3ff30000 {
                    /* |x| < 1.1875 */
                    if ix < 0x3fe60000 {
                        /* 7/16 <= |x| < 11/16 */
                        x = (2. * x - 1.) / (2. + x);
                        0
                    } else {
                        /* 11/16 <= |x| < 19/16 */
                        x = (x - 1.) / (x + 1.);
                        1
                    }
                } else if ix < 0x40038000 {
                    /* |x| < 2.4375 */
                    x = (x - 1.5) / (1. + 1.5 * x);
                    2
                } else {
                    /* 2.4375 <= |x| < 2^66 */
                    x = -1. / x;
                    3
                }
            };

            let z = x * x;
            let w = z * z;
            /* break sum from i=0 to 10 AT[i]z**(i+1) into odd and even poly */
            let s1 = z * (AT[0] + w * (AT[2] + w * (AT[4] + w * (AT[6] + w * (AT[8] + w * AT[10])))));
            let s2 = w * (AT[1] + w * (AT[3] + w * (AT[5] + w * (AT[7] + w * AT[9]))));

            if id < 0 {
                return x - x * (s1 + s2);
            }

            let z = i!(ATANHI, id as usize) - (x * (s1 + s2) - i!(ATANLO, id as usize) - x);

            if sign != 0 {
                -z
            } else {
                z
            }
        }
    }

    #[allow(clippy::all, unused_imports, unused_unsafe, non_upper_case_globals)]
    pub mod atan2 {
        use super::atan::atan;
        #[allow(unused_imports)]
        use super::*;
        /* origin: FreeBSD /usr/src/lib/msun/src/e_atan2.c */
        /*
         * ====================================================
         * Copyright (C) 1993 by Sun Microsystems, Inc. All rights reserved.
         *
         * Developed at SunSoft, a Sun Microsystems, Inc. business.
         * Permission to use, copy, modify, and distribute this
         * software is freely granted, provided that this notice
         * is preserved.
         * ====================================================
         *
         */
        /* atan2(y,x)
         * Method :
         *      1. Reduce y to positive by atan2(y,x)=-atan2(-y,x).
         *      2. Reduce x to positive by (if x and y are unexceptional):
         *              ARG (x+iy) = arctan(y/x)           ... if x > 0,
         *              ARG (x+iy) = pi - arctan[y/(-x)]   ... if x < 0,
         *
         * Special cases:
         *
         *      ATAN2((anything), NaN ) is NaN;
         *      ATAN2(NAN , (anything) ) is NaN;
         *      ATAN2(+-0, +(anything but NaN)) is +-0  ;
         *      ATAN2(+-0, -(anything but NaN)) is +-pi ;
         *      ATAN2(+-(anything but 0 and NaN), 0) is +-pi/2;
         *      ATAN2(+-(anything but INF and NaN), +INF) is +-0 ;
         *      ATAN2(+-(anything but INF and NaN), -INF) is +-pi;
         *      ATAN2(+-INF,+INF ) is +-pi/4 ;
         *      ATAN2(+-INF,-INF ) is +-3pi/4;
         *      ATAN2(+-INF, (anything but,0,NaN, and INF)) is +-pi/2;
         *
         * Constants:
         * The hexadecimal values are the intended ones for the following
         * constants. The decimal values may be used, provided that the
         * compiler will convert from decimal to binary accurately enough
         * to produce the hexadecimal values shown.
         */

        const PI: f64 = 3.1415926535897931160E+00; /* 0x400921FB, 0x54442D18 */
        const PI_LO: f64 = 1.2246467991473531772E-16; /* 0x3CA1A626, 0x33145C07 */

        /// Arctangent of y/x (f64)
        ///
        /// Computes the inverse tangent (arc tangent) of `y/x`.
        /// Produces the correct result even for angles near pi/2 or -pi/2 (that is, when `x` is near 0).
        /// Returns a value in radians, in the range of -pi to pi.
        pub fn atan2(y: f64, x: f64) -> f64 {
            if x.is_nan() || y.is_nan() {
                return x + y;
            }
            let mut ix = (x.to_bits() >> 32) as u32;
            let lx = x.to_bits() as u32;
            let mut iy = (y.to_bits() >> 32) as u32;
            let ly = y.to_bits() as u32;
            if ((ix.wrapping_sub(0x3ff00000)) | lx) == 0 {
                /* x = 1.0 */
                return atan(y);
            }
            let m = ((iy >> 31) & 1) | ((ix >> 30) & 2); /* 2*sign(x)+sign(y) */
            ix &= 0x7fffffff;
            iy &= 0x7fffffff;

            /* when y = 0 */
            if (iy | ly) == 0 {
                return match m {
                    0 | 1 => y, /* atan(+-0,+anything)=+-0 */
                    2 => PI,    /* atan(+0,-anything) = PI */
                    _ => -PI,   /* atan(-0,-anything) =-PI */
                };
            }
            /* when x = 0 */
            if (ix | lx) == 0 {
                return if m & 1 != 0 { -PI / 2.0 } else { PI / 2.0 };
            }
            /* when x is INF */
            if ix == 0x7ff00000 {
                if iy == 0x7ff00000 {
                    return match m {
                        0 => PI / 4.0,        /* atan(+INF,+INF) */
                        1 => -PI / 4.0,       /* atan(-INF,+INF) */
                        2 => 3.0 * PI / 4.0,  /* atan(+INF,-INF) */
                        _ => -3.0 * PI / 4.0, /* atan(-INF,-INF) */
                    };
                } else {
                    return match m {
                        0 => 0.0,  /* atan(+...,+INF) */
                        1 => -0.0, /* atan(-...,+INF) */
                        2 => PI,   /* atan(+...,-INF) */
                        _ => -PI,  /* atan(-...,-INF) */
                    };
                }
            }
            /* |y/x| > 0x1p64 */
            if ix.wrapping_add(64 << 20) < iy || iy == 0x7ff00000 {
                return if m & 1 != 0 { -PI / 2.0 } else { PI / 2.0 };
            }

            /* z = atan(|y/x|) without spurious underflow */
            let z = if (m & 2 != 0) && iy.wrapping_add(64 << 20) < ix {
                /* |y/x| < 0x1p-64, x<0 */
                0.0
            } else {
                atan(f64::abs(y / x))
            };
            match m {
                0 => z,                /* atan(+,+) */
                1 => -z,               /* atan(-,+) */
                2 => PI - (z - PI_LO), /* atan(+,-) */
                _ => (z - PI_LO) - PI, /* atan(-,-) */
            }
        }
    }

    #[allow(clippy::all, unused_imports, unused_unsafe, non_upper_case_globals)]
    pub mod exp {
        use super::scalbn;
        #[allow(unused_imports)]
        use super::*;
        /* origin: FreeBSD /usr/src/lib/msun/src/e_exp.c */
        /*
         * ====================================================
         * Copyright (C) 2004 by Sun Microsystems, Inc. All rights reserved.
         *
         * Permission to use, copy, modify, and distribute this
         * software is freely granted, provided that this notice
         * is preserved.
         * ====================================================
         */
        /* exp(x)
         * Returns the exponential of x.
         *
         * Method
         *   1. Argument reduction:
         *      Reduce x to an r so that |r| <= 0.5*ln2 ~ 0.34658.
         *      Given x, find r and integer k such that
         *
         *               x = k*ln2 + r,  |r| <= 0.5*ln2.
         *
         *      Here r will be represented as r = hi-lo for better
         *      accuracy.
         *
         *   2. Approximation of exp(r) by a special rational function on
         *      the interval [0,0.34658]:
         *      Write
         *          R(r**2) = r*(exp(r)+1)/(exp(r)-1) = 2 + r*r/6 - r**4/360 + ...
         *      We use a special Remez algorithm on [0,0.34658] to generate
         *      a polynomial of degree 5 to approximate R. The maximum error
         *      of this polynomial approximation is bounded by 2**-59. In
         *      other words,
         *          R(z) ~ 2.0 + P1*z + P2*z**2 + P3*z**3 + P4*z**4 + P5*z**5
         *      (where z=r*r, and the values of P1 to P5 are listed below)
         *      and
         *          |                  5          |     -59
         *          | 2.0+P1*z+...+P5*z   -  R(z) | <= 2
         *          |                             |
         *      The computation of exp(r) thus becomes
         *                              2*r
         *              exp(r) = 1 + ----------
         *                            R(r) - r
         *                                 r*c(r)
         *                     = 1 + r + ----------- (for better accuracy)
         *                                2 - c(r)
         *      where
         *                              2       4             10
         *              c(r) = r - (P1*r  + P2*r  + ... + P5*r   ).
         *
         *   3. Scale back to obtain exp(x):
         *      From step 1, we have
         *         exp(x) = 2^k * exp(r)
         *
         * Special cases:
         *      exp(INF) is INF, exp(NaN) is NaN;
         *      exp(-INF) is 0, and
         *      for finite argument, only exp(0)=1 is exact.
         *
         * Accuracy:
         *      according to an error analysis, the error is always less than
         *      1 ulp (unit in the last place).
         *
         * Misc. info.
         *      For IEEE double
         *          if x >  709.782712893383973096 then exp(x) overflows
         *          if x < -745.133219101941108420 then exp(x) underflows
         */

        const HALF: [f64; 2] = [0.5, -0.5];
        const LN2HI: f64 = 6.93147180369123816490e-01; /* 0x3fe62e42, 0xfee00000 */
        const LN2LO: f64 = 1.90821492927058770002e-10; /* 0x3dea39ef, 0x35793c76 */
        const INVLN2: f64 = 1.44269504088896338700e+00; /* 0x3ff71547, 0x652b82fe */
        const P1: f64 = 1.66666666666666019037e-01; /* 0x3FC55555, 0x5555553E */
        const P2: f64 = -2.77777777770155933842e-03; /* 0xBF66C16C, 0x16BEBD93 */
        const P3: f64 = 6.61375632143793436117e-05; /* 0x3F11566A, 0xAF25DE2C */
        const P4: f64 = -1.65339022054652515390e-06; /* 0xBEBBBD41, 0xC5D26BF1 */
        const P5: f64 = 4.13813679705723846039e-08; /* 0x3E663769, 0x72BEA4D0 */

        /// Exponential, base *e* (f64)
        ///
        /// Calculate the exponential of `x`, that is, *e* raised to the power `x`
        /// (where *e* is the base of the natural system of logarithms, approximately 2.71828).
        pub fn exp(mut x: f64) -> f64 {
            let x1p1023 = f64::from_bits(0x7fe0000000000000); // 0x1p1023 === 2 ^ 1023
            let x1p_149 = f64::from_bits(0x36a0000000000000); // 0x1p-149 === 2 ^ -149

            let hi: f64;
            let lo: f64;
            let c: f64;
            let xx: f64;
            let y: f64;
            let k: i32;
            let sign: i32;
            let mut hx: u32;

            hx = (x.to_bits() >> 32) as u32;
            sign = (hx >> 31) as i32;
            hx &= 0x7fffffff; /* high word of |x| */

            /* special cases */
            if hx >= 0x4086232b {
                /* if |x| >= 708.39... */
                if x.is_nan() {
                    return x;
                }
                if x > 709.782712893383973096 {
                    /* overflow if x!=inf */
                    x *= x1p1023;
                    return x;
                }
                if x < -708.39641853226410622 {
                    /* underflow if x!=-inf */
                    force_eval!((-x1p_149 / x) as f32);
                    if x < -745.13321910194110842 {
                        return 0.;
                    }
                }
            }

            /* argument reduction */
            if hx > 0x3fd62e42 {
                /* if |x| > 0.5 ln2 */
                if hx >= 0x3ff0a2b2 {
                    /* if |x| >= 1.5 ln2 */
                    k = (INVLN2 * x + i!(HALF, sign as usize)) as i32;
                } else {
                    k = 1 - sign - sign;
                }
                hi = x - k as f64 * LN2HI; /* k*ln2hi is exact here */
                lo = k as f64 * LN2LO;
                x = hi - lo;
            } else if hx > 0x3e300000 {
                /* if |x| > 2**-28 */
                k = 0;
                hi = x;
                lo = 0.;
            } else {
                /* inexact if x!=0 */
                force_eval!(x1p1023 + x);
                return 1. + x;
            }

            /* x is now in primary range */
            xx = x * x;
            c = x - xx * (P1 + xx * (P2 + xx * (P3 + xx * (P4 + xx * P5))));
            y = 1. + (x * c / (2. - c) - lo + hi);
            if k == 0 {
                y
            } else {
                scalbn(y, k)
            }
        }
    }

    #[allow(clippy::all, unused_imports, unused_unsafe, non_upper_case_globals)]
    pub mod log {
        #[allow(unused_imports)]
        use super::*;
        /* origin: FreeBSD /usr/src/lib/msun/src/e_log.c */
        /*
         * ====================================================
         * Copyright (C) 1993 by Sun Microsystems, Inc. All rights reserved.
         *
         * Developed at SunSoft, a Sun Microsystems, Inc. business.
         * Permission to use, copy, modify, and distribute this
         * software is freely granted, provided that this notice
         * is preserved.
         * ====================================================
         */
        /* log(x)
         * Return the logarithm of x
         *
         * Method :
         *   1. Argument Reduction: find k and f such that
         *                      x = 2^k * (1+f),
         *         where  f64::sqrt(2)/2 < 1+f < f64::sqrt(2) .
         *
         *   2. Approximation of log(1+f).
         *      Let s = f/(2+f) ; based on log(1+f) = log(1+s) - log(1-s)
         *               = 2s + 2/3 s**3 + 2/5 s**5 + .....,
         *               = 2s + s*R
         *      We use a special Remez algorithm on [0,0.1716] to generate
         *      a polynomial of degree 14 to approximate R The maximum error
         *      of this polynomial approximation is bounded by 2**-58.45. In
         *      other words,
         *                      2      4      6      8      10      12      14
         *          R(z) ~ Lg1*s +Lg2*s +Lg3*s +Lg4*s +Lg5*s  +Lg6*s  +Lg7*s
         *      (the values of Lg1 to Lg7 are listed in the program)
         *      and
         *          |      2          14          |     -58.45
         *          | Lg1*s +...+Lg7*s    -  R(z) | <= 2
         *          |                             |
         *      Note that 2s = f - s*f = f - hfsq + s*hfsq, where hfsq = f*f/2.
         *      In order to guarantee error in log below 1ulp, we compute log
         *      by
         *              log(1+f) = f - s*(f - R)        (if f is not too large)
         *              log(1+f) = f - (hfsq - s*(hfsq+R)).     (better accuracy)
         *
         *      3. Finally,  log(x) = k*ln2 + log(1+f).
         *                          = k*ln2_hi+(f-(hfsq-(s*(hfsq+R)+k*ln2_lo)))
         *         Here ln2 is split into two floating point number:
         *                      ln2_hi + ln2_lo,
         *         where n*ln2_hi is always exact for |n| < 2000.
         *
         * Special cases:
         *      log(x) is NaN with signal if x < 0 (including -INF) ;
         *      log(+INF) is +INF; log(0) is -INF with signal;
         *      log(NaN) is that NaN with no signal.
         *
         * Accuracy:
         *      according to an error analysis, the error is always less than
         *      1 ulp (unit in the last place).
         *
         * Constants:
         * The hexadecimal values are the intended ones for the following
         * constants. The decimal values may be used, provided that the
         * compiler will convert from decimal to binary accurately enough
         * to produce the hexadecimal values shown.
         */

        const LN2_HI: f64 = 6.93147180369123816490e-01; /* 3fe62e42 fee00000 */
        const LN2_LO: f64 = 1.90821492927058770002e-10; /* 3dea39ef 35793c76 */
        const LG1: f64 = 6.666666666666735130e-01; /* 3FE55555 55555593 */
        const LG2: f64 = 3.999999999940941908e-01; /* 3FD99999 9997FA04 */
        const LG3: f64 = 2.857142874366239149e-01; /* 3FD24924 94229359 */
        const LG4: f64 = 2.222219843214978396e-01; /* 3FCC71C5 1D8E78AF */
        const LG5: f64 = 1.818357216161805012e-01; /* 3FC74664 96CB03DE */
        const LG6: f64 = 1.531383769920937332e-01; /* 3FC39A09 D078C69F */
        const LG7: f64 = 1.479819860511658591e-01; /* 3FC2F112 DF3E5244 */

        /// The natural logarithm of `x` (f64).
        pub fn log(mut x: f64) -> f64 {
            let x1p54 = f64::from_bits(0x4350000000000000); // 0x1p54 === 2 ^ 54

            let mut ui = x.to_bits();
            let mut hx: u32 = (ui >> 32) as u32;
            let mut k: i32 = 0;

            if (hx < 0x00100000) || ((hx >> 31) != 0) {
                /* x < 2**-126  */
                if ui << 1 == 0 {
                    return -1. / (x * x); /* log(+-0)=-inf */
                }
                if hx >> 31 != 0 {
                    return (x - x) / 0.0; /* log(-#) = NaN */
                }
                /* subnormal number, scale x up */
                k -= 54;
                x *= x1p54;
                ui = x.to_bits();
                hx = (ui >> 32) as u32;
            } else if hx >= 0x7ff00000 {
                return x;
            } else if hx == 0x3ff00000 && ui << 32 == 0 {
                return 0.;
            }

            /* reduce x into [f64::sqrt(2)/2, f64::sqrt(2)] */
            hx += 0x3ff00000 - 0x3fe6a09e;
            k += ((hx >> 20) as i32) - 0x3ff;
            hx = (hx & 0x000fffff) + 0x3fe6a09e;
            ui = ((hx as u64) << 32) | (ui & 0xffffffff);
            x = f64::from_bits(ui);

            let f: f64 = x - 1.0;
            let hfsq: f64 = 0.5 * f * f;
            let s: f64 = f / (2.0 + f);
            let z: f64 = s * s;
            let w: f64 = z * z;
            let t1: f64 = w * (LG2 + w * (LG4 + w * LG6));
            let t2: f64 = z * (LG1 + w * (LG3 + w * (LG5 + w * LG7)));
            let r: f64 = t2 + t1;
            let dk: f64 = k as f64;
            s * (hfsq + r) + dk * LN2_LO - hfsq + f + dk * LN2_HI
        }
    }
}

#[cfg(test)]
#[path = "fdlibm.test.rs"]
mod test;
