// pxl-core/domain/layer/terrain/pool.test.rs

use super::*;

#[test]
fn runs_taken_past_each_other_grow_the_pool() {
    let mut m = WayPool::default();
    assert_eq!(m.take(3), Ok([1, 3]), "child group 0 is none's");
    assert_eq!(
        m.take(3 * CHUNK_CHILD_GROUPS),
        Ok([4, 3 * CHUNK_CHILD_GROUPS])
    );
    assert_eq!(m.chunks.len(), 4, "grown a chunk at a time to its last");
    assert_eq!(m.top(), 4 + 3 * CHUNK_CHILD_GROUPS);
    m.set_first_child(8 * 2, 7);
    m.set_data(8 * 2 + 1, 9);
    assert_eq!((m.first_child(8 * 2), m.data(8 * 2 + 1)), (7, 9));
    assert_eq!(
        m.first_child(8 * 10 * CHUNK_CHILD_GROUPS),
        0,
        "past the chunks, nothing"
    );
}

#[test]
fn run_past_cap_refused() {
    let mut m = WayPool::default();
    assert_eq!(
        m.take(way_tree::POOL_CHILD_GROUPS),
        Err(PoolRefusal::Cap),
        "past the cap, which no asking again changes"
    );
    assert!(m.chunks.is_empty());
}

#[test]
fn runs_given_back_are_taken_again_joined() {
    let mut m = WayPool::default();
    let runs = [m.take(4), m.take(4), m.take(4), m.take(4)].map(Result::unwrap);
    m.give(runs[2]);
    m.give(runs[0]);
    assert_eq!(m.take(8), Ok([17, 8]), "apart, neither holds eight");
    m.give(runs[1]);
    assert_eq!(m.free.len(), 1, "joined, what touched");
    assert_eq!(m.take(10), Ok([1, 10]));
    assert_eq!(m.take(2), Ok([11, 2]), "the lowest that holds it");
    assert!(m.free.is_empty());
}

#[test]
fn run_lies_in_a_piece_a_chunk() {
    let mut m = WayPool::default();
    let run = m.take(2 * CHUNK_CHILD_GROUPS).unwrap();
    let p = m.pieces(run);
    let groups: Vec<_> = p
        .iter()
        .map(|(g, w)| (*g, w.len() as u32 / CHILD_GROUP_WORDS))
        .collect();
    assert_eq!(
        groups,
        [
            (1, CHUNK_CHILD_GROUPS - 1),
            (CHUNK_CHILD_GROUPS, CHUNK_CHILD_GROUPS),
            (2 * CHUNK_CHILD_GROUPS, 1)
        ]
    );
}
