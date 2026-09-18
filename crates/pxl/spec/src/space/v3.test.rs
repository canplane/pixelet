// pxl-spec/space/v3.test.rs
// (no pair: v3.ts has no test file)

use xpute_core::golden::Golden;

use super::*;

const GOLDEN: &str = include_str!("../../golden/spec/space/v3.tsv");

fn vec3(v: &Golden, k: &str) -> V3 {
    [v.f64(&format!("{k}.0")), v.f64(&format!("{k}.1")), v.f64(&format!("{k}.2"))]
}

/// Bitwise, so that 0.0 and -0.0 are told apart — except between two NaNs,
/// whose sign and payload IEEE 754 leaves to the implementation and which do
/// differ by architecture: `0.0 / 0.0` is the negative quiet NaN on x86-64 and
/// the positive one on aarch64 and in JavaScript. A vector records the machine
/// that wrote it, so comparing a NaN's bits compares machines rather than
/// implementations. xpute-core's `fdlibm.test.rs` already draws the line here.
fn same_f64(got: f64, want: f64) -> bool {
    got.to_bits() == want.to_bits() || (got.is_nan() && want.is_nan())
}

fn same(got: &V3, want: V3, what: &str) {
    let ok = got.iter().zip(want.iter()).all(|(g, w)| same_f64(*g, *w));
    assert!(ok, "{what}: {got:?} vs {want:?} ({:016x?} vs {:016x?})", got.map(f64::to_bits), want.map(f64::to_bits));
}

#[test]
fn v3_computes_what_the_typescript_computes() {
    let v = Golden::load(GOLDEN);
    v.each("unary", |k| {
        let a = vec3(&v, &format!("{k}.v"));
        let mut o = [0.0; 3];
        same(neg3(&a, &mut o), vec3(&v, &format!("{k}.neg3")), &format!("{k}.neg3"));
        same(abs3(&a, &mut o), vec3(&v, &format!("{k}.abs3")), &format!("{k}.abs3"));
        same(norm3(&a, &mut o), vec3(&v, &format!("{k}.norm3")), &format!("{k}.norm3"));
        same(i3(&a, &mut o), vec3(&v, &format!("{k}.i3")), &format!("{k}.i3"));
        same(snap3n(&a, 7, &mut o), vec3(&v, &format!("{k}.snap3n")), &format!("{k}.snap3n"));
        same(add3n(&a, 0.25, &mut o), vec3(&v, &format!("{k}.add3n")), &format!("{k}.add3n"));
        same(sub3n(&a, 0.25, &mut o), vec3(&v, &format!("{k}.sub3n")), &format!("{k}.sub3n"));
        same(mul3n(&a, -3.0, &mut o), vec3(&v, &format!("{k}.mul3n")), &format!("{k}.mul3n"));
        same(div3n(&a, 7.0, &mut o), vec3(&v, &format!("{k}.div3n")), &format!("{k}.div3n"));
        same(mod3n(&a, 2.0, &mut o), vec3(&v, &format!("{k}.mod3n")), &format!("{k}.mod3n"));
        same(floor3n(&a, 2.0, &mut o), vec3(&v, &format!("{k}.floor3n")), &format!("{k}.floor3n"));
        same(center3n(&a, 2.0, &mut o), vec3(&v, &format!("{k}.center3n")), &format!("{k}.center3n"));
        same(ceil3n(&a, 2.0, &mut o), vec3(&v, &format!("{k}.ceil3n")), &format!("{k}.ceil3n"));
        same(round3n(&a, 1.0, &mut o), vec3(&v, &format!("{k}.round3n")), &format!("{k}.round3n"));
    });
    v.each("binary", |k| {
        let (a, b) = (vec3(&v, &format!("{k}.a")), vec3(&v, &format!("{k}.b")));
        let mut o = [0.0; 3];
        same(add3(&a, &b, &mut o), vec3(&v, &format!("{k}.add3")), &format!("{k}.add3"));
        same(sub3(&a, &b, &mut o), vec3(&v, &format!("{k}.sub3")), &format!("{k}.sub3"));
        same(mul3(&a, &b, &mut o), vec3(&v, &format!("{k}.mul3")), &format!("{k}.mul3"));
        same(div3(&a, &b, &mut o), vec3(&v, &format!("{k}.div3")), &format!("{k}.div3"));
        same(mod3(&a, &b, &mut o), vec3(&v, &format!("{k}.mod3")), &format!("{k}.mod3"));
        let (got, want) = (dot3(&a, &b), v.f64(&format!("{k}.dot3")));
        assert!(same_f64(got, want), "{k}.dot3: {got} vs {want}");
        same(cross3(&a, &b, &mut o), vec3(&v, &format!("{k}.cross3")), &format!("{k}.cross3"));
        same(floor3(&a, &b, &mut o), vec3(&v, &format!("{k}.floor3")), &format!("{k}.floor3"));
        same(center3(&a, &b, &mut o), vec3(&v, &format!("{k}.center3")), &format!("{k}.center3"));
        same(ceil3(&a, &b, &mut o), vec3(&v, &format!("{k}.ceil3")), &format!("{k}.ceil3"));
        same(round3(&a, &b, &mut o), vec3(&v, &format!("{k}.round3")), &format!("{k}.round3"));
    });
}
