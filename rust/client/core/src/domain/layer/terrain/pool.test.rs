// pxl-core/domain/layer/terrain/pool.test.rs

use super::*;

#[test]
fn runs_taken_past_each_other_grow_the_pool_across_chunks() {
    crate::on_core(|| {
        let mut m = WayPool::default();
        assert_eq!(m.take(3), Ok([1, 3]), "word 0 is none's");
        let big = 3 * CHUNK_WORDS;
        assert_eq!(m.take(big), Ok([4, big]), "a run crosses chunks");
        assert_eq!(m.chunks.len(), 4, "grown a chunk at a time to its last");
        assert_eq!(m.top(), 4 + big);
        m.set(CHUNK_WORDS + 9, 77);
        assert_eq!(m.word(CHUNK_WORDS + 9), 77, "indexed across chunks");
        assert_eq!(m.word(10 * WAYS_POOL_WORDS), 0, "past the chunks, nothing");
        let p: u32 = m.pieces([4, big]).iter().map(|(_, w)| w.len() as u32).sum();
        assert_eq!(p, big, "its words in pieces");
    });
}

#[test]
fn run_past_cap_refused() {
    crate::on_core(|| {
        let mut m = WayPool {
            top: WAYS_POOL_WORDS - 1,
            ..WayPool::default()
        };
        assert_eq!(m.take(8), Err(PoolRefusal::Cap), "past the cap");
        assert!(m.chunks.is_empty());
    });
}

#[test]
fn runs_given_back_are_taken_again_joined() {
    crate::on_core(|| {
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
    });
}

#[test]
fn take_passes_over_a_free_run_the_runtime_holds() {
    crate::on_core(|| {
        let mut m = WayPool::default();
        let run = m.take(5).unwrap();
        let _pad = m.take(1).unwrap();
        TEST_HELD.set(run);
        m.give(run);
        assert_eq!(m.take(5), Ok([7, 5]), "held: passed over");
        TEST_HELD.set([0; 2]);
        assert_eq!(m.take(5), Ok([1, 5]), "released: taken");
    });
}

#[test]
fn free_list_holds_every_gap_of_every_brick() {
    crate::on_core(|| {
        let mut m = WayPool::default();
        let runs: Vec<Run> = (0..SLOTS).map(|_| m.take(1).unwrap()).collect();
        // Every other run given back: the most holes held runs can leave.
        for r in runs.iter().step_by(2) {
            m.give(*r);
        }
        assert_eq!(m.free.len(), runs.len().div_ceil(2));
        for r in runs.iter().skip(1).step_by(2) {
            m.give(*r);
        }
        assert_eq!(m.free.len(), 1, "joined whole");
    });
}
