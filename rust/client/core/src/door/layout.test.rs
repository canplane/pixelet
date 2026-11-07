// pxl-core/door/layout.test.rs

use super::*;
use crate::domain::layer::terrain::fixed;

#[test]
fn machine_json_matches_layout() {
    let note = include_str!(concat!(env!("PXL_REPO"), "/rust/client/core/machine.json"));
    let mib = |bytes: u64| format!("{:.2} MiB", bytes as f64 / f64::from(1u32 << 20));
    let figures = [
        ("the fixed ranges", u64::from(FIXED_BYTES)),
        ("the terrain's range", u64::from(fixed::TERRAIN_BYTES)),
        ("the heap's bounded flows", HEAP_FLOWS_BYTES),
    ];
    for (what, bytes) in figures {
        assert!(
            note.contains(&mib(bytes)),
            "machine.json's mem note states {what} at {}",
            mib(bytes)
        );
    }
}
