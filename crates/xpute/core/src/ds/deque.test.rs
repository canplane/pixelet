// xpute_core/ds/deque.test.rs
// (no pair: deque.ts has no test file)

use super::*;
use crate::sys::vector::Vector;

const VECTOR: &str = include_str!("../../vector/ds/deque.tsv");

#[test]
fn deque_holds_what_the_typescript_holds() {
    let v = Vector::load(VECTOR);
    let mut buf: [Option<u32>; 8] = [None; 8];
    let mut d = Deque::new(&mut buf, 0, 0);
    v.each("", |k| {
        let step: u32 = k.parse().unwrap();
        let r = v.u32(&format!("{k}.r"));
        let mut popped = "none".to_string();
        if r == 0 && !d.full() {
            d.push_back(step);
        } else if r == 1 && !d.full() {
            d.push_front(step);
        } else if r == 2 && !d.empty() {
            popped = d.pop_front().to_string();
        } else if r == 3 && !d.empty() {
            popped = d.pop_back().to_string();
        }
        assert_eq!(popped, v.s(&format!("{k}.popped")), "{k}.popped");
        assert_eq!((d.head, d.len), (v.u32(&format!("{k}.head")), v.u32(&format!("{k}.len"))), "{k}");
        assert_eq!(d.front().map_or("none".to_string(), |x| x.to_string()), v.s(&format!("{k}.front")), "{k}.front");
        assert_eq!(d.back().map_or("none".to_string(), |x| x.to_string()), v.s(&format!("{k}.back")), "{k}.back");
    });
}
