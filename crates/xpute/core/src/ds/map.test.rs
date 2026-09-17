// xpute-core/ds/map.test.rs

use std::collections::HashMap;

use super::*;

#[test]
fn indexedmap_positions_are_dense_and_stable_until_a_removal() {
    let mut m = IndexedMap::new(I64_KEYS, 4);
    assert_eq!(m.insert(10), 0);
    assert_eq!(m.insert(20), 1);
    assert_eq!(m.insert(10), 0, "an existing key keeps its position");
    assert_eq!(m.find(20), 1);
    assert_eq!(m.find(30), -1);
}

#[test]
fn indexedmap_removal_moves_the_last_entry_into_the_hole_and_says_where_it_came_from() {
    let mut m = IndexedMap::new(U32_KEYS, 4);
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
    let mut m = IndexedMap::new(traits, 2);
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
    let mut m = IndexedMap::new(I64_KEYS, 16);
    let big = ((1u64 << 63) + 5) as i64;
    let pos = m.insert(big);
    assert_eq!(m.find(big), pos as i32);
}

#[test]
fn indexedmap_a_seeded_walk_visits_every_position_once() {
    let mut m = IndexedMap::new(U32_KEYS, 16);
    for k in 0..37 {
        m.insert(k * 3);
    }
    let mut seen: Vec<u32> = m.positions_seeded(99).collect();
    seen.sort();
    assert_eq!(seen, (0..37).collect::<Vec<u32>>());
}

#[test]
fn indexedmap_clear_forgets_every_entry_and_the_next_inserts_start_from_zero() {
    let mut m = IndexedMap::new(U32_KEYS, 4);
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

/// script/vector/xpute/core/ds/map.ts's output.
const VECTOR: &str = include_str!("../../vector/ds/map.tsv");

fn replay<T: KeyTraits>(v: &crate::sys::vector::Vector, base: &str, traits: T, key: impl Fn(&str) -> T::Key)
where
    T::Key: std::fmt::Debug,
{
    let mut m = IndexedMap::new(traits, 2);
    v.each(&format!("{base}.steps"), |k| {
        if v.s(&format!("{k}.op")) == "remove" {
            let pos = v.u32(&format!("{k}.pos"));
            assert_eq!(m.key_at(pos), key(v.s(&format!("{k}.key"))), "{k}.key");
            assert_eq!(m.remove_at(pos), v.u32(&format!("{k}.from")), "{k}.from");
        } else {
            assert_eq!(m.insert(key(v.s(&format!("{k}.key")))), v.u32(&format!("{k}.pos")), "{k}.pos");
        }
        assert_eq!(m.size(), v.u32(&format!("{k}.size")), "{k}.size");
    });
    let walk: Vec<u32> = m.positions_seeded(99).collect();
    let want: Vec<u32> = (0..v.len(&format!("{base}.walk"))).map(|j| v.u32(&format!("{base}.walk.{j}"))).collect();
    assert_eq!(walk, want, "{base}.walk");
    let walk2: Vec<u32> = m.positions_seeded(0xffffffffffff).collect();
    let want2: Vec<u32> = (0..v.len(&format!("{base}.walk2"))).map(|j| v.u32(&format!("{base}.walk2.{j}"))).collect();
    assert_eq!(walk2, want2, "{base}.walk2");
}

#[test]
fn indexedmap_places_and_moves_what_the_typescript_places_and_moves() {
    let v = crate::sys::vector::Vector::load(VECTOR);
    replay(&v, "i64", I64_KEYS, |s| s.parse().unwrap());
    replay(&v, "u32", U32_KEYS, |s| s.parse().unwrap());
}
