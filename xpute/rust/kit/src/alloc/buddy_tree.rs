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

//! evanw/buddy-malloc ported line for line: the guest's page allocator, with
//! `slab.rs` over it for anything smaller than a page. Two changes from the C:
//! `update_max_ptr` asks the `Range` instead of `brk`, and there is no 8-byte
//! header, since every caller passes the size back to `free`.

use core::cell::UnsafeCell;
use core::marker::PhantomData;
use core::ptr;

/// The range this allocator spans. `MAX_LOG2` bounds the address space the
/// tree can describe, not the memory; `end` says how much of it is real.
/// `Split` must be `2^(MAX_LOG2 - MIN_LOG2) / 8` bytes (stable Rust cannot
/// size an array from a trait's constants); the first `malloc` checks it.
pub trait Range {
    const MIN_LOG2: usize;
    const MAX_LOG2: usize;
    type Split: AsRef<[u8]> + AsMut<[u8]> + Copy;
    const SPLIT_INIT: Self::Split;
    fn base() -> usize;
    fn end() -> usize;
}

/// Enough free list heads for any pair of exponents in a 32-bit address space.
const MAX_BUCKETS: usize = 33;

/// A circular free list threaded through the free blocks, so the smallest
/// block must hold one.
#[repr(C)]
#[derive(Clone, Copy)]
struct List {
    prev: *mut List,
    next: *mut List,
}

struct Heap<S> {
    /// Bucket 0 is the largest block, the whole address space.
    buckets: [List; MAX_BUCKETS],
    /// The tree starts small and grows: one root the size of the whole space
    /// would reserve half of it on the first split.
    bucket_limit: usize,
    /// Linearized tree: parent `(i - 1) / 2`, children `2i + 1` and `2i + 2`.
    /// Only SPLIT is stored; UNUSED and USED are told apart from context.
    node_is_split: S,
    min_log2: usize,
    max_log2: usize,
    bucket_count: usize,
    base_ptr: usize,
    max_ptr: usize,
    end_ptr: usize,
    /// Not the C's. `releases` moves exactly when a refused request might now
    /// be served.
    live: usize,
    releases: u32,
    allocs: u32,
}

/// Must not move once a block is handed out: the bucket heads are their own
/// lists' sentinels.
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
                bucket_count: 0,
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

    /// Bytes ever reached into, not bytes live.
    pub fn used(&self) -> usize {
        // SAFETY: single-threaded; see the Sync impl.
        let h = unsafe { &*self.heap.get() };
        h.max_ptr - h.base_ptr
    }

    pub fn live(&self) -> usize {
        // SAFETY: single-threaded; see the Sync impl.
        unsafe { (*self.heap.get()).live }
    }

    pub fn range_bytes(&self) -> usize {
        // SAFETY: single-threaded; see the Sync impl.
        let h = unsafe { &*self.heap.get() };
        h.end_ptr - h.base_ptr
    }

    pub fn releases(&self) -> u32 {
        // SAFETY: single-threaded; see the Sync impl.
        unsafe { (*self.heap.get()).releases }
    }

    /// The largest block on a free list, or 0. The untouched range past the
    /// tree's root is not counted.
    pub fn largest_free(&self) -> usize {
        // SAFETY: single-threaded; see the Sync impl.
        let h = unsafe { &*self.heap.get() };
        if h.base_ptr == 0 {
            return 0;
        }
        (h.bucket_limit..h.bucket_count)
            .find(|&b| {
                let list = &h.buckets[b] as *const List;
                // SAFETY: the head is this heap's own field; an empty list
                // points at itself.
                unsafe { !core::ptr::eq((*list).next, list) }
            })
            .map_or(0, |b| 1usize << (h.max_log2 - b))
    }

    pub fn allocs(&self) -> usize {
        // SAFETY: single-threaded; see the Sync impl.
        unsafe { (*self.heap.get()).allocs as usize }
    }

    /// The host reads `base_ptr`, `max_ptr`, `end_ptr`, `live`, `releases`,
    /// `allocs` in place from here, so their order in `Heap` is a contract.
    pub fn stat(&self) -> *const usize {
        // SAFETY: single-threaded; the fields are laid out as the row.
        unsafe { core::ptr::addr_of!((*self.heap.get()).base_ptr) }
    }

    /// Null on failure. A block is aligned to its own size from the base, so a
    /// caller can find its start by masking.
    ///
    /// # Safety
    /// The range `R` names must be this module's, and nothing else may write
    /// a free block's first two words.
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
    /// The C's `brk`, answered by the range.
    fn update_max_ptr(&mut self, new_value: usize) -> bool {
        if new_value > self.max_ptr {
            if new_value > self.end_ptr {
                return false;
            }
            self.max_ptr = new_value;
        }
        true
    }

    /// `index + 1` comes first: the C's `index - (1 << bucket) + 1` underflows
    /// on a bucket's first node and traps in a debug build.
    fn ptr_for_node(&self, index: usize, bucket: usize) -> *mut List {
        (self.base_ptr + ((index + 1 - (1 << bucket)) << (self.max_log2 - bucket))) as *mut List
    }

    fn node_for_ptr(&self, ptr: *mut List, bucket: usize) -> usize {
        ((ptr as usize - self.base_ptr) >> (self.max_log2 - bucket)) + (1 << bucket) - 1
    }

    fn parent_is_split(&self, index: usize) -> bool {
        let index = (index - 1) / 2;
        (self.node_is_split.as_ref()[index / 8] >> (index % 8)) & 1 == 1
    }

    fn flip_parent_is_split(&mut self, index: usize) {
        let index = (index - 1) / 2;
        self.node_is_split.as_mut()[index / 8] ^= 1 << (index % 8);
    }

    fn bucket_for_request(&self, request: usize) -> usize {
        let mut bucket = self.bucket_count - 1;
        let mut size = 1usize << self.min_log2;
        while size < request {
            bucket -= 1;
            size *= 2;
        }
        bucket
    }

    fn list_init(&mut self, bucket: usize) {
        let list = &mut self.buckets[bucket] as *mut List;
        // SAFETY: the head is this heap's own field, live for its life.
        unsafe {
            (*list).prev = list;
            (*list).next = list;
        }
    }

    /// # Safety
    /// `entry` is a free block of at least `size_of::<List>()` bytes.
    unsafe fn list_push(&mut self, bucket: usize, entry: *mut List) {
        let list = &mut self.buckets[bucket] as *mut List;
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
        let list = &mut self.buckets[bucket] as *mut List;
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

    /// Doubles the tree until its root lies at `bucket`.
    fn lower_bucket_limit(&mut self, bucket: usize) -> bool {
        while bucket < self.bucket_limit {
            let root = self.node_for_ptr(self.base_ptr as *mut List, self.bucket_limit);

            // Root UNUSED: the whole space is free, so the new root takes it.
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

            // Root in use: add a SPLIT parent with its right child free.
            let right_child = self.ptr_for_node(root + 1, self.bucket_limit);
            if !self.update_max_ptr(right_child as usize + core::mem::size_of::<List>()) {
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

    fn malloc(&mut self, base: usize, end: usize, min_log2: usize, max_log2: usize, request: usize) -> *mut u8 {
        if self.base_ptr == 0 {
            self.min_log2 = min_log2;
            self.max_log2 = max_log2;
            self.bucket_count = max_log2 - min_log2 + 1;
            crate::ensure!(
                self.bucket_count <= MAX_BUCKETS && self.node_is_split.as_ref().len() * 8 == 1 << (self.bucket_count - 1),
                ENOTRECOVERABLE
            );
            crate::ensure!(1usize << min_log2 >= core::mem::size_of::<List>(), ENOTRECOVERABLE);
            self.base_ptr = base;
            self.max_ptr = base;
            self.end_ptr = end;
            self.bucket_limit = self.bucket_count - 1;
            self.update_max_ptr(self.base_ptr + core::mem::size_of::<List>());
            self.list_init(self.bucket_count - 1);
            let base = self.base_ptr as *mut List;
            // SAFETY: the first block of the range, reserved just above.
            unsafe { self.list_push(self.bucket_count - 1, base) };
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

                // The root is used, so growing one level puts its new right
                // sibling on this bucket's list.
                if !self.lower_bucket_limit(bucket - 1) {
                    return ptr::null_mut();
                }
                ptr = self.list_pop(bucket);
            }

            let size = 1usize << (self.max_log2 - bucket);
            let bytes_needed = if bucket < original_bucket { size / 2 + core::mem::size_of::<List>() } else { size };
            if !self.update_max_ptr(ptr as usize + bytes_needed) {
                // SAFETY: the block just came off this bucket's free list.
                unsafe { self.list_push(bucket, ptr) };
                return ptr::null_mut();
            }

            // The split bit is the XOR of the children's UNUSED flags. The
            // grandparent needs no flip: the buddy is USED.
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
                // by `bytes_needed` above.
                unsafe { self.list_push(bucket, right) };
            }

            self.live += 1 << (self.max_log2 - original_bucket);
            self.allocs = self.allocs.wrapping_add(1);
            return ptr as *mut u8;
        }
    }

    fn free(&mut self, ptr: *mut u8, request: usize) {
        if ptr.is_null() {
            return;
        }

        let ptr = ptr as *mut List;
        let mut bucket = self.bucket_for_request(request);
        self.live -= 1 << (self.max_log2 - bucket);
        self.releases = self.releases.wrapping_add(1);
        let mut i = self.node_for_ptr(ptr, bucket);

        while i != 0 {
            self.flip_parent_is_split(i);

            // SPLIT now means the buddy is USED; the root has no buddy.
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
