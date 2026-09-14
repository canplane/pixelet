// pxl_shared/spec/space/pvf3/pvf3.test.rs
// (no pair: pvf3.ts has no test file; the vector holds ivec, fvec and pvf3
// to the TypeScript bit for bit)

use xpute_core::vector::{caught, Vector};

use super::*;
use crate::spec::space::pvf3::{fvec, ivec};

/// script/vector/pxl/shared/spec/space/pvf3/pvf3.ts's output.
const VECTOR: &str = include_str!("../../../../vector/spec/space/pvf3/pvf3.tsv");

fn tuple(v: &Vector, k: &str) -> PVF3 {
    PVF3 {
        i: v.u64(&format!("{k}.i")),
        f: v.u64(&format!("{k}.f")),
    }
}

const SHIFTS: [i32; 11] = [-40, -24, -17, -8, -1, 0, 1, 7, 8, 16, 23];

macro_rules! words {
    ($v:ident, $m:ident, $base:literal, $with_signed:expr) => {{
        $v.each(concat!($base, ".unary"), |k| {
            let w = $v.u64(&format!("{k}.w"));
            assert_eq!($m::X(w), $v.i64(&format!("{k}.X")), "{k}.X");
            assert_eq!($m::Y(w), $v.i64(&format!("{k}.Y")), "{k}.Y");
            assert_eq!($m::Z(w), $v.i64(&format!("{k}.Z")), "{k}.Z");
            assert_eq!($m::repr(w), $v.s(&format!("{k}.repr")), "{k}.repr");
            assert_eq!($m::not(w), $v.u64(&format!("{k}.not")), "{k}.not");
            for (j, &sh) in SHIFTS.iter().enumerate() {
                let want = $v.s(&format!("{k}.sh.{j}"));
                let got = caught(|| $m::sh(w, sh)).map_or("throw".to_string(), |x| x.to_string());
                assert_eq!(got, want, "{k}.sh({sh})");
            }
            assert_eq!($m::max_comp(w), $v.i64(&format!("{k}.max_comp")), "{k}.max_comp");
            assert_eq!($m::min_comp(w), $v.i64(&format!("{k}.min_comp")), "{k}.min_comp");
            assert_eq!($m::sum(w), $v.i64(&format!("{k}.sum")), "{k}.sum");
            assert_eq!($m::norm1(w), $v.i64(&format!("{k}.norm1")), "{k}.norm1");
            assert_eq!($m::norm_sq(w), $v.i64(&format!("{k}.norm_sq")), "{k}.norm_sq");
            $with_signed(k, w);
        });
        $v.each(concat!($base, ".binary"), |k| {
            let (a, b) = ($v.u64(&format!("{k}.v")), $v.u64(&format!("{k}.w")));
            let u = |name: &str, got: u64| assert_eq!(got, $v.u64(&format!("{k}.{name}")), "{k}.{name}");
            let i = |name: &str, got: i64| assert_eq!(got, $v.i64(&format!("{k}.{name}")), "{k}.{name}");
            u("and", $m::and(a, b));
            u("or", $m::or(a, b));
            u("xor", $m::xor(a, b));
            u("add", $m::add(a, b));
            u("sub", $m::sub(a, b));
            u("mul", $m::mul(a, b));
            assert_eq!(caught(|| $m::div(a, b)).map_or("throw".to_string(), |x| x.to_string()), $v.s(&format!("{k}.div")), "{k}.div");
            assert_eq!(caught(|| $m::r#mod(a, b)).map_or("throw".to_string(), |x| x.to_string()), $v.s(&format!("{k}.mod")), "{k}.mod");
            u("eq", $m::eq(a, b));
            assert_eq!($m::equals(a, b), $v.bool(&format!("{k}.equals")), "{k}.equals");
            u("lt", $m::lt(a, b));
            u("gt", $m::gt(a, b));
            u("max", $m::max(a, b));
            u("min", $m::min(a, b));
            u("clamp", $m::clamp(a, -5, 300));
            i("dot", $m::dot(a, b));
            i("dist1", $m::dist1(a, b));
            i("dist_sq", $m::dist_sq(a, b));
        });
        $v.each(concat!($base, ".from_real"), |k| {
            assert_eq!($m::from_real($v.f64(&format!("{k}.r"))), $v.i64(&format!("{k}.v")), "{k}.from_real");
        });
        $v.each(concat!($base, ".to_real"), |k| {
            assert_eq!($m::to_real($v.i64(&format!("{k}.c"))).to_bits(), $v.f64(&format!("{k}.r")).to_bits(), "{k}.to_real");
        });
    }};
}

#[test]
fn ivec_computes_what_the_typescript_computes() {
    let v = Vector::load(VECTOR);
    words!(v, ivec, "ivec", |k: &str, w: u64| {
        assert_eq!(ivec::neg(w), v.u64(&format!("{k}.neg")), "{k}.neg");
        assert_eq!(ivec::abs(w), v.u64(&format!("{k}.abs")), "{k}.abs");
    });
    v.each("ivec.gather", |k| {
        assert_eq!(
            ivec::gather(v.i64(&format!("{k}.x")), v.i64(&format!("{k}.y")), v.i64(&format!("{k}.z"))),
            v.u64(&format!("{k}.w")),
            "{k}"
        );
    });
}

#[test]
fn fvec_computes_what_the_typescript_computes() {
    let v = Vector::load(VECTOR);
    words!(v, fvec, "fvec", |_: &str, _: u64| {});
}

#[test]
fn pvf3_computes_what_the_typescript_computes() {
    let v = Vector::load(VECTOR);
    let out = || PVF3::default();
    v.each("pvf3.unary", |k| {
        let p = tuple(&v, &format!("{k}.p"));
        assert_eq!(X(&p), v.i64(&format!("{k}.X")), "{k}.X");
        assert_eq!(Y(&p), v.i64(&format!("{k}.Y")), "{k}.Y");
        assert_eq!(Z(&p), v.i64(&format!("{k}.Z")), "{k}.Z");
        assert_eq!(repr(&p), v.s(&format!("{k}.repr")), "{k}.repr");
        assert_eq!(*not(&mut out(), &p), tuple(&v, &format!("{k}.not")), "{k}.not");
        for (j, &s) in SHIFTS.iter().enumerate() {
            let key = format!("{k}.sh.{j}");
            let got = caught(|| *sh(&mut out(), &p, s));
            if v.has(&key) {
                assert_eq!(v.s(&key), "throw", "{key}");
                assert!(got.is_none(), "{key}: the TypeScript threw");
            } else {
                assert_eq!(got, Some(tuple(&v, &key)), "{key}");
            }
        }
        assert_eq!(*neg(&mut out(), &p), tuple(&v, &format!("{k}.neg")), "{k}.neg");
        assert_eq!(*abs(&mut out(), &p), tuple(&v, &format!("{k}.abs")), "{k}.abs");
        assert_eq!(sum(&p), v.i64(&format!("{k}.sum")), "{k}.sum");
        assert_eq!(norm1(&p), v.i64(&format!("{k}.norm1")), "{k}.norm1");
        assert_eq!(norm_sq(&p), v.i64(&format!("{k}.norm_sq")), "{k}.norm_sq");
        assert_eq!(max_comp(&p), v.i64(&format!("{k}.max_comp")), "{k}.max_comp");
        assert_eq!(min_comp(&p), v.i64(&format!("{k}.min_comp")), "{k}.min_comp");
    });
    v.each("pvf3.binary", |k| {
        let (a, b) = (tuple(&v, &format!("{k}.v")), tuple(&v, &format!("{k}.w")));
        let pp = |name: &str, got: PVF3| assert_eq!(got, tuple(&v, &format!("{k}.{name}")), "{k}.{name}");
        let thrown = |name: &str, got: Option<PVF3>| {
            let key = format!("{k}.{name}");
            if v.has(&key) {
                assert_eq!(v.s(&key), "throw", "{key}");
                assert!(got.is_none(), "{key}: the TypeScript threw");
            } else {
                assert_eq!(got, Some(tuple(&v, &key)), "{key}");
            }
        };
        pp("and", *and(&mut out(), &a, &b));
        pp("or", *or(&mut out(), &a, &b));
        pp("xor", *xor(&mut out(), &a, &b));
        pp("add", *add(&mut out(), &a, &b));
        pp("sub", *sub(&mut out(), &a, &b));
        pp("mul", *mul(&mut out(), &a, &b));
        thrown("div", caught(|| *div(&mut out(), &a, &b)));
        thrown("mod", caught(|| *r#mod(&mut out(), &a, &b)));
        pp("eq", *eq(&mut out(), &a, &b));
        assert_eq!(equals(&a, &b), v.bool(&format!("{k}.equals")), "{k}.equals");
        pp("lt", *lt(&mut out(), &a, &b));
        pp("gt", *gt(&mut out(), &a, &b));
        pp("max", *max(&mut out(), &a, &b));
        pp("min", *min(&mut out(), &a, &b));
        pp("clamp", *clamp(&mut out(), &a, -5000, 70000));
        assert_eq!(dot(&a, &b), v.i64(&format!("{k}.dot")), "{k}.dot");
        assert_eq!(dist1(&a, &b), v.i64(&format!("{k}.dist1")), "{k}.dist1");
        assert_eq!(dist_sq(&a, &b), v.i64(&format!("{k}.dist_sq")), "{k}.dist_sq");
    });
    v.each("pvf3.gather", |k| {
        let got = *gather(&mut out(), v.i64(&format!("{k}.x")), v.i64(&format!("{k}.y")), v.i64(&format!("{k}.z")));
        assert_eq!(got, tuple(&v, &format!("{k}.p")), "{k}");
    });
    v.each("pvf3.from_real", |k| {
        assert_eq!(from_real(v.f64(&format!("{k}.r"))), v.i64(&format!("{k}.v")), "{k}");
    });
    v.each("pvf3.to_real", |k| {
        assert_eq!(to_real(v.i64(&format!("{k}.c"))).to_bits(), v.f64(&format!("{k}.r")).to_bits(), "{k}");
    });
}
