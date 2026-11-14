// pxl-spec/asset/catalog.test.rs

use super::*;

fn laid(slot_cnt: usize) -> Vec<u8> {
    let mut bytes = (slot_cnt as u32).to_le_bytes().to_vec();
    for s in 0..slot_cnt {
        bytes.extend((s as u32 + 1).to_le_bytes());
    }
    for s in 0..slot_cnt {
        for k in 0..3 {
            bytes.extend((s as f64 + k as f64 / 4.0).to_le_bytes());
        }
    }
    bytes
}

#[test]
fn reads_each_slot() {
    let c = Catalog::read(&laid(SLOT_CNT)).expect("a catalog of the compiled slots");
    let last = SLOT_CNT as Slot - 1;
    assert_eq!(c.file(last), SLOT_CNT as u32);
    assert_eq!(
        c.extent(last),
        [0.0, 0.25, 0.5].map(|k| (SLOT_CNT - 1) as f64 + k)
    );
}

#[test]
fn refuses_another_shape() {
    assert!(Catalog::read(&laid(SLOT_CNT + 1)).is_none());
    let mut short = laid(SLOT_CNT);
    short.pop();
    assert!(Catalog::read(&short).is_none());
}
