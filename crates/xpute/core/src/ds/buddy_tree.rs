// xpute_core/ds/buddy_tree.rs

//! A buddy allocator over one fixed range, as a linearized binary tree.
//!
//! evanw/buddy-malloc in Rust, with the two things that only made sense for a
//! drop-in `malloc` taken out.
//!
//! **No growth.** The original reserves a 2 GB address range and calls `brk`
//! as it reaches into it, growing the tree by doubling — which is why its node
//! indices are relative to a root that moves. Here the range is whole from the
//! first call, so the root is node 0 forever, indices never shift, and
//! `lower_bucket_limit` and `update_max_ptr` have nothing to do.
//!
//! **No header.** The original stores the request size in eight bytes before
//! the address it returns, because `free(ptr)` is told nothing else. `release`
//! here is told the size, so nothing is stored beside a block and a block is
//! exactly its bytes. With a leaf of a page that is not a nicety: a header
//! pushes every power-of-two request into the next bucket, so a 4 MB record
//! would take 8 MB and a 1 MB band store 2 MB.
//!
//! What is kept is the part that is the algorithm:
//!
//! - **The tree is the address space.** A node's index says where it is and
//!   how big it is, so an address maps to a node and back with two shifts, and
//!   there is no table between them: parent `(i-1)/2`, children `2i+1` and
//!   `2i+2`, buddy `((i-1)^1)+1`.
//! - **One bit a node pair.** A node is UNUSED, SPLIT or USED, which is two
//!   bits, but the bit stored is the exclusive-or of its children's UNUSED
//!   flags — and the case that leaves ambiguous is settled by which free list
//!   the caller's block came off. One bit, and the bit is flipped rather than
//!   set, so an allocation and its release are the same operation twice.
//! - **Free lists inside the free blocks.** A block is only linked while it is
//!   free, and a borrower gets its bytes dirty in any case, so the links cost
//!   nothing. They are circular and doubly linked, with the bucket's own node
//!   as the sentinel, so a push or a remove has no branch for the empty case
//!   and none for the ends.
//!
//! The range is `1 << max_log2` bytes and the smallest block `1 << min_log2`.
//! Neither the bitmap nor the lists are allocated: the bitmap is the caller's
//! slice, and the lists live in the memory this tree hands out.

use core::ptr;

/// Buckets a tree can have, which bounds `max_log2 - min_log2`. A range of
/// 4 GB in blocks of 8 bytes is 30, so nothing can ask for more.
pub const MAX_BUCKETS: usize = 32;

/// The links a free block holds, in the block itself. Two words, so the
/// smallest block must be at least this large.
#[repr(C)]
#[derive(Clone, Copy)]
struct List {
    prev: *mut List,
    next: *mut List,
}

/// Bytes of bitmap a tree of these logs needs: a bit for every node that can
/// be a parent, which is every node above the leaves.
pub const fn bitmap_bytes(min_log2: u32, max_log2: u32) -> usize {
    (1usize << (max_log2 - min_log2)) / 8
}

pub struct BuddyTree {
    /// Where the range starts. Every block is this plus an offset under
    /// `1 << max_log2`.
    base: usize,
    min_log2: u32,
    max_log2: u32,
    /// Buckets, largest first: bucket 0 is the whole range and bucket
    /// `buckets - 1` the smallest block. Each is the sentinel of its own
    /// circular list, so its address is part of the list and must not move.
    heads: [List; MAX_BUCKETS],
    buckets: usize,
    /// A bit per node pair, the caller's. `bitmap_bytes` says how long.
    split: *mut u8,
    /// Whether `open` has run. The lists cannot be built before the value has
    /// stopped moving, so the first call builds them.
    opened: bool,
}

impl BuddyTree {
    /// A tree over `1 << max_log2` bytes at `base`, in blocks of at least
    /// `1 << min_log2`, with `split` as its bitmap.
    ///
    /// Nothing is read or written until the first `acquire`: the bucket heads
    /// are the sentinels of their own lists, so they cannot be linked while
    /// the value is still being moved into place.
    ///
    /// # Safety
    /// `base` addresses `1 << max_log2` bytes that stay valid and unmoved for
    /// the tree's life and that nothing else writes, `split` addresses
    /// `bitmap_bytes(min_log2, max_log2)` bytes likewise, and the tree itself
    /// must not move once a block has been handed out.
    pub const unsafe fn new(base: usize, min_log2: u32, max_log2: u32, split: *mut u8) -> BuddyTree {
        let empty = List {
            prev: ptr::null_mut(),
            next: ptr::null_mut(),
        };
        BuddyTree {
            base,
            min_log2,
            max_log2,
            heads: [empty; MAX_BUCKETS],
            buckets: (max_log2 - min_log2 + 1) as usize,
            split,
            opened: false,
        }
    }

    /// The largest block the tree can hand out, which is its whole range.
    pub fn range_bytes(&self) -> usize {
        1usize << self.max_log2
    }

    /// Borrows at least `bytes`: where the block starts, or none when no
    /// block that large is free. The bytes are whatever the last borrower
    /// left; a caller that reads what it did not write must zero them.
    pub fn acquire(&mut self, bytes: u32) -> Option<usize> {
        if bytes == 0 || bytes as usize > self.range_bytes() {
            return None;
        }
        self.open();
        let want = self.bucket_of(bytes);

        // The smallest free block that fits, splitting down to `want`: each
        // step takes the left child and puts the right one on its list.
        let mut bucket = want;
        loop {
            if let Some(at) = self.pop(bucket) {
                let mut i = self.node_at(at, bucket);
                // UNUSED to USED: the parent's bit is the exclusive-or of its
                // children's UNUSED flags and one of them just changed. The
                // grandparent cannot need the same, since a buddy that was
                // UNUSED would have kept the parent from being split at all.
                if i != 0 {
                    self.flip_parent(i);
                }
                while bucket < want {
                    i = i * 2 + 1;
                    bucket += 1;
                    self.flip_parent(i);
                    let right = self.addr_of(i + 1, bucket);
                    self.push(bucket, right);
                }
                return Some(at);
            }
            if bucket == 0 {
                return None;
            }
            bucket -= 1;
        }
    }

    /// Returns the block of `bytes` at `at`, which must be a block this tree
    /// handed out at that size and has not taken back.
    pub fn release(&mut self, at: usize, bytes: u32) {
        if bytes == 0 {
            return;
        }
        self.open();
        let mut bucket = self.bucket_of(bytes);
        let mut i = self.node_at(at, bucket);

        // Up to the root, USED to UNUSED, merging with a buddy that is UNUSED
        // too. The parent's bit says which: after the flip, a parent that
        // reads SPLIT has one child still in use, so the walk stops there.
        while i != 0 {
            self.flip_parent(i);
            if self.parent_is_split(i) || bucket == 0 {
                break;
            }
            let buddy = self.addr_of(((i - 1) ^ 1) + 1, bucket);
            self.remove(buddy);
            i = (i - 1) / 2;
            bucket -= 1;
        }
        let at = self.addr_of(i, bucket);
        self.push(bucket, at);
    }

    // ---- the tree ----

    /// The smallest bucket a request of `bytes` fits in.
    fn bucket_of(&self, bytes: u32) -> usize {
        let mut bucket = self.buckets - 1;
        let mut size = 1u64 << self.min_log2;
        while size < bytes as u64 {
            bucket -= 1;
            size *= 2;
        }
        bucket
    }

    /// Where the node at `index` of `bucket` starts.
    fn addr_of(&self, index: usize, bucket: usize) -> usize {
        self.base + ((index - (1 << bucket) + 1) << (self.max_log2 as usize - bucket))
    }

    /// The node of `bucket` that starts at `at`. An address is in one node per
    /// bucket, which is why the bucket has to be given.
    fn node_at(&self, at: usize, bucket: usize) -> usize {
        ((at - self.base) >> (self.max_log2 as usize - bucket)) + (1 << bucket) - 1
    }

    /// Whether the parent of `index` reads SPLIT.
    fn parent_is_split(&self, index: usize) -> bool {
        let i = (index - 1) / 2;
        // SAFETY: `i` is a node above the leaves, which is what the bitmap is
        // sized for (bitmap_bytes).
        unsafe { (*self.split.add(i / 8) >> (i % 8)) & 1 == 1 }
    }

    fn flip_parent(&mut self, index: usize) {
        let i = (index - 1) / 2;
        // SAFETY: as `parent_is_split`.
        unsafe { *self.split.add(i / 8) ^= 1 << (i % 8) }
    }

    // ---- the free lists ----

    /// Builds the lists and gives the whole range to bucket 0. Runs once, at
    /// the first call, since a head is its own list's sentinel and cannot be
    /// linked before the tree has stopped moving.
    fn open(&mut self) {
        if self.opened {
            return;
        }
        self.opened = true;
        for b in 0..self.buckets {
            let head = &mut self.heads[b] as *mut List;
            // SAFETY: the head is this value's own field, live for its life.
            unsafe {
                (*head).prev = head;
                (*head).next = head;
            }
        }
        // SAFETY: the bitmap is the caller's, of `bitmap_bytes`.
        unsafe { ptr::write_bytes(self.split, 0, bitmap_bytes(self.min_log2, self.max_log2)) };
        let base = self.base;
        self.push(0, base);
    }

    /// Puts the block at `at` on `bucket`'s list, at the back — `pop` takes
    /// from the back, so a release and a request of the same size hand back
    /// the same address.
    fn push(&mut self, bucket: usize, at: usize) {
        let head = &mut self.heads[bucket] as *mut List;
        let entry = at as *mut List;
        // SAFETY: `at` is a free block of this bucket, at least two words
        // wide (min_log2 is checked at construction), and the head is live.
        unsafe {
            let prev = (*head).prev;
            (*entry).prev = prev;
            (*entry).next = head;
            (*prev).next = entry;
            (*head).prev = entry;
        }
    }

    /// Takes the block off whichever list it is on. The lists being circular,
    /// the list itself needs no mention: a head is a node like any other, so
    /// removing the first or last entry updates it without a branch.
    fn remove(&mut self, at: usize) {
        let entry = at as *mut List;
        // SAFETY: `at` is a block currently on one of this tree's lists.
        unsafe {
            let (prev, next) = ((*entry).prev, (*entry).next);
            (*prev).next = next;
            (*next).prev = prev;
        }
    }

    /// Takes the last block off `bucket`'s list, or none when it is empty —
    /// which a circular list says by pointing at itself.
    fn pop(&mut self, bucket: usize) -> Option<usize> {
        let head = &mut self.heads[bucket] as *mut List;
        // SAFETY: the head is live and its links are this tree's own.
        unsafe {
            let back = (*head).prev;
            if back == head {
                return None;
            }
            self.remove(back as usize);
            Some(back as usize)
        }
    }
}
