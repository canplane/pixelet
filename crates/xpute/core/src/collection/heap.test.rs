// xpute-core/collection/heap.test.rs
// (no pair: heap.ts has no test file)

use super::*;
use crate::golden::Golden;

const GOLDEN: &str = include_str!("../../golden/collection/heap.tsv");

/// The two heaps' shared element type, named so a pair of them can be read.
type Elements = Vec<Element<f64, i32>>;

fn show(e: Option<Element<f64, i32>>) -> String {
    e.map_or("none".to_string(), |e| format!("{}:{}", e.key, e.val))
}

#[test]
fn heaps_pop_what_the_record_holds() {
    let v = Golden::load(GOLDEN);
    let (mut a, mut b): (Elements, Elements) = (Vec::new(), Vec::new());
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
    let mut arr: Vec<Element<f64, i32>> = (0..v.len("input"))
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
