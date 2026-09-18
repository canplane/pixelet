// xpute-core/ds/treap_set.test.rs
// (no pair: treap_set.ts has no test file)

use super::*;
use crate::golden::Golden;
use crate::kit::comp::comp_numeric;

const GOLDEN: &str = include_str!("../../golden/ds/treap_set.tsv");

fn list(v: &Golden, base: &str) -> Vec<f64> {
    if !v.has(&format!("{base}.0")) {
        return Vec::new();
    }
    (0..v.len(base)).map(|j| v.s(&format!("{base}.{j}")).parse().unwrap()).collect()
}

#[test]
fn treap_set_holds_what_the_typescript_holds() {
    let v = Golden::load(GOLDEN);
    let mut t = TreapSet::<f64>::new(Box::new(|a: &f64, b: &f64| comp_numeric(*a, *b)), Some(42));
    v.each("ops", |k| {
        let x = v.u32(&format!("{k}.v")) as f64;
        if v.bool(&format!("{k}.insert")) {
            t.insert(x);
        } else {
            t.delete(&x);
        }
        assert_eq!(t.size(), v.u32(&format!("{k}.size")), "{k}.size");
    });
    let size = t.size();
    for (j, k) in [0, 1, 5, size - 1, size].into_iter().enumerate() {
        assert_eq!(t.kth(k).map_or("none".to_string(), |x| x.to_string()), v.s(&format!("kth.{j}")), "kth({k})");
    }
    assert_eq!(t.iter().copied().collect::<Vec<_>>(), list(&v, "iter"));
    assert_eq!(t.iter_prefix(7).copied().collect::<Vec<_>>(), list(&v, "prefix"));
    assert_eq!(t.iter_from(&140.0, 12).copied().collect::<Vec<_>>(), list(&v, "from"));
    assert_eq!(t.iter_from(&33.0, 5).copied().collect::<Vec<_>>(), list(&v, "from_mid"));
    for (j, x) in [0.0, 33.0, 149.0, 150.0].into_iter().enumerate() {
        assert_eq!(t.has(&x), v.bool(&format!("has.{j}")), "has({x})");
    }
}
