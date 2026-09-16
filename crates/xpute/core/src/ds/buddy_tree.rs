// xpute_core/ds/buddy_tree.rs

//! evanw/buddy-malloc, line for line, over a range that is given rather than
//! asked for — the kernel's page allocator, with `ds/slab.rs` over it for
//! everything smaller than a page.
//!
//! A buddy allocator that spans a fixed linear address range with a binary
//! tree tracking free space. `malloc` and `free` are both O(log N) in the
//! maximum number of allocations.
//!
//! Two changes from the C.
//!
//! Where the memory comes from. The C calls `brk` to have the kernel make more
//! of its address range usable, and fails the allocation when the kernel says
//! no. Here `update_max_ptr` asks the `Range` instead, and fails in exactly the
//! same place, which leaves everything built on it — the tree that starts small
//! and doubles, `bucket_limit`, the order the free lists are filled in — as it
//! was. The range is asked every time rather than read once, so how far it
//! reaches is the range's answer and never this file's assumption.
//!
//! The 8-byte header is gone. It exists in the C because `free(void *)` is
//! told an address and nothing else, so the size has to be written beside the
//! block. Every caller here is handed the size back — `GlobalAlloc::dealloc`
//! gets the `Layout` the allocation was made with — so `free` takes it as an
//! argument and a block is nothing but its bytes.
//!
//! Everything else is the original: the linearized tree whose index says both
//! where a node is and how large, one bit a node pair holding the
//! exclusive-or of its children's UNUSED flags, and circular doubly-linked
//! free lists threaded through the free blocks themselves.

use core::cell::UnsafeCell;
use core::marker::PhantomData;
use core::ptr;

/// Where the range this allocator spans lies, read once at the first request:
/// the pair of the C's `sbrk(0)` and of the kernel that answers its `brk`.
pub trait Range {
    fn base() -> usize;
    fn end() -> usize;
}

/// The smallest block, which is the kernel's page: this tree hands out pages
/// and runs of pages, and `ds/slab.rs` cuts the classes below one. The C sets
/// this to 4 because its own `free` needs a header in the block and 16 bytes
/// is the smallest that leaves room for one; nothing here needs a header, and
/// what sets the number instead is the split bitmap below, which is
/// `2^(MAX - MIN) / 8` and so pays for every level of resolution in a static.
pub const MIN_ALLOC_LOG2: usize = 12;
pub const MIN_ALLOC: usize = 1 << MIN_ALLOC_LOG2;

/// The largest block the tree can describe, which is the C's number and for
/// the C's reason: two gigabytes is what a 32-bit address space holds a whole
/// power of two of, so this is the top of the shape rather than a claim about
/// any memory.
///
/// **It is not the size of the range.** The tree begins as a single leaf and
/// doubles only as far as a request makes it (`lower_bucket_limit`); how much
/// of it is real is `update_max_ptr`'s answer, and that comes from the range
/// at runtime. So a smaller heap costs nothing but the levels it never reaches,
/// and the allocator does not have to be told how large the memory it was given
/// is — which is the only reason this constant can sit in a crate that knows
/// nothing about the kernel above it.
const MAX_ALLOC_LOG2: usize = 31;
const MAX_ALLOC: usize = 1 << MAX_ALLOC_LOG2;

/// Allocations are done in powers of two from MIN_ALLOC to MAX_ALLOC
/// inclusive, and each size has a bucket holding its free list. Given a
/// bucket, the size of its allocations is `1 << (MAX_ALLOC_LOG2 - bucket)`.
const BUCKET_COUNT: usize = MAX_ALLOC_LOG2 - MIN_ALLOC_LOG2 + 1;

/// A bit for every node that can be a parent, which is every node above the
/// leaves — we only ever ask about parents. 64 KiB at the exponents above, and
/// the same 64 KiB whatever the device gave, since it is a function of the two
/// and of nothing else.
const SPLIT_BYTES: usize = (1 << (BUCKET_COUNT - 1)) / 8;

/// Free lists are circular doubly-linked lists. Every allocation size has one,
/// threaded through all currently free blocks of that size, so MIN_ALLOC must
/// be at least `size_of::<List>()`.
#[repr(C)]
#[derive(Clone, Copy)]
struct List {
    prev: *mut List,
    next: *mut List,
}

const _: () = assert!(MIN_ALLOC >= core::mem::size_of::<List>(), "a free block must hold its own list links");

struct Heap {
    /// Each bucket corresponds to an allocation size and holds the free list
    /// for that size. Bucket 0 is MAX_ALLOC, the whole address space.
    buckets: [List; BUCKET_COUNT],
    /// The tree starts out small and grows as more memory is used, rather
    /// than starting with one free block the size of the whole space — which
    /// would reserve half of it on the first allocation, the first split
    /// writing a free list entry into the right child of the root.
    bucket_limit: usize,
    /// The linearized binary tree of bits. Given a node's index:
    /// parent `(i - 1) / 2`, left child `i * 2 + 1`, right child `i * 2 + 2`,
    /// sibling `((i - 1) ^ 1) + 1`.
    ///
    /// A node is UNUSED (both children UNUSED), SPLIT (one child UNUSED and
    /// the other not) or USED (neither UNUSED) — two bits, except that
    /// UNUSED and USED are told apart from context, so only SPLIT is stored.
    node_is_split: [u8; SPLIT_BYTES],
    /// The start of the address range. Every allocation is an offset of this
    /// from 0 to MAX_ALLOC.
    base_ptr: usize,
    /// The highest address the allocator has ever used, and the end of what
    /// it may use.
    max_ptr: usize,
    end_ptr: usize,
    /// What the callers read, none of it the C's. `live` and `end_ptr` are
    /// what a pressure test compares; `releases` is what a build refused a
    /// block waits on — it moves when, and only when, a request refused
    /// before might now be served; `allocs` is every request served, moves
    /// included, so a span of work that allocates nothing leaves it alone.
    live: usize,
    releases: u32,
    allocs: u32,
}

/// The allocator, as a `static`: its bucket heads are their own lists'
/// sentinels, so it must not move once a block has been handed out.
pub struct BuddyMalloc<R: Range> {
    heap: UnsafeCell<Heap>,
    range: PhantomData<R>,
}

// SAFETY: one thread — a module instantiated once per memory, which is what
// the imported-memory design guarantees.
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
                buckets: [empty; BUCKET_COUNT],
                bucket_limit: 0,
                node_is_split: [0; SPLIT_BYTES],
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

    /// Bytes of the range that have ever been used: what the allocator has
    /// reached into, not what is live in it.
    pub fn used(&self) -> usize {
        // SAFETY: single-threaded; see the Sync impl.
        let h = unsafe { &*self.heap.get() };
        h.max_ptr - h.base_ptr
    }

    /// Bytes handed out and not yet given back, each at its bucket's size.
    pub fn live(&self) -> usize {
        // SAFETY: single-threaded; see the Sync impl.
        unsafe { (*self.heap.get()).live }
    }

    /// The bytes of the range the allocator was given.
    pub fn range_bytes(&self) -> usize {
        // SAFETY: single-threaded; see the Sync impl.
        let h = unsafe { &*self.heap.get() };
        h.end_ptr - h.base_ptr
    }

    /// Blocks given back. A request refused before might be served once this
    /// has moved, and not before.
    pub fn releases(&self) -> u32 {
        // SAFETY: single-threaded; see the Sync impl.
        unsafe { (*self.heap.get()).releases }
    }

    /// Requests served since the first call, moves included.
    pub fn allocs(&self) -> usize {
        // SAFETY: single-threaded; see the Sync impl.
        unsafe { (*self.heap.get()).allocs as usize }
    }

    /// Where the row lies — `base_ptr`, `max_ptr`, `end_ptr`, `live`, then
    /// `releases` and `allocs` as half-words — for a host to read in place.
    pub fn stat(&self) -> *const usize {
        // SAFETY: single-threaded; the fields are laid out as the row.
        unsafe { core::ptr::addr_of!((*self.heap.get()).base_ptr) }
    }

    /// The address a request of `request` bytes was given, or null. The block
    /// is a power of two of at least MIN_ALLOC bytes, at an offset from the
    /// range's base that is a multiple of its own size — which is what lets a
    /// caller find a block's start again by masking the offset.
    ///
    /// # Safety
    /// The range `R` names must be this module's, and nothing else may write
    /// a free block's first two words.
    pub unsafe fn malloc(&self, request: usize) -> *mut u8 {
        // SAFETY: single-threaded; see the Sync impl.
        unsafe { (*self.heap.get()).malloc(R::base(), R::end(), request) }
    }

    /// Gives back what `malloc` returned, told the same size it asked for.
    ///
    /// # Safety
    /// `ptr` is an address this allocator returned and has not taken back, and
    /// `request` is the size it was asked for.
    pub unsafe fn free(&self, ptr: *mut u8, request: usize) {
        // SAFETY: single-threaded; see the Sync impl.
        unsafe { (*self.heap.get()).free(ptr, request) }
    }
}

impl Heap {
    /// Make sure all addresses before `new_value` are valid and can be used.
    /// The C asks the kernel with `brk`; the range here was reserved before
    /// any of this ran, so what is asked is the range. Returns false where the
    /// memory could not be reserved, which is the same answer in the same
    /// place.
    fn update_max_ptr(&mut self, new_value: usize) -> bool {
        if new_value > self.max_ptr {
            if new_value > self.end_ptr {
                return false;
            }
            self.max_ptr = new_value;
        }
        true
    }

    /// This maps from the index of a node to the address of memory that node
    /// represents. The bucket could be derived from the index with a loop but
    /// is passed so this returns in constant time.
    fn ptr_for_node(&self, index: usize, bucket: usize) -> *mut List {
        (self.base_ptr + ((index - (1 << bucket) + 1) << (MAX_ALLOC_LOG2 - bucket))) as *mut List
    }

    /// This maps from an address of memory to the node that represents it.
    /// Many nodes map to the same address, so the bucket is what makes one of
    /// them unique.
    fn node_for_ptr(&self, ptr: *mut List, bucket: usize) -> usize {
        ((ptr as usize - self.base_ptr) >> (MAX_ALLOC_LOG2 - bucket)) + (1 << bucket) - 1
    }

    /// Given the index of a node, the "is split" flag of its parent.
    fn parent_is_split(&self, index: usize) -> bool {
        let index = (index - 1) / 2;
        (self.node_is_split[index / 8] >> (index % 8)) & 1 == 1
    }

    /// Given the index of a node, flips the "is split" flag of its parent.
    fn flip_parent_is_split(&mut self, index: usize) {
        let index = (index - 1) / 2;
        self.node_is_split[index / 8] ^= 1 << (index % 8);
    }

    /// The index of the smallest bucket that fits `request`.
    fn bucket_for_request(request: usize) -> usize {
        let mut bucket = BUCKET_COUNT - 1;
        let mut size = MIN_ALLOC;
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

    /// Appends the entry to the end of the list. Assumes the entry is not in a
    /// list already, since it overwrites the links.
    ///
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

    /// Removes the entry from whichever list it is in. The list itself needs
    /// no mention: the lists being circular, its own links are updated when
    /// the first or last entry goes.
    ///
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

    /// Removes and returns the last entry in the list, or null when it is
    /// empty — which a circular list says by pointing at itself.
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

    /// The tree is always rooted at the current bucket limit. This grows it by
    /// repeatedly doubling until the root lies at `bucket`, each doubling
    /// lowering the limit by one.
    fn lower_bucket_limit(&mut self, bucket: usize) -> bool {
        while bucket < self.bucket_limit {
            let root = self.node_for_ptr(self.base_ptr as *mut List, self.bucket_limit);

            // If the parent is not SPLIT, the node at the current limit is
            // UNUSED and the whole space is free: clear the root free list,
            // raise the tree, and give the newly-widened space to the new root.
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

            // Otherwise the tree is in use. Make a parent for the current root
            // in the SPLIT state with its right child on the free list, taking
            // the memory for that entry before writing it. The parent's own
            // "is split" flag is already on — that is what was just checked.
            let right_child = self.ptr_for_node(root + 1, self.bucket_limit);
            if !self.update_max_ptr(right_child as usize + core::mem::size_of::<List>()) {
                return false;
            }
            // SAFETY: the right child is a free block, and the bytes for its
            // links were just reserved.
            unsafe { self.list_push(self.bucket_limit, right_child) };
            self.bucket_limit -= 1;
            self.list_init(self.bucket_limit);

            // Set the grandparent's SPLIT flag, so that lowering the limit
            // again knows the root just added is in use.
            let root = (root - 1) / 2;
            if root != 0 {
                self.flip_parent_is_split(root);
            }
        }
        true
    }

    fn malloc(&mut self, base: usize, end: usize, request: usize) -> *mut u8 {
        // Make sure an allocation of this size could succeed at all. The limit
        // is hard-coded because of the way the allocator works.
        if request > MAX_ALLOC {
            return ptr::null_mut();
        }

        // Initialize on the first call. The tree starts as a single node of
        // the smallest possible size; more of the range is taken as needed.
        if self.base_ptr == 0 {
            self.base_ptr = base;
            self.max_ptr = base;
            self.end_ptr = end;
            self.bucket_limit = BUCKET_COUNT - 1;
            self.update_max_ptr(self.base_ptr + core::mem::size_of::<List>());
            self.list_init(BUCKET_COUNT - 1);
            let base = self.base_ptr as *mut List;
            // SAFETY: the first block of the range, reserved just above.
            unsafe { self.list_push(BUCKET_COUNT - 1, base) };
        }

        // The smallest bucket that fits. Whether there is room for it is not
        // checked yet.
        let original_bucket = Self::bucket_for_request(request);
        let mut bucket = original_bucket;

        // Look for a bucket with a non-empty free list at least as large as
        // what is needed, splitting a larger one where there is no exact
        // match.
        loop {
            // The tree may have to grow to fit an allocation of this size.
            if !self.lower_bucket_limit(bucket) {
                return ptr::null_mut();
            }

            let mut ptr = self.list_pop(bucket);
            if ptr.is_null() {
                // Away from the root, or unable to grow the tree further:
                // carry on to the next bucket.
                if bucket != self.bucket_limit || bucket == 0 {
                    if bucket == 0 {
                        return ptr::null_mut();
                    }
                    bucket -= 1;
                    continue;
                }

                // Otherwise grow the tree one more level and pop again. The
                // root is known to be used (the free list was empty), so this
                // adds a parent above it in the SPLIT state with the new right
                // child on this bucket's free list, which is what comes back.
                if !self.lower_bucket_limit(bucket - 1) {
                    return ptr::null_mut();
                }
                ptr = self.list_pop(bucket);
            }

            // Take the address range before going further. Where there is no
            // room, put the block back and fail.
            let size = 1usize << (MAX_ALLOC_LOG2 - bucket);
            let bytes_needed = if bucket < original_bucket { size / 2 + core::mem::size_of::<List>() } else { size };
            if !self.update_max_ptr(ptr as usize + bytes_needed) {
                // SAFETY: the block just came off this bucket's free list.
                unsafe { self.list_push(bucket, ptr) };
                return ptr::null_mut();
            }

            // A node off the free list goes from UNUSED to USED, which flips
            // the parent's "is split" bit — that bit being the exclusive-or of
            // both children's UNUSED flags, and this one just changed.
            //
            // The grandparent never needs the same: the buddy is USED, so the
            // grandparent cannot be UNUSED — had the buddy been UNUSED the
            // parent would never have been split.
            let mut i = self.node_for_ptr(ptr, bucket);
            if i != 0 {
                self.flip_parent_is_split(i);
            }

            // A node larger than needed is split down to size, each step
            // moving to the left child, splitting the parent, and putting the
            // right child on the free list for its bucket.
            while bucket < original_bucket {
                i = i * 2 + 1;
                bucket += 1;
                self.flip_parent_is_split(i);
                let right = self.ptr_for_node(i + 1, bucket);
                // SAFETY: the right child is free and its bytes were reserved
                // by `bytes_needed` above.
                unsafe { self.list_push(bucket, right) };
            }

            self.live += 1 << (MAX_ALLOC_LOG2 - original_bucket);
            self.allocs = self.allocs.wrapping_add(1);
            return ptr as *mut u8;
        }
    }

    fn free(&mut self, ptr: *mut u8, request: usize) {
        if ptr.is_null() {
            return;
        }

        // The block's own address is the one that was handed out, there being
        // no header in front of it.
        let ptr = ptr as *mut List;
        let mut bucket = Self::bucket_for_request(request);
        self.live -= 1 << (MAX_ALLOC_LOG2 - bucket);
        self.releases = self.releases.wrapping_add(1);
        let mut i = self.node_for_ptr(ptr, bucket);

        // Up to the root, flipping USED blocks to UNUSED and merging UNUSED
        // buddies into a single UNUSED parent.
        while i != 0 {
            // USED to UNUSED, which flips the parent's bit for the same reason
            // the other direction did.
            self.flip_parent_is_split(i);

            // A parent that now reads SPLIT means the buddy is USED, so there
            // is nothing to merge with: stop and join this bucket's free list.
            // Stop at the root too — a root has no buddy.
            if self.parent_is_split(i) || bucket == self.bucket_limit {
                break;
            }

            // The buddy is UNUSED, so merge with it and carry on up. It comes
            // off its free list here; the merged parent joins one after the
            // loop.
            let buddy = self.ptr_for_node(((i - 1) ^ 1) + 1, bucket);
            // SAFETY: the buddy is UNUSED, so it is on its bucket's list.
            unsafe { Self::list_remove(buddy) };
            i = (i - 1) / 2;
            bucket -= 1;
        }

        // Join this bucket's free list, at the back — `malloc` takes from the
        // back, so a free followed by a malloc of the same size ideally hands
        // back the same address.
        let at = self.ptr_for_node(i, bucket);
        // SAFETY: the block is free and at least MIN_ALLOC.
        unsafe { self.list_push(bucket, at) };
    }
}
