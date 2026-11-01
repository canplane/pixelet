// pxl-spec/asset/catalog.test.rs

use super::*;

fn catalog_bytes(slots: usize) -> Vec<u8> {
    let mut bytes = (slots as u32).to_le_bytes().to_vec();
    for s in 0..slots {
        bytes.extend((s as u32 + 1).to_le_bytes());
    }
    for s in 0..slots {
        for k in 0..3 {
            bytes.extend((s as f64 + k as f64 / 4.0).to_le_bytes());
        }
    }
    bytes
}

#[test]
fn reads_each_slot() {
    let c = Catalog::read(&catalog_bytes(SLOTS)).expect("a catalog of the compiled slots");
    let last = Slot(SLOTS as u16 - 1);
    assert_eq!(c.file(last), SLOTS as u32);
    assert_eq!(
        c.extent(last),
        [0.0, 0.25, 0.5].map(|k| (SLOTS - 1) as f64 + k)
    );
}

#[test]
fn refuses_another_shape() {
    assert!(Catalog::read(&catalog_bytes(SLOTS + 1)).is_none());
    let mut short = catalog_bytes(SLOTS);
    short.pop();
    assert!(Catalog::read(&short).is_none());
}
