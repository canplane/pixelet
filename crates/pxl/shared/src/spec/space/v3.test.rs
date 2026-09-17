// pxl_shared/spec/space/v3.test.rs
// (no pair: v3.ts has no test file)

use xpute_core::sys::vector::Vector;

use super::*;

const VECTOR: &str = include_str!("../../../vector/spec/space/v3.tsv");

fn vec3(v: &Vector, k: &str) -> V3 {
    [v.f64(&format!("{k}.0")), v.f64(&format!("{k}.1")), v.f64(&format!("{k}.2"))]
}

fn same(got: &V3, want: V3, what: &str) {
    assert_eq!(got.map(f64::to_bits), want.map(f64::to_bits), "{what}: {got:?} vs {want:?}");
}

#[test]
fn v3_computes_what_the_typescript_computes() {
    let v = Vector::load(VECTOR);
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
        assert_eq!(dot3(&a, &b).to_bits(), v.f64(&format!("{k}.dot3")).to_bits(), "{k}.dot3");
        same(cross3(&a, &b, &mut o), vec3(&v, &format!("{k}.cross3")), &format!("{k}.cross3"));
        same(floor3(&a, &b, &mut o), vec3(&v, &format!("{k}.floor3")), &format!("{k}.floor3"));
        same(center3(&a, &b, &mut o), vec3(&v, &format!("{k}.center3")), &format!("{k}.center3"));
        same(ceil3(&a, &b, &mut o), vec3(&v, &format!("{k}.ceil3")), &format!("{k}.ceil3"));
        same(round3(&a, &b, &mut o), vec3(&v, &format!("{k}.round3")), &format!("{k}.round3"));
    });
}
