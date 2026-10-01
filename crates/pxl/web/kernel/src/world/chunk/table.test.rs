// pxl-kernel/world/chunk/table.test.rs
//
// The slots' chains (`claim`, `let_go`): a claim is k free slots of its
// pool or none, each chained once, with no cycle, all held by the one
// claimant; letting a chain go frees exactly its slots, so what is free
// before a claim is free again once it is let go — among other holders'
// slots, scattered, and past what a pool has.

use super::*;
use crate::allocations;
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
    let mut m = meshes();
    let other = 7;
    let pool = Pool::Coarse;
    // Another's slots, every third of the pool, so what is free is scattered.
    for at in pool.range().step_by(3) {
        m.holder[at] = Some(other);
    }
    let free = claimable(&m, pool);
    let us = 1;
    for k in [1, 2, 5, 40] {
        let first = claim(&mut m, pool, k, us).expect("room for it");
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
        assert_eq!(claimable(&m, pool), free - k, "a claim of {k} did not take {k}");
        let_go(&mut m, first, us);
        assert_eq!(claimable(&m, pool), free, "letting a chain of {k} go did not free it all");
        assert!(c.iter().all(|&at| m.holder[at].is_none() && m.next[at].is_none()), "a slot of the chain left held or linked");
    }
    // More than the pool has free takes none of them.
    assert!(claim(&mut m, pool, free + 1, us).is_none());
    assert_eq!(claimable(&m, pool), free, "a claim refused took some slots");
    // What another holds is theirs whatever is let go.
    assert!(pool.range().step_by(3).all(|at| m.holder[at] == Some(other)));
}

/// A chunk being built as `Scheduler::add` mounts one: nothing drawn yet.
fn building(chunk: Chunk, id: u32, cube: Cube) -> Slot {
    Slot {
        id,
        chunk,
        role: Role::Building,
        bc: [0.0; 3],
        radius: 0.0,
        order: Order::LAST,
        flags: 0,
        pending_since: -1.0,
        need: None,
        recube: false,
        tiers: Held::EMPTY,
        decor: Held::EMPTY,
        layout_primary: cube,
        layout_generation: 0,
        layout_ready: false,
        decor_whole: false,
        desired: cube,
        refused: None,
        sources: Sources {
            structures: false,
            textured: false,
            objects_revision: 0,
            hide: None,
            carve: None,
            read_from: None,
            beside_from: [None; 8],
        },
        decor_sources: DecorSources {
            revision: 0,
            shown: false,
            hide: None,
        },
    }
}

/// A slot holds what its layout draws and what it has just built, however
/// many tiers land before a layout is applied again — its sources telling
/// it anew, the cube it wants changing — and the mesh slots it holds are
/// those tiers', no more: one built over an older generation's shares its
/// slots, and a cube no longer built gives its own back.
#[test]
fn a_slot_holds_its_layout_and_what_last_landed_whatever_lands_between() {
    let (k, id) = (Chunk::at(1, [0, 0, 0]), 1);
    let (a, b) = (frontier::coarsest(1), GROUND.scale(2));
    assert_ne!(a, b);
    let mut slots = vec![building(k, id, a)];
    let mut place = vec![None; OCCUPANCY];
    place[id as usize] = Some(0);
    let mut m = meshes();
    let landing = |slots: &mut Vec<Slot>, m: &mut Meshes, generation: u32, cube: Cube| {
        slots[0].desired = cube;
        let e = BuildEvent::Tier {
            id,
            generation,
            cube,
            box_cnt: 0,
            heights: Heights::default(),
        };
        land(slots, &place, m, e);
    };
    // The layout: `a` of the first generation, drawn.
    landing(&mut slots, &mut m, 1, a);
    let s = &mut slots[0];
    (s.layout_primary, s.layout_generation, s.layout_ready) = (a, 1, true);
    for generation in 2..40 {
        let cube = if generation % 2 == 0 { b } else { a };
        landing(&mut slots, &mut m, generation, cube);
        let s = &slots[0];
        assert!(s.tier(a, 1).is_some(), "the layout's tier let go");
        assert!(s.tier(cube, generation).is_some(), "what landed is not held");
        assert!(s.tiers.len() <= HELD, "{} tiers held", s.tiers.len());
        let mut own: Vec<usize> = s.tiers.iter().map(|t| t.slot.index).collect();
        own.sort_unstable();
        own.dedup();
        let held = m.holder.iter().filter(|&&h| h == Some(id)).count();
        assert_eq!(held, own.len(), "mesh slots held past the tiers that draw from them");
    }
}

/// A chunk mounted, its tier landed, let go, and another mounted: the mount
/// the first held is free the moment it goes and is the next one's at once,
/// nothing of the first — its build, its mesh slots — is seen by the
/// second, and once the table has been through it, the cycle allocates
/// nothing.
#[test]
fn a_mount_let_go_is_the_next_ones_at_once_and_a_cycle_allocates_nothing() {
    let mut sched = Scheduler::new();
    let (mut volumes, store, mut painter) = (
        pxl_render::mesh::volume::Volumes::new(),
        crate::layer::object::store::Store::new(),
        pxl_render::mesh::paint::object::adapter::Painter::new(),
    );
    let mut world = World {
        volumes: &mut volumes,
        store: &store,
        painter: &mut painter,
    };
    let cube = frontier::coarsest(1);
    let cycle = |sched: &mut Scheduler, world: &mut World, k: Chunk| {
        sched.add(world, k);
        let i = sched.find(&k).expect("mounted");
        let id = sched.slots[i].id;
        assert!(sched.meshes.holder.iter().all(|&h| h != Some(id)), "a mesh slot held under a mount just given");
        let generation = sched.generation(i);
        let e = BuildEvent::Tier {
            id,
            generation,
            cube,
            box_cnt: 0,
            heights: Heights::default(),
        };
        land(&mut sched.slots, &sched.place, &mut sched.meshes, e);
        assert!(sched.slots[i].tier(cube, generation).is_some(), "the tier did not land");
        sched.remove(world, k);
        assert!(sched.find(&k).is_none());
        id
    };
    let (a, b) = (Chunk::at(1, [0, 0, 0]), Chunk::at(1, [5, 3, 0]));
    let first = cycle(&mut sched, &mut world, a);
    assert_eq!(cycle(&mut sched, &mut world, b), first, "the mount let go was not the next one's");
    let (allocated, _) = allocations::during(|| {
        cycle(&mut sched, &mut world, a);
        cycle(&mut sched, &mut world, b);
    });
    assert_eq!(allocated, 0, "a cycle the table has been through allocated");
}
