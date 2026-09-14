// xpute_core/ds/buddy.test.rs

use super::*;

/// 4KB blocks in a 64KB region: 16 leaves, 5 orders. Small enough to reason
/// about exactly, which is the point — buddy's failure modes are arithmetic.
fn pool(overrides: BuddyOptions) -> BuddyAllocator {
    BuddyAllocator::new(BuddyOptions {
        min_block_bytes: Some(1 << 12),
        region_bytes: Some(1 << 16),
        ..overrides
    })
    .unwrap()
}

#[test]
fn pool_a_released_block_s_exact_span_is_handed_to_the_next_request_of_that_order() {
    let mut p = pool(BuddyOptions::default());
    let mut first = p.acquire(1 << 12).unwrap();
    let span = first.byte_offset;
    p.release(&mut first);

    let second = p.acquire(1 << 12).unwrap();
    assert_eq!(second.byte_offset, span);
    assert_eq!(p.stat().regions, 1);
}

#[test]
fn pool_blocks_never_overlap_while_they_are_all_live() {
    let mut p = pool(BuddyOptions::default());
    let live: Vec<Block> = (0..16).map(|_| p.acquire(1 << 12).unwrap()).collect();
    let mut ranges: Vec<(u32, u32)> = live.iter().map(|b| (b.byte_offset, b.byte_offset + (1 << 12))).collect();
    ranges.sort();
    for i in 1..ranges.len() {
        assert!(ranges[i].0 >= ranges[i - 1].1, "overlap at {i}");
    }
    assert_eq!(p.stat().regions, 1); // exactly filled it
    assert_eq!(p.stat().live_blocks, 16);
}

/// The whole reason this is a buddy allocator rather than per-class free
/// lists: a workload that frees one order and immediately asks for another
/// (a LOD tier change) must be able to move bytes between orders in both
/// directions. Splitting covers the shrink, merging covers the grow back.
#[test]
fn pool_a_freed_large_block_splits_to_serve_smaller_ones_and_merges_back() {
    let mut p = pool(BuddyOptions::default());
    let mut big = p.acquire(1 << 16).unwrap(); // the whole region, one block
    assert_eq!(p.stat().regions, 1);
    p.release(&mut big);

    let mut small: Vec<Block> = (0..16).map(|_| p.acquire(1 << 12).unwrap()).collect();
    assert_eq!(p.stat().regions, 1); // split out of what was freed, never grown
    assert!(p.stat().splits > 0);

    for b in small.iter_mut() {
        p.release(b);
    }
    assert!(p.stat().merges > 0);

    // ...and the merged space serves the original size again.
    let mut again = p.acquire(1 << 16).unwrap();
    assert_eq!(p.stat().regions, 1);
    assert_eq!(again.byte_offset, 0);
    p.release(&mut again);
}

#[test]
fn pool_a_request_larger_than_a_region_gets_its_own_buffer_and_is_never_pooled() {
    let mut p = pool(BuddyOptions::default());
    let mut huge = p.acquire((1 << 16) + 1).unwrap();
    assert_eq!(p.stat().spills, 1);
    assert_eq!(p.stat().regions, 0);
    assert_eq!(huge.byte_offset, 0);
    p.release(&mut huge);
    assert_eq!(p.stat().regions, 0);
}

#[test]
fn pool_empty_regions_past_the_idle_ceiling_go_back_to_the_collector() {
    let mut p = pool(BuddyOptions {
        max_idle_regions: Some(1),
        ..Default::default()
    });
    let mut blocks: Vec<Block> = (0..48).map(|_| p.acquire(1 << 12).unwrap()).collect(); // 3 regions
    assert_eq!(p.stat().regions, 3);
    for b in blocks.iter_mut() {
        p.release(b);
    }
    assert_eq!(p.stat().regions, 1);
    assert_eq!(p.stat().evictions, 2);
}

#[test]
fn pool_a_double_release_is_refused_rather_than_corrupting_the_free_lists() {
    let mut p = pool(BuddyOptions::default());
    let mut b = p.acquire(1 << 12).unwrap();
    p.release(&mut b);
    p.release(&mut b);
    assert_eq!(p.stat().live_blocks, 0);
    // The span is handed out once, not twice.
    let x = p.acquire(1 << 12).unwrap();
    let y = p.acquire(1 << 12).unwrap();
    assert!(x.byte_offset != y.byte_offset);
}

#[test]
fn pool_bypass_hands_every_request_its_own_buffer_and_reuses_nothing() {
    let mut p = pool(BuddyOptions {
        bypass: Some(true),
        ..Default::default()
    });
    let mut first = p.acquire(1 << 12).unwrap();
    p.release(&mut first);
    let second = p.acquire(1 << 12).unwrap();
    assert!(!first.buf.is(&second.buf));
    assert_eq!(p.stat().regions, 0);
    assert_eq!(p.stat().spills, 2);
}

// ---- backing ----

const REGION: u32 = 1 << 16;

fn backed(regions: u32, overrides: BuddyOptions) -> (ArrayBuffer, BuddyAllocator) {
    let backing = ArrayBuffer::new((REGION * regions) as usize);
    let p = pool(BuddyOptions {
        backing: Some(backing.clone()),
        max_idle_regions: Some(0),
        ..overrides
    });
    (backing, p)
}

#[test]
fn pool_regions_come_out_of_the_backing_first_at_absolute_offsets() {
    let (backing, mut p) = backed(2, BuddyOptions::default());
    let a = p.acquire(REGION).unwrap();
    let b = p.acquire(REGION).unwrap();
    assert!(a.buf.is(&backing) && b.buf.is(&backing), "both regions carved from the backing");
    let mut offsets = [a.byte_offset, b.byte_offset];
    offsets.sort();
    assert_eq!(offsets, [0, REGION]);

    // Writing through one block must not show up in the other.
    TypedArray::<u8>::over(a.buf.clone(), a.byte_offset as usize, a.bytes as usize).fill(1);
    TypedArray::<u8>::over(b.buf.clone(), b.byte_offset as usize, b.bytes as usize).fill(2);
    assert!(TypedArray::<u8>::over(a.buf.clone(), a.byte_offset as usize, a.bytes as usize).as_slice().iter().all(|&v| v == 1));
}

#[test]
fn pool_a_block_in_a_backed_region_past_the_first_round_trips_through_release() {
    // The second region's base is not zero, so release has to subtract it
    // before the buddy arithmetic, or it frees the wrong span.
    let (_, mut p) = backed(2, BuddyOptions::default());
    let mut first = p.acquire(REGION).unwrap();
    let mut small = p.acquire(1 << 12).unwrap();
    assert!(small.byte_offset >= REGION);
    p.release(&mut small);
    let mut again = p.acquire(1 << 12).unwrap();
    assert_eq!(again.byte_offset, small.byte_offset);
    p.release(&mut again);
    p.release(&mut first);
}

#[test]
fn pool_past_the_backing_it_falls_back_to_plain_regions_and_only_those_are_reclaimed() {
    let (backing, mut p) = backed(1, BuddyOptions::default());
    let mut a = p.acquire(REGION).unwrap();
    let mut b = p.acquire(REGION).unwrap();
    assert!(a.buf.is(&backing), "first region is the backing");
    assert!(!b.buf.is(&backing), "second region is a plain buffer");
    assert_eq!(p.stat().regions, 2);

    p.release(&mut a);
    p.release(&mut b);
    // max_idle_regions is 0: the plain region goes, the backed one stays.
    assert_eq!(p.stat().regions, 1);
    p.clear();
    assert_eq!(p.stat().regions, 1);

    // And the backed region is what serves the next request.
    let c = p.acquire(1 << 12).unwrap();
    assert!(c.buf.is(&backing), "reused from the backing");
}

#[test]
fn pool_a_backing_smaller_than_a_region_is_not_used_at_all() {
    let backing = ArrayBuffer::new((REGION - 1) as usize);
    let mut p = pool(BuddyOptions {
        backing: Some(backing.clone()),
        ..Default::default()
    });
    assert!(!p.acquire(1 << 12).unwrap().buf.is(&backing));
}

#[test]
fn pool_two_pools_over_disjoint_ranges_of_one_backing_never_reach_each_other_s_regions_and_report_their_own_use() {
    let backing = ArrayBuffer::new((REGION * 4) as usize);
    let mut a = pool(BuddyOptions {
        backing: Some(backing.clone()),
        backing_offset: Some(0),
        backing_bytes: Some(REGION * 2),
        no_spill: Some(true),
        ..Default::default()
    });
    let mut b = pool(BuddyOptions {
        backing: Some(backing.clone()),
        backing_offset: Some(REGION * 2),
        backing_bytes: Some(REGION * 2),
        no_spill: Some(true),
        ..Default::default()
    });
    let (ba, bb) = (a.acquire(1 << 12).unwrap(), b.acquire(1 << 12).unwrap());
    assert!(ba.buf.is(&backing) && bb.buf.is(&backing));
    assert!(ba.byte_offset < REGION * 2 && bb.byte_offset >= REGION * 2);
    assert_eq!(a.stat().backing_bytes, REGION * 2);
    assert_eq!(a.stat().backing_used, REGION);
    assert_eq!(b.stat().backing_used, REGION);
}

#[test]
fn pool_no_spill_refuses_past_its_range_and_for_a_request_larger_than_a_region_and_never_opens_a_plain_buffer() {
    let backing = ArrayBuffer::new((REGION * 2) as usize);
    let mut p = pool(BuddyOptions {
        backing: Some(backing),
        no_spill: Some(true),
        ..Default::default()
    });
    let mut held = Vec::new();
    while let Some(block) = p.try_acquire(REGION) {
        held.push(block);
    }
    assert_eq!(held.len(), 2);
    assert_eq!(p.stat().refusals, 1);
    assert!(p.try_acquire(REGION * 2).is_none(), "larger than a region");
    assert_eq!(p.stat().spills, 0);
    assert!(p.acquire(REGION).is_err());
    p.release(&mut held[0]);
    assert!(p.try_acquire(REGION).is_some(), "room again once a block is back");
}

#[test]
fn pool_a_pool_that_may_spill_never_returns_null_from_try_acquire() {
    let backing = ArrayBuffer::new(REGION as usize);
    let mut p = pool(BuddyOptions {
        backing: Some(backing),
        ..Default::default()
    });
    assert!(p.try_acquire(REGION).is_some());
    assert!(p.try_acquire(REGION).is_some(), "past the backing it spills to a plain region");
    assert_eq!(p.stat().spills, 0);
    assert_eq!(p.stat().backing_used, REGION);
}

/// script/vector/xpute/core/ds/buddy.ts's output.
const VECTOR: &str = include_str!("../../vector/ds/buddy.tsv");

#[test]
fn buddy_hands_out_what_the_typescript_hands_out() {
    let v = crate::vector::Vector::load(VECTOR);
    let backing = ArrayBuffer::new(1 << 18);
    let mut p = BuddyAllocator::new(BuddyOptions {
        min_block_bytes: Some(1 << 8),
        region_bytes: Some(1 << 16),
        max_idle_regions: Some(1),
        backing: Some(backing.clone()),
        ..Default::default()
    })
    .unwrap();
    let mut live: Vec<Block> = Vec::new();
    v.each("", |k| {
        if v.s(&format!("{k}.op")) == "release" {
            let (at, backed) = (v.u32(&format!("{k}.at")), v.bool(&format!("{k}.backed")));
            let bytes = v.u32(&format!("{k}.bytes"));
            let i = live
                .iter()
                .position(|b| b.byte_offset == at && b.bytes == bytes && b.buf.is(&backing) == backed)
                .unwrap_or_else(|| panic!("{k}: no live block at {at}"));
            let mut b = live.remove(i);
            p.release(&mut b);
            let s = p.stat();
            assert_eq!(
                (s.live_bytes, s.regions, s.merges, s.evictions),
                (
                    v.u32(&format!("{k}.live_bytes")),
                    v.u32(&format!("{k}.regions")),
                    v.u32(&format!("{k}.merges")),
                    v.u32(&format!("{k}.evictions"))
                ),
                "{k}"
            );
        } else {
            let bytes = v.u32(&format!("{k}.bytes"));
            let b = p.acquire(bytes).unwrap();
            assert_eq!(
                (b.byte_offset, b.buf.is(&backing), b.order, b.region),
                (v.u32(&format!("{k}.at")), v.bool(&format!("{k}.backed")), v.i32(&format!("{k}.order")), v.i32(&format!("{k}.region"))),
                "{k}"
            );
            let handle = if b.order >= 0 { p.handle_at(&b.buf, b.byte_offset, bytes).unwrap() as i64 } else { -1 };
            assert_eq!(handle, v.i64(&format!("{k}.handle")), "{k}.handle");
            let s = p.stat();
            assert_eq!(
                (s.live_bytes, s.regions, s.splits, s.spills),
                (
                    v.u32(&format!("{k}.live_bytes")),
                    v.u32(&format!("{k}.regions")),
                    v.u32(&format!("{k}.splits")),
                    v.u32(&format!("{k}.spills"))
                ),
                "{k}"
            );
            live.push(b);
        }
    });
}
