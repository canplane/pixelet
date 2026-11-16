// xpute-kit/alloc/buddy_tree.rs

// Copyright 2018 Evan Wallace
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in
// all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.

//! This is the guest's page allocator, and `slab.rs` serves anything smaller
//! than a page from it. It is a port of evanw/buddy-malloc that follows the C
//! line for line, and departs from it in two places. `update_max_ptr` asks the
//! `Range` for memory where the C calls `brk`. A block carries no 8-byte header
//! with its size, so every caller passes the size it asked for back to `free`.

use core::cell::UnsafeCell;
use core::marker::PhantomData;
use core::ptr;

/// The memory this allocator manages. A block is `2^MIN_LOG2` bytes at the
/// smallest and `2^MAX_LOG2` at the largest. `MAX_LOG2` bounds the address
/// space the tree can describe, not the memory that exists: `end` is where
/// the real memory stops. `Split` holds one bit for each node that can
/// split, so it must be `2^(MAX_LOG2 - MIN_LOG2) / 8` bytes; the first
/// `malloc` checks this.
pub trait Range {
    const MIN_LOG2: usize;
    const MAX_LOG2: usize;
    type Split: AsRef<[u8]> + AsMut<[u8]> + Copy;
    const SPLIT_INIT: Self::Split;
    fn base() -> usize;
    fn end() -> usize;
}

/// One free list head for each block size the address space allows, from
/// exponent 0 through `ADDR_BITS`.
const MAX_BUCKETS: usize = super::ADDR_BITS + 1;

/// A link of a circular free list. The links live inside the free blocks
/// themselves, so the smallest block must be large enough to hold one.
#[repr(C)]
#[derive(Clone, Copy)]
struct List {
    prev: *mut List,
    next: *mut List,
}

/// `repr(C)`, because the host reads the fields from `base_ptr` through
/// `allocs` in place, in this order (`stat`).
#[repr(C)]
struct Heap<S> {
    /// Bucket 0 holds the largest block, which spans the whole address
    /// space; each bucket after it holds blocks half as large.
    buckets: [List; MAX_BUCKETS],
    /// The bucket of the tree's current root. The tree starts with its root
    /// at the smallest block and grows toward bucket 0 as larger blocks are
    /// asked for.
    bucket_limit: usize,
    /// A tree laid out in an array: node `i` has its parent at `(i - 1) / 2`,
    /// its children at `2i + 1` and `2i + 2`, and its buddy at
    /// `((i - 1) ^ 1) + 1`. Each parent keeps one bit, the XOR of its two
    /// children's UNUSED flags. A node's state (UNUSED, SPLIT or USED) is not
    /// stored; the walks tell it from these bits.
    node_is_split: S,
    min_log2: usize,
    max_log2: usize,
    bucket_cnt: usize,
    base_ptr: usize,
    max_ptr: usize,
    end_ptr: usize,
    live: usize,
    /// Changes exactly when a request refused before might now be served, so
    /// a caller can wait for it to change before it retries.
    releases: u32,
    allocs: u32,
}

/// A `BuddyMalloc` must not move once it has handed out a block: each
/// bucket's head is the sentinel of its own circular list, and the free
/// blocks point at it.
pub struct BuddyMalloc<R: Range> {
    heap: UnsafeCell<Heap<R::Split>>,
    range: PhantomData<R>,
}

// SAFETY: one thread; a module is instantiated once per memory.
unsafe impl<R: Range> Sync for BuddyMalloc<R> {}

impl<R: Range> Default for BuddyMalloc<R> {
    fn default() -> BuddyMalloc<R> {
        BuddyMalloc::new()
    }
}

impl<R: Range> BuddyMalloc<R> {
    pub const fn new() -> BuddyMalloc<R> {
        let empty = List {
            prev: ptr::null_mut(),
            next: ptr::null_mut(),
        };
        BuddyMalloc {
            heap: UnsafeCell::new(Heap {
                buckets: [empty; MAX_BUCKETS],
                bucket_limit: 0,
                node_is_split: R::SPLIT_INIT,
                min_log2: 0,
                max_log2: 0,
                bucket_cnt: 0,
                base_ptr: 0,
                max_ptr: 0,
                end_ptr: 0,
                live: 0,
                releases: 0,
                allocs: 0,
            }),
            range: PhantomData,
        }
    }

    /// The most bytes of the range the allocator has ever reached into, not
    /// the bytes live now.
    pub fn peak(&self) -> usize {
        // SAFETY: single-threaded; see the Sync impl.
        let h = unsafe { &*self.heap.get() };
        h.max_ptr - h.base_ptr
    }

    pub fn live(&self) -> usize {
        // SAFETY: single-threaded; see the Sync impl.
        unsafe { (*self.heap.get()).live }
    }

    pub fn range_sz(&self) -> usize {
        // SAFETY: single-threaded; see the Sync impl.
        let h = unsafe { &*self.heap.get() };
        h.end_ptr - h.base_ptr
    }

    pub fn releases(&self) -> u32 {
        // SAFETY: single-threaded; see the Sync impl.
        unsafe { (*self.heap.get()).releases }
    }

    /// `largest_free` answers the size of the largest block on a free list, or
    /// 0. The memory past the tree's current root, which no block covers yet,
    /// is not counted.
    pub fn largest_free(&self) -> usize {
        // SAFETY: single-threaded; see the Sync impl.
        let h = unsafe { &*self.heap.get() };
        if h.base_ptr == 0 {
            return 0;
        }
        (h.bucket_limit..h.bucket_cnt)
            .find(|&b| {
                let list = &raw const h.buckets[b];
                // SAFETY: the head is this heap's own field; an empty list
                // points at itself.
                unsafe { !ptr::eq((*list).next, list) }
            })
            .map_or(0, |b| 1usize << (h.max_log2 - b))
    }

    pub fn allocs(&self) -> usize {
        // SAFETY: single-threaded; see the Sync impl.
        unsafe { (*self.heap.get()).allocs as usize }
    }

    /// Where the host reads `base_ptr`, `max_ptr`, `end_ptr`, `live`,
    /// `releases` and `allocs` in place, one after another. Their order in
    /// `Heap` is part of the contract with the host.
    pub fn stat(&self) -> *const usize {
        // SAFETY: single-threaded; the fields are laid out as the row.
        unsafe { core::ptr::addr_of!((*self.heap.get()).base_ptr) }
    }

    /// Returns null when no block can be had. A block's offset from the
    /// range's base is a multiple of its own size, so a caller can find a
    /// block's start from any address inside it by masking the offset.
    ///
    /// # Safety
    /// The range `R` names must be this module's own memory. Nothing but the
    /// allocator may write the first two words of a free block, which hold
    /// its free list links.
    pub unsafe fn malloc(&self, request: usize) -> *mut u8 {
        // SAFETY: single-threaded; see the Sync impl.
        unsafe { (*self.heap.get()).malloc(R::base(), R::end(), R::MIN_LOG2, R::MAX_LOG2, request) }
    }

    /// # Safety
    /// `ptr` is an address this allocator returned and has not taken back, and
    /// `request` is the size it was asked for.
    pub unsafe fn free(&self, ptr: *mut u8, request: usize) {
        // SAFETY: single-threaded; see the Sync impl.
        unsafe { (*self.heap.get()).free(ptr, request) }
    }
}

impl<S: AsRef<[u8]> + AsMut<[u8]>> Heap<S> {
    /// This stands in for the C's `brk`: the reached memory grows up to the
    /// range's end and is refused past it.
    fn update_max_ptr(&mut self, new_val: usize) -> bool {
        if new_val > self.max_ptr {
            if new_val > self.end_ptr {
                return false;
            }
            self.max_ptr = new_val;
        }
        true
    }

    /// The sum `idx + 1` is taken before the subtraction. The C's order,
    /// `index - (1 << bucket) + 1`, underflows for a bucket's first node,
    /// which traps in a debug build.
    fn ptr_for_node(&self, idx: usize, bucket: usize) -> *mut List {
        (self.base_ptr + ((idx + 1 - (1 << bucket)) << (self.max_log2 - bucket))) as *mut List
    }

    fn node_for_ptr(&self, ptr: *mut List, bucket: usize) -> usize {
        ((ptr as usize - self.base_ptr) >> (self.max_log2 - bucket)) + (1 << bucket) - 1
    }

    fn parent_is_split(&self, idx: usize) -> bool {
        let parent = (idx - 1) / 2;
        (self.node_is_split.as_ref()[parent / 8] >> (parent % 8)) & 1 == 1
    }

    fn flip_parent_is_split(&mut self, idx: usize) {
        let parent = (idx - 1) / 2;
        self.node_is_split.as_mut()[parent / 8] ^= 1 << (parent % 8);
    }

    fn bucket_for_request(&self, request: usize) -> usize {
        let mut bucket = self.bucket_cnt - 1;
        let mut sz = 1usize << self.min_log2;
        while sz < request {
            bucket -= 1;
            sz *= 2;
        }
        bucket
    }

    fn list_init(&mut self, bucket: usize) {
        let list = &raw mut self.buckets[bucket];
        // SAFETY: the head is this heap's own field, live for its life.
        unsafe {
            (*list).prev = list;
            (*list).next = list;
        }
    }

    /// # Safety
    /// `entry` is a free block of at least `size_of::<List>()` bytes.
    unsafe fn list_push(&mut self, bucket: usize, entry: *mut List) {
        let list = &raw mut self.buckets[bucket];
        // SAFETY: the caller's, and the head is live.
        unsafe {
            let prev = (*list).prev;
            (*entry).prev = prev;
            (*entry).next = list;
            (*prev).next = entry;
            (*list).prev = entry;
        }
    }

    /// # Safety
    /// `entry` is currently in one of this heap's lists.
    unsafe fn list_remove(entry: *mut List) {
        // SAFETY: the caller's.
        unsafe {
            let (prev, next) = ((*entry).prev, (*entry).next);
            (*prev).next = next;
            (*next).prev = prev;
        }
    }

    fn list_pop(&mut self, bucket: usize) -> *mut List {
        let list = &raw mut self.buckets[bucket];
        // SAFETY: the head is live and its links are this heap's own.
        unsafe {
            let back = (*list).prev;
            if back == list {
                return ptr::null_mut();
            }
            Self::list_remove(back);
            back
        }
    }

    /// Doubles the tree until its root is at `bucket`. Returns false when the
    /// range has no room for the links of a new free block.
    fn lower_bucket_limit(&mut self, bucket: usize) -> bool {
        while bucket < self.bucket_limit {
            let root = self.node_for_ptr(self.base_ptr as *mut List, self.bucket_limit);

            // The root is UNUSED, so the whole tree is free and the new,
            // larger root becomes one free block.
            if !self.parent_is_split(root) {
                // SAFETY: the base is on the root's list, which is what
                // UNUSED at the limit means.
                unsafe { Self::list_remove(self.base_ptr as *mut List) };
                self.bucket_limit -= 1;
                self.list_init(self.bucket_limit);
                let base = self.base_ptr as *mut List;
                // SAFETY: the base is a free block of the new root's size.
                unsafe { self.list_push(self.bucket_limit, base) };
                continue;
            }

            // The root is in use, so the new root is SPLIT: its left child is
            // the old root and its right child a new free block.
            let right_child = self.ptr_for_node(root + 1, self.bucket_limit);
            if !self.update_max_ptr(right_child as usize + size_of::<List>()) {
                return false;
            }
            // SAFETY: the right child is a free block, and the bytes for its
            // links were just reserved.
            unsafe { self.list_push(self.bucket_limit, right_child) };
            self.bucket_limit -= 1;
            self.list_init(self.bucket_limit);

            let root = (root - 1) / 2;
            if root != 0 {
                self.flip_parent_is_split(root);
            }
        }
        true
    }

    fn malloc(
        &mut self,
        base: usize,
        end: usize,
        min_log2: usize,
        max_log2: usize,
        request: usize,
    ) -> *mut u8 {
        if self.base_ptr == 0 {
            self.min_log2 = min_log2;
            self.max_log2 = max_log2;
            self.bucket_cnt = max_log2 - min_log2 + 1;
            crate::ensure!(
                self.bucket_cnt <= MAX_BUCKETS
                    && self.node_is_split.as_ref().len() * 8 == 1 << (self.bucket_cnt - 1),
                ENOTRECOVERABLE
            );
            crate::ensure!(1usize << min_log2 >= size_of::<List>(), ENOTRECOVERABLE);
            self.base_ptr = base;
            self.max_ptr = base;
            self.end_ptr = end;
            self.bucket_limit = self.bucket_cnt - 1;
            self.update_max_ptr(self.base_ptr + size_of::<List>());
            self.list_init(self.bucket_cnt - 1);
            let base = self.base_ptr as *mut List;
            // SAFETY: the first block of the range, reserved just above.
            unsafe { self.list_push(self.bucket_cnt - 1, base) };
        }

        if request > 1usize << self.max_log2 {
            return ptr::null_mut();
        }

        let original_bucket = self.bucket_for_request(request);
        let mut bucket = original_bucket;

        loop {
            if !self.lower_bucket_limit(bucket) {
                return ptr::null_mut();
            }

            let mut ptr = self.list_pop(bucket);
            if ptr.is_null() {
                if bucket != self.bucket_limit || bucket == 0 {
                    if bucket == 0 {
                        return ptr::null_mut();
                    }
                    bucket -= 1;
                    continue;
                }

                // This bucket is the root's and the root is in use. Growing
                // the tree one level puts the root's new right sibling on
                // this bucket's free list.
                if !self.lower_bucket_limit(bucket - 1) {
                    return ptr::null_mut();
                }
                ptr = self.list_pop(bucket);
            }

            let sz = 1usize << (self.max_log2 - bucket);
            let need_sz = if bucket < original_bucket {
                sz / 2 + size_of::<List>()
            } else {
                sz
            };
            if !self.update_max_ptr(ptr as usize + need_sz) {
                // SAFETY: the block just came off this bucket's free list.
                unsafe { self.list_push(bucket, ptr) };
                return ptr::null_mut();
            }

            // A parent's bit is the XOR of its children's UNUSED flags, so
            // taking this block flips its parent's bit. The grandparent's bit
            // stays: this block's buddy is USED, so the parent was never
            // UNUSED.
            let mut i = self.node_for_ptr(ptr, bucket);
            if i != 0 {
                self.flip_parent_is_split(i);
            }

            while bucket < original_bucket {
                i = i * 2 + 1;
                bucket += 1;
                self.flip_parent_is_split(i);
                let right = self.ptr_for_node(i + 1, bucket);
                // SAFETY: the right child is free and its bytes were reserved
                // by `need_sz` above.
                unsafe { self.list_push(bucket, right) };
            }

            self.live += 1 << (self.max_log2 - original_bucket);
            self.allocs = self.allocs.wrapping_add(1);
            return ptr.cast::<u8>();
        }
    }

    fn free(&mut self, ptr: *mut u8, request: usize) {
        if ptr.is_null() {
            return;
        }

        let ptr = ptr.cast::<List>();
        let mut bucket = self.bucket_for_request(request);
        self.live -= 1 << (self.max_log2 - bucket);
        self.releases = self.releases.wrapping_add(1);
        let mut i = self.node_for_ptr(ptr, bucket);

        while i != 0 {
            self.flip_parent_is_split(i);

            // After the flip, the parent's bit says SPLIT exactly when the
            // buddy is still USED, and the two cannot merge. The tree's
            // current root has no buddy to merge with.
            if self.parent_is_split(i) || bucket == self.bucket_limit {
                break;
            }

            let buddy = self.ptr_for_node(((i - 1) ^ 1) + 1, bucket);
            // SAFETY: the buddy is UNUSED, so it is on its bucket's list.
            unsafe { Self::list_remove(buddy) };
            i = (i - 1) / 2;
            bucket -= 1;
        }

        let at = self.ptr_for_node(i, bucket);
        // SAFETY: the block is free and at least the smallest block.
        unsafe { self.list_push(bucket, at) };
    }
}

#[cfg(test)]
#[path = "buddy_tree.test.rs"]
mod test;
