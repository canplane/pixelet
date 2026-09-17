// xpute_core/ds/heap.test.rs
// (no pair: heap.ts has no test file)

use super::*;
use crate::sys::vector::Vector;

const VECTOR: &str = include_str!("../../vector/ds/heap.tsv");

fn show(e: Option<Element<i32>>) -> String {
    e.map_or("none".to_string(), |e| format!("{}:{}", e.key, e.val))
}

#[test]
fn heaps_pop_what_the_typescript_pops() {
    let v = Vector::load(VECTOR);
    let (mut a, mut b): (Vec<Element<i32>>, Vec<Element<i32>>) = (Vec::new(), Vec::new());
    v.each("steps", |k| {
        if v.s(&format!("{k}.op")) == "pop" {
            assert_eq!(show(MinHeap::new(&mut a).pop()), v.s(&format!("{k}.min")), "{k}.min");
            assert_eq!(show(MaxHeap::new(&mut b).pop()), v.s(&format!("{k}.max")), "{k}.max");
        } else {
            let e = Element {
                key: v.i32(&format!("{k}.key")) as f64,
                val: v.i32(&format!("{k}.val")),
            };
            MinHeap::new(&mut a).push(e);
            MaxHeap::new(&mut b).push(e);
        }
    });
    let mut arr: Vec<Element<i32>> = (0..v.len("input"))
        .map(|j| {
            let (key, val) = v.s(&format!("input.{j}")).split_once(':').unwrap();
            Element {
                key: key.parse().unwrap(),
                val: val.parse().unwrap(),
            }
        })
        .collect();
    heapsort(&mut arr);
    let got: Vec<String> = arr.iter().map(|e| format!("{}:{}", e.key, e.val)).collect();
    let want: Vec<&str> = (0..v.len("sorted")).map(|j| v.s(&format!("sorted.{j}"))).collect();
    assert_eq!(got, want);
}
