// pxl-render/gpu/read.test.rs

use super::*;

fn read_of(r: &mut Read, lane: &[u32]) -> bool {
    r.begin();
    r.read(lane);
    r.end()
}

/// After a dropped frame or a turn put off, what a stage reads next reads as
/// changed, though it is what the stage read before.
#[test]
fn forgotten_reads_as_changed() {
    let mut r = Read::default();
    assert!(!read_of(&mut r, &[1, 2]), "the first read");
    assert!(read_of(&mut r, &[1, 2]), "read again");
    r.forget();
    r.begin();
    assert!(r.read(&[1u32, 2]));
    assert!(!r.same());
    assert!(!r.end(), "read after forgetting");
    assert!(read_of(&mut r, &[1, 2]), "read again after that");
}
