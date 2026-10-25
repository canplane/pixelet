// pxl-kernel/domain/layer/terrain/mirror.test.rs

use super::*;

/// Records of blocks `ids`, block `b`'s heap words `b` and its data words
/// `b + 1`.
fn records(ids: &[u32]) -> Vec<u32> {
    let mut r = vec![ids.len() as u32];
    for &b in ids {
        r.push(b);
        r.extend([b; 8]);
        r.extend([b + 1; 8]);
    }
    r
}

#[test]
fn a_landing_grows_the_mirror_to_its_highest_block_and_reads_back_at_its_slots() {
    let mut m = Mirror::default();
    assert_eq!(m.lay(&records(&[1, 3 * CHUNK_BLOCKS + 5])), Ok(()));
    assert_eq!(m.chunks.len(), 4, "grown a chunk at a time to the highest id laid");
    for b in [1, 3 * CHUNK_BLOCKS + 5] {
        for k in 0..8 {
            assert_eq!((m.heap(8 * b + k), m.data(8 * b + k)), (b, b + 1));
        }
    }
    assert_eq!((m.heap(8 * 2), m.data(8 * 2)), (0, 0), "a block never laid holds nothing");
    assert_eq!(m.heap(8 * 10 * CHUNK_BLOCKS), 0, "past the chunks, nothing");
}

#[test]
fn a_block_past_the_cap_is_refused_and_nothing_laid() {
    let mut m = Mirror::default();
    assert_eq!(m.lay(&records(&[1, MOST_BLOCKS])), Err(Unlaid::Cap), "past the cap, which no asking again changes");
    assert_eq!(m.heap(8), 0);
    assert!(m.chunks.is_empty());
}
