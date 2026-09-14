// @xpute/core/ds/map.test.ts

use core::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use super::*;
use crate::ds::buddy::{BuddyAllocator, BuddyOptions};
use crate::ds::typed_alloc::buddy_alloc;

#[test]
fn indexedmap_positions_are_dense_and_stable_until_a_removal() {
    let mut m = IndexedMap::new(I64_KEYS, 4, None);
    assert_eq!(m.insert(10), 0);
    assert_eq!(m.insert(20), 1);
    assert_eq!(m.insert(10), 0, "an existing key keeps its position");
    assert_eq!(m.find(20), 1);
    assert_eq!(m.find(30), -1);
}

#[test]
fn indexedmap_removal_moves_the_last_entry_into_the_hole_and_says_where_it_came_from() {
    let mut m = IndexedMap::new(U32_KEYS, 4, None);
    for k in [1, 2, 3] {
        m.insert(k);
    }
    assert_eq!(m.remove_at(0), 2);
    assert_eq!(m.size(), 2);
    assert_eq!(m.key_at(0), 3);
    assert_eq!(m.find(3), 0);
    assert_eq!(m.find(1), -1);
    assert_eq!(m.remove_at(1), 1, "removing the last moves nothing");
}

fn agrees_with_map<T: KeyTraits>(traits: T, make: impl Fn(u32) -> T::Key)
where
    T::Key: std::hash::Hash + Eq,
{
    let pool = Rc::new(RefCell::new(
        BuddyAllocator::new(BuddyOptions {
            min_block_bytes: Some(1 << 12),
            region_bytes: Some(1 << 20),
            max_idle_regions: Some(1),
            poison: Some(false),
            bypass: Some(false),
            ..Default::default()
        })
        .unwrap(),
    ));
    let mut m = IndexedMap::new(traits, 2, Some(Box::new(buddy_alloc(pool))));
    let mut r: HashMap<T::Key, u32> = HashMap::new();
    let mut live: Vec<T::Key> = Vec::new();
    let mut seed: u32 = 12345;
    let mut rnd = || {
        seed = seed.wrapping_mul(1103515245).wrapping_add(12345) & 0x7fffffff;
        seed
    };
    for _ in 0..5000 {
        if !live.is_empty() && rnd() % 3 == 0 {
            let k = live.remove((rnd() % live.len() as u32) as usize);
            let pos = m.find(k) as u32;
            let from = m.remove_at(pos);
            let moved = *r.iter().find(|(_, &p)| p == from).unwrap().0;
            r.remove(&k);
            if from != pos {
                r.insert(moved, pos);
            }
        } else {
            // Clustered keys, the case linear probing is worst at.
            let k = make(rnd() % 4096);
            if let std::collections::hash_map::Entry::Vacant(e) = r.entry(k) {
                live.push(k);
                e.insert(m.insert(k));
            }
        }
    }
    assert_eq!(m.size() as usize, r.len());
    for (k, pos) in r {
        assert_eq!(m.find(k), pos as i32);
    }
}

#[test]
fn indexedmap_agrees_with_a_map_across_growth_and_many_removals_64_bit_keys() {
    agrees_with_map(I64_KEYS, |n| (n as i64) << 6);
}

#[test]
fn indexedmap_agrees_with_a_map_across_growth_and_many_removals_32_bit_keys() {
    agrees_with_map(U32_KEYS, |n| n << 6);
}

#[test]
fn indexedmap_a_64_bit_key_past_2_63_finds_itself() {
    let mut m = IndexedMap::new(I64_KEYS, 16, None);
    let big = ((1u64 << 63) + 5) as i64;
    let pos = m.insert(big);
    assert_eq!(m.find(big), pos as i32);
}

#[test]
fn indexedmap_a_seeded_walk_visits_every_position_once() {
    let mut m = IndexedMap::new(U32_KEYS, 16, None);
    for k in 0..37 {
        m.insert(k * 3);
    }
    let mut seen: Vec<u32> = m.positions_seeded(99).collect();
    seen.sort();
    assert_eq!(seen, (0..37).collect::<Vec<u32>>());
}

#[test]
fn indexedmap_clear_forgets_every_entry_and_the_next_inserts_start_from_zero() {
    let mut m = IndexedMap::new(U32_KEYS, 4, None);
    for k in 0..40 {
        m.insert(k);
    }
    m.clear();
    assert_eq!(m.size(), 0);
    assert_eq!(m.find(3), -1);
    assert_eq!(m.insert(7), 0);
    assert_eq!(m.insert(3), 1);
    assert_eq!(m.find(7), 0);
}
