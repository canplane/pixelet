// xpute-core/ds/typed_alloc.test.rs

use core::cell::RefCell;
use std::rc::Rc;

use super::*;
use crate::ds::arena::{ArenaOptions, Vector};
use crate::ds::buddy::{BuddyAllocator, BuddyOptions};

fn pool() -> Rc<RefCell<BuddyAllocator>> {
    Rc::new(RefCell::new(
        BuddyAllocator::new(BuddyOptions {
            min_block_bytes: Some(1 << 12),
            region_bytes: Some(1 << 20),
            max_idle_regions: Some(1),
            poison: Some(false),
            bypass: Some(false),
            ..Default::default()
        })
        .unwrap(),
    ))
}

#[test]
fn typed_alloc_the_heap_allocator_is_new_ctor_n() {
    let mut heap = HEAP_ALLOC;
    let view = heap.take::<f64>(8);
    assert_eq!(view.length(), 8);
    assert_eq!(view.as_slice(), &[0.0; 8]);
}

#[test]
fn typed_alloc_a_pooled_view_comes_back_zeroed_even_when_its_block_was_written() {
    let mut alloc = buddy_alloc(pool());
    let first = alloc.take::<i32>(1024);
    first.fill(-7);
    alloc.give(&first.view());
    let again = alloc.take::<i32>(1024);
    assert!(again.as_slice().iter().all(|&v| v == 0));
}

#[test]
fn typed_alloc_bigint_lanes_are_zeroed_too() {
    let mut alloc = buddy_alloc(pool());
    let view = alloc.take::<i64>(16);
    assert_eq!(view[15], 0);
}

#[test]
fn typed_alloc_giving_a_view_back_returns_its_block_to_the_pool() {
    let p = pool();
    let mut alloc = buddy_alloc(p.clone());
    let before = p.borrow().stat().live_bytes;
    let view = alloc.take::<f32>(4096);
    assert!(p.borrow().stat().live_bytes > before);
    alloc.give(&view.view());
    assert_eq!(p.borrow().stat().live_bytes, before);
}

#[test]
fn vector_growing_from_a_pool_hands_the_old_storage_back() {
    let p = pool();
    let mut v = Vector::<u32>::new(ArenaOptions {
        init_cap: Some(1024),
        alloc: Some(Box::new(buddy_alloc(p.clone()))),
        ..Default::default()
    });
    for i in 0..5000 {
        v.push(i).unwrap();
    }
    assert_eq!(v.get(4999), 4999);
    assert_eq!(v.get(0), 0);
    assert_eq!(p.borrow().stat().live_bytes, v.cap() * 4, "only the current storage is borrowed");
}

#[test]
fn typed_alloc_a_handle_names_the_storage_and_a_view_made_from_it_frees_the_same_block() {
    let p = pool();
    let mut alloc = buddy_alloc(p.clone());
    let before = p.borrow().stat().live_bytes;
    let mut view = alloc.take::<u16>(3000);
    view[2999] = 41;
    let handle = alloc.handle_of(&view.view());
    let again = alloc.view_of::<u16>(handle, 3000);
    assert_eq!(again[2999], 41);
    alloc.give(&again.view());
    assert_eq!(p.borrow().stat().live_bytes, before);
}

#[test]
fn typed_alloc_handles_hold_across_regions_backed_and_plain() {
    let backing = ArrayBuffer::new(1 << 20);
    let p = Rc::new(RefCell::new(
        BuddyAllocator::new(BuddyOptions {
            min_block_bytes: Some(1 << 12),
            region_bytes: Some(1 << 20),
            max_idle_regions: Some(1),
            poison: Some(false),
            bypass: Some(false),
            backing: Some(backing.clone()),
            ..Default::default()
        })
        .unwrap(),
    ));
    let mut alloc = buddy_alloc(p);
    // A 1 MiB backing cut into 1 MiB regions holds two half-region blocks; the
    // third opens a plain region of its own.
    let backed = alloc.take::<u8>(1 << 19);
    let also_backed = alloc.take::<u8>(1 << 19);
    let plain = alloc.take::<u8>(1 << 19);
    assert!(backed.buffer().is(&backing) && also_backed.buffer().is(&backing));
    assert!(!plain.buffer().is(&backing));
    for mut view in [backed, also_backed, plain] {
        view[7] = 9;
        let back = alloc.view_of::<u8>(alloc.handle_of(&view.view()), view.length() as u32);
        assert_eq!(back.byte_offset(), view.byte_offset());
        assert!(back.buffer().is(view.buffer()));
        assert_eq!(back[7], 9);
    }
}

#[test]
fn buddy_release_by_address_ignores_a_buffer_the_pool_never_lent() {
    let p = pool();
    let mut alloc = buddy_alloc(p.clone());
    alloc.take::<u8>(64);
    let live = p.borrow().stat().live_blocks;
    alloc.give(&TypedArray::<u8>::new(64).view());
    assert_eq!(p.borrow().stat().live_blocks, live);
}
