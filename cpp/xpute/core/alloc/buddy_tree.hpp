// cpp/xpute/core/alloc/buddy_tree.hpp

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

// evanw/buddy-malloc, line for line, over a range that is given rather than
// asked for — the page allocator, with slab.hpp over it for everything
// smaller than a page. The same two changes from the C that xpute-core's
// alloc/buddy_tree.rs makes, and for the same reasons:
//
// Where the memory comes from. The C calls `brk`; here `update_max_ptr` asks
// the `Range`, and fails in exactly the same place, which leaves everything
// built on it — the tree that starts small and doubles, `bucket_limit`, the
// order the free lists are filled in — as it was.
//
// The 8-byte header is gone. It exists in the C because `free(void *)` is told
// an address and nothing else; every caller here is handed the size back, so
// `free` takes it and a block is nothing but its bytes.
//
// Everything else is the original: the linearized tree whose index says both
// where a node is and how large, one bit a node pair holding the exclusive-or
// of its children's UNUSED flags, and circular doubly-linked free lists
// threaded through the free blocks themselves.

#pragma once

#include <array>
#include <concepts>
#include <cstddef>
#include <cstdint>

namespace xpute {

// The range an allocator spans and the two exponents that shape its tree.
// `base()`/`end()` are asked at the first request and every one after, so how
// far the range reaches is the range's answer. `MIN_LOG2` is the smallest
// block; `MAX_LOG2` the largest the tree can describe, a statement about the
// address space and not about any memory.
template <class R>
concept Range = requires {
    { R::MIN_LOG2 } -> std::convertible_to<std::size_t>;
    { R::MAX_LOG2 } -> std::convertible_to<std::size_t>;
    { R::base() } -> std::convertible_to<std::uintptr_t>;
    { R::end() } -> std::convertible_to<std::uintptr_t>;
};

template <Range R> class BuddyMalloc {
    static constexpr std::size_t BUCKET_COUNT = R::MAX_LOG2 - R::MIN_LOG2 + 1;
    // A tree of fewer than four buckets has no split bit to keep, and one of
    // more than 33 a bitmap past what a 32-bit range can address.
    static_assert(BUCKET_COUNT >= 4 && BUCKET_COUNT <= 33, "a buddy tree's bucket count");

    // Free lists are circular doubly-linked lists, one a bucket, threaded
    // through the free blocks of that size.
    struct List {
        List *prev;
        List *next;
    };
    static_assert((std::size_t{1} << R::MIN_LOG2) >= sizeof(List), "a free block holds its list links");

  public:
    BuddyMalloc() noexcept = default;
    // Its bucket heads are their own lists' sentinels, so it must not move
    // once a block has been handed out.
    BuddyMalloc(const BuddyMalloc &) = delete;
    BuddyMalloc &operator=(const BuddyMalloc &) = delete;

    // Bytes of the range that have ever been used: what the allocator has
    // reached into, not what is live in it.
    std::size_t used() const noexcept { return max_ptr_ - base_ptr_; }
    // Bytes handed out and not yet given back, each at its bucket's size.
    std::size_t live() const noexcept { return live_; }
    // The bytes of the range the allocator was given.
    std::size_t range_bytes() const noexcept { return end_ptr_ - base_ptr_; }
    // Blocks given back. A request refused before might be served once this
    // has moved, and not before.
    std::uint32_t releases() const noexcept { return releases_; }
    // Where the row lies — base, max, end, live, then releases and allocs as
    // half-words — for a host to read in place.
    const std::uintptr_t *stat() const noexcept { return &base_ptr_; }
    // Requests served since the first call.
    std::size_t allocs() const noexcept { return allocs_; }

    // The largest block on a free list, or 0 when every list is empty.
    std::size_t largest_free() const noexcept {
        if (base_ptr_ == 0) return 0;
        for (std::size_t b = bucket_limit_; b < BUCKET_COUNT; b++) {
            if (buckets_[b].next != &buckets_[b]) return std::size_t{1} << (R::MAX_LOG2 - b);
        }
        return 0;
    }

    // The address a request of `request` bytes was given, or null. The block
    // is a power of two of at least the smallest block, at an offset from the
    // range's base that is a multiple of its own size.
    void *malloc(std::size_t request) noexcept {
        // Initialize on the first call: the tree starts as a single node of
        // the smallest size, and more of the range is taken as needed.
        if (base_ptr_ == 0) {
            base_ptr_ = R::base();
            max_ptr_ = base_ptr_;
            end_ptr_ = R::end();
            bucket_limit_ = BUCKET_COUNT - 1;
            update_max_ptr(base_ptr_ + sizeof(List));
            list_init(BUCKET_COUNT - 1);
            list_push(BUCKET_COUNT - 1, reinterpret_cast<List *>(base_ptr_));
        }

        if (request > std::size_t{1} << R::MAX_LOG2) return nullptr;

        // The smallest bucket that fits; whether there is room is not checked
        // yet.
        std::size_t original_bucket = bucket_for_request(request);
        std::size_t bucket = original_bucket;

        // A bucket with a non-empty free list at least as large as needed,
        // splitting a larger one where there is no exact match.
        for (;;) {
            if (!lower_bucket_limit(bucket)) return nullptr;

            List *ptr = list_pop(bucket);
            if (ptr == nullptr) {
                // Away from the root, or unable to grow the tree further:
                // carry on to the next bucket.
                if (bucket != bucket_limit_ || bucket == 0) {
                    if (bucket == 0) return nullptr;
                    bucket--;
                    continue;
                }
                // Otherwise grow the tree one more level and pop again: the
                // root is known to be used, so this adds a parent above it in
                // the SPLIT state with its right child on this bucket's list.
                if (!lower_bucket_limit(bucket - 1)) return nullptr;
                ptr = list_pop(bucket);
            }

            // Take the address range before going further; where there is no
            // room, put the block back and fail.
            std::size_t size = std::size_t{1} << (R::MAX_LOG2 - bucket);
            std::size_t bytes_needed = bucket < original_bucket ? size / 2 + sizeof(List) : size;
            if (!update_max_ptr(reinterpret_cast<std::uintptr_t>(ptr) + bytes_needed)) {
                list_push(bucket, ptr);
                return nullptr;
            }

            // A node off the free list goes from UNUSED to USED, which flips
            // the parent's "is split" bit. The grandparent never needs the
            // same: the buddy is USED, or the parent would not be split.
            std::size_t i = node_for_ptr(ptr, bucket);
            if (i != 0) flip_parent_is_split(i);

            // A node larger than needed is split down to size, each step
            // moving to the left child, splitting the parent, and putting the
            // right child on its bucket's list.
            while (bucket < original_bucket) {
                i = i * 2 + 1;
                bucket++;
                flip_parent_is_split(i);
                list_push(bucket, ptr_for_node(i + 1, bucket));
            }

            live_ += std::size_t{1} << (R::MAX_LOG2 - original_bucket);
            allocs_++;
            return ptr;
        }
    }

    // Gives back what `malloc` returned, told the same size it asked for.
    void free(void *at, std::size_t request) noexcept {
        if (at == nullptr) return;
        List *ptr = static_cast<List *>(at);
        std::size_t bucket = bucket_for_request(request);
        live_ -= std::size_t{1} << (R::MAX_LOG2 - bucket);
        releases_++;
        std::size_t i = node_for_ptr(ptr, bucket);

        // Up to the root, flipping USED blocks to UNUSED and merging UNUSED
        // buddies into a single UNUSED parent.
        while (i != 0) {
            flip_parent_is_split(i);
            // A parent that now reads SPLIT means the buddy is USED, so there
            // is nothing to merge with; a root has no buddy.
            if (parent_is_split(i) || bucket == bucket_limit_) break;
            list_remove(ptr_for_node(((i - 1) ^ 1) + 1, bucket));
            i = (i - 1) / 2;
            bucket--;
        }

        // At the back of this bucket's list: `malloc` takes from the back, so
        // a free followed by a malloc of the same size hands back the same
        // address.
        list_push(bucket, ptr_for_node(i, bucket));
    }

  private:
    // Make sure every address before `new_value` may be used: the C asks the
    // kernel with `brk`, and here the range says. False where it cannot.
    bool update_max_ptr(std::uintptr_t new_value) noexcept {
        if (new_value > max_ptr_) {
            if (new_value > end_ptr_) return false;
            max_ptr_ = new_value;
        }
        return true;
    }

    // The address a node stands for. A bucket's first node is
    // `(1 << bucket) - 1`, so the `+ 1` comes before the subtraction.
    List *ptr_for_node(std::size_t index, std::size_t bucket) const noexcept {
        return reinterpret_cast<List *>(base_ptr_ + ((index + 1 - (std::size_t{1} << bucket)) << (R::MAX_LOG2 - bucket)));
    }

    // The node an address stands for, the bucket making it one of many.
    std::size_t node_for_ptr(List *ptr, std::size_t bucket) const noexcept {
        return ((reinterpret_cast<std::uintptr_t>(ptr) - base_ptr_) >> (R::MAX_LOG2 - bucket)) + (std::size_t{1} << bucket) - 1;
    }

    bool parent_is_split(std::size_t index) const noexcept {
        index = (index - 1) / 2;
        return (node_is_split_[index / 8] >> (index % 8) & 1) == 1;
    }

    void flip_parent_is_split(std::size_t index) noexcept {
        index = (index - 1) / 2;
        node_is_split_[index / 8] ^= static_cast<std::uint8_t>(1 << (index % 8));
    }

    // The smallest bucket that fits `request`.
    std::size_t bucket_for_request(std::size_t request) const noexcept {
        std::size_t bucket = BUCKET_COUNT - 1;
        std::size_t size = std::size_t{1} << R::MIN_LOG2;
        while (size < request) {
            bucket--;
            size *= 2;
        }
        return bucket;
    }

    void list_init(std::size_t bucket) noexcept {
        buckets_[bucket].prev = &buckets_[bucket];
        buckets_[bucket].next = &buckets_[bucket];
    }

    // Appends a free block that is in no list, overwriting its links.
    void list_push(std::size_t bucket, List *entry) noexcept {
        List *list = &buckets_[bucket];
        List *prev = list->prev;
        entry->prev = prev;
        entry->next = list;
        prev->next = entry;
        list->prev = entry;
    }

    // Removes an entry from whichever list it is in: the lists being
    // circular, a head's own links are updated when its first or last goes.
    static void list_remove(List *entry) noexcept {
        entry->prev->next = entry->next;
        entry->next->prev = entry->prev;
    }

    // The last entry, removed, or null for an empty list — which a circular
    // list says by pointing at itself.
    List *list_pop(std::size_t bucket) noexcept {
        List *list = &buckets_[bucket];
        List *back = list->prev;
        if (back == list) return nullptr;
        list_remove(back);
        return back;
    }

    // The tree is rooted at the bucket limit; this grows it by doubling until
    // the root lies at `bucket`, each doubling lowering the limit by one.
    bool lower_bucket_limit(std::size_t bucket) noexcept {
        while (bucket < bucket_limit_) {
            std::size_t root = node_for_ptr(reinterpret_cast<List *>(base_ptr_), bucket_limit_);
            // A parent that is not SPLIT means the whole space is free: clear
            // the root's list, raise the tree, and give the widened space to
            // the new root.
            if (!parent_is_split(root)) {
                list_remove(reinterpret_cast<List *>(base_ptr_));
                bucket_limit_--;
                list_init(bucket_limit_);
                list_push(bucket_limit_, reinterpret_cast<List *>(base_ptr_));
                continue;
            }
            // Otherwise the tree is in use: make a parent for the root in the
            // SPLIT state with its right child on the free list, taking the
            // memory for that entry before writing it.
            List *right_child = ptr_for_node(root + 1, bucket_limit_);
            if (!update_max_ptr(reinterpret_cast<std::uintptr_t>(right_child) + sizeof(List))) return false;
            list_push(bucket_limit_, right_child);
            bucket_limit_--;
            list_init(bucket_limit_);
            // The grandparent's SPLIT flag, so lowering the limit again knows
            // the root just added is in use.
            root = (root - 1) / 2;
            if (root != 0) flip_parent_is_split(root);
        }
        return true;
    }

    std::array<List, BUCKET_COUNT> buckets_{};
    std::size_t bucket_limit_ = 0;
    // The linearized tree of bits: parent `(i - 1) / 2`, children `i * 2 + 1`
    // and `i * 2 + 2`, sibling `((i - 1) ^ 1) + 1`. Only SPLIT is stored.
    std::array<std::uint8_t, (std::size_t{1} << (BUCKET_COUNT - 1)) / 8> node_is_split_{};
    std::uintptr_t base_ptr_ = 0;
    std::uintptr_t max_ptr_ = 0;
    std::uintptr_t end_ptr_ = 0;
    std::size_t live_ = 0;
    std::uint32_t releases_ = 0;
    std::uint32_t allocs_ = 0;
};

} // namespace xpute
