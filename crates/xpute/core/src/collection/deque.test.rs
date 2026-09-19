// xpute-core/collection/deque.test.rs
// (no pair: deque.ts has no test file)

use super::*;
use crate::golden::Golden;

const GOLDEN: &str = include_str!("../../golden/collection/deque.tsv");

#[test]
fn deque_holds_what_the_record_holds() {
    let v = Golden::load(GOLDEN);
    let mut buf: [Option<u32>; 8] = [None; 8];
    let mut d = Deque::new(&mut buf, 0, 0);
    v.each("", |k| {
        let step: u32 = k.parse().unwrap();
        let r = v.u32(&format!("{k}.r"));
        let mut popped = "none".to_string();
        if r == 0 && !d.is_full() {
            d.push_back(step);
        } else if r == 1 && !d.is_full() {
            d.push_front(step);
        } else if r == 2 {
            popped = d.pop_front().map_or("none".to_string(), |x| x.to_string());
        } else if r == 3 {
            popped = d.pop_back().map_or("none".to_string(), |x| x.to_string());
        }
        assert_eq!(popped, v.s(&format!("{k}.popped")), "{k}.popped");
        assert_eq!((d.head, d.len), (v.u32(&format!("{k}.head")), v.u32(&format!("{k}.len"))), "{k}");
        assert_eq!(d.front().map_or("none".to_string(), |x| x.to_string()), v.s(&format!("{k}.front")), "{k}.front");
        assert_eq!(d.back().map_or("none".to_string(), |x| x.to_string()), v.s(&format!("{k}.back")), "{k}.back");
    });
}
