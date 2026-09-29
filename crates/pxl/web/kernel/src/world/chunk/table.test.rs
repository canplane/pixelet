// pxl-kernel/world/chunk/table.test.rs
//
// The slots' chains (`claim`, `let_go`): a claim is k free slots of its
// pool or none, each chained once, with no cycle, all held by the one
// claimant; letting a chain go frees exactly its slots, so what is free
// before a claim is free again once it is let go — among other holders'
// slots, scattered, and past what a pool has.

use super::*;
use crate::world::chunk::mesh_slot::Pool;

fn meshes() -> Meshes {
    Meshes {
        holder: vec![None; OCCUPANCY],
        next: vec![None; OCCUPANCY],
        landing: None,
        epoch: 0,
        chained: 0,
        needs: 0,
        fine_peak: 0,
        coarse_peak: 0,
        waits: 0,
        wait_levels: 0,
    }
}

/// The chain from `first`, in order: none past as many slots as there are,
/// which only a cycle would reach.
fn chain(m: &Meshes, first: usize) -> Option<Vec<usize>> {
    let mut out = vec![first];
    while let Some(next) = m.next[*out.last().unwrap()] {
        if out.len() > OCCUPANCY {
            return None;
        }
        out.push(next);
    }
    Some(out)
}

#[test]
fn a_claim_is_all_of_its_slots_or_none_and_letting_it_go_frees_them_all() {
    let every = |_: (Chunk, u32)| true;
    let mut m = meshes();
    let other = (Chunk::at(1, [9, 9, 0]), 7);
    let pool = Pool::Coarse;
    // Another's slots, every third of the pool, so what is free is scattered.
    for at in pool.range().step_by(3) {
        m.holder[at] = Some(other);
    }
    let free = claimable(&m, &every, pool);
    let us = (Chunk::at(1, [0, 0, 0]), 1);
    for k in [1, 2, 5, 40] {
        let first = claim(&mut m, &every, pool, k, us).expect("room for it");
        let c = chain(&m, first).expect("a chain with a cycle");
        assert_eq!(c.len() as u32, k, "a chain of {k} holds {}", c.len());
        let mut seen = c.clone();
        seen.sort_unstable();
        seen.dedup();
        assert_eq!(seen.len(), c.len(), "a slot twice in one chain");
        assert!(
            c.iter().all(|&at| m.holder[at] == Some(us) && pool.range().contains(&at)),
            "a slot of the chain not ours or not the pool's"
        );
        assert_eq!(claimable(&m, &every, pool), free - k, "a claim of {k} did not take {k}");
        let_go(&mut m, first, us.1);
        assert_eq!(claimable(&m, &every, pool), free, "letting a chain of {k} go did not free it all");
        assert!(c.iter().all(|&at| m.holder[at].is_none() && m.next[at].is_none()), "a slot of the chain left held or linked");
    }
    // More than the pool has free takes none of them.
    assert!(claim(&mut m, &every, pool, free + 1, us).is_none());
    assert_eq!(claimable(&m, &every, pool), free, "a claim refused took some slots");
    // What another holds is theirs whatever is let go.
    assert!(pool.range().step_by(3).all(|at| m.holder[at] == Some(other)));
}
