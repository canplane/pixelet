// xpute-core/ds/lanes.test.rs

use super::*;

#[test]
fn lanes_rows_are_claimed_written_swap_removed_and_the_moved_row_is_reported() {
    let mut t = Lanes::new(
        vec![("id", Kind::U32), ("x", Kind::F64), ("key", Kind::I64)],
        LanesOptions {
            init_cap: Some(2),
            ..Default::default()
        },
    );
    for i in 0..5 {
        let row = t.push().unwrap() as usize;
        t.lane::<u32>("id").put(row, i * 10);
        t.lane::<f64>("x").put(row, i as f64 / 2.0);
        t.lane::<i64>("key").put(row, -(i as i64));
    }
    assert_eq!(t.len(), 5);
    assert!(t.lane::<u32>("id").length() >= 5, "grew past init_cap");
    assert_eq!(&t.lane::<u32>("id").as_slice()[..5], &[0, 10, 20, 30, 40]);

    let from = t.swap_remove(1);
    assert_eq!(from, 4);
    assert_eq!(t.len(), 4);
    assert_eq!(t.lane::<u32>("id")[1], 40);
    assert_eq!(t.lane::<f64>("x")[1], 2.0);
    assert_eq!(t.lane::<i64>("key")[1], -4);

    assert_eq!(t.swap_remove(3), 3);
    assert_eq!(t.len(), 3);
    t.truncate(1);
    assert_eq!(t.len(), 1);
    assert_eq!(t.lane::<u32>("id")[0], 0);
}
