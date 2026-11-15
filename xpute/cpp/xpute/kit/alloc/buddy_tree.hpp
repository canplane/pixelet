// cpp/xpute/kit/alloc/buddy_tree.hpp

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

// This is the guest's page allocator, and `slab.hpp` serves anything smaller
// than a page from it. It is a port of evanw/buddy-malloc that follows the C
// line for line, and departs from it in two places. `update_max_ptr` asks the
// `Range` for memory where the C calls `brk`. A block carries no 8-byte header
// with its size, so every caller passes the size it asked for back to `free`.

#pragma once

#include <array>
#include <concepts>
#include <cstddef>
#include <cstdint>

namespace xpute {

// The width in bits that a range's size and the buddy's exponents fit in; the
// range's base alone is a full pointer.
inline constexpr std::size_t ADDR_BITS = 32;

// The memory this allocator manages. A block is `2^MIN_LOG2` bytes at the
// smallest and `2^MAX_LOG2` at the largest. `MAX_LOG2` bounds the address
// space the tree can describe, not the memory that exists: `end` is where the
// real memory stops.
template <class R>
concept Range = requires {
    { R::MIN_LOG2 } -> std::convertible_to<std::size_t>;
    { R::MAX_LOG2 } -> std::convertible_to<std::size_t>;
    { R::base() } -> std::convertible_to<std::uintptr_t>;
    { R::end() } -> std::convertible_to<std::uintptr_t>;
};

template <Range R> class BuddyMalloc {
    static constexpr std::size_t BUCKET_CNT = R::MAX_LOG2 - R::MIN_LOG2 + 1;
    // The split bits take `2^(BUCKET_CNT - 1) / 8` bytes, which is none at
    // all below four buckets. Past `ADDR_BITS + 1` buckets, one for each
    // exponent from 0 through `ADDR_BITS`, the tree would describe more than
    // a range can address.
    static_assert(BUCKET_CNT >= 4 && BUCKET_CNT <= ADDR_BITS + 1, "a buddy tree's bucket count");

    struct List {
        List *prev;
        List *next;
    };
    static_assert((std::size_t{1} << R::MIN_LOG2) >= sizeof(List), "a free block holds its list links");

  public:
    BuddyMalloc() noexcept = default;
    // A `BuddyMalloc` must not move once it has handed out a block: each
    // bucket's head is the sentinel of its own circular list, and the free
    // blocks point at it.
    BuddyMalloc(const BuddyMalloc &) = delete;
    BuddyMalloc &operator=(const BuddyMalloc &) = delete;

    // The most bytes of the range the allocator has ever reached into, not
    // the bytes live now.
    std::size_t peak() const noexcept { return max_ptr_ - base_ptr_; }
    std::size_t live() const noexcept { return live_; }
    std::size_t range_sz() const noexcept { return end_ptr_ - base_ptr_; }
    // Changes exactly when a request refused before might now be served, so
    // a caller can wait for it to change before it retries.
    std::uint32_t releases() const noexcept { return releases_; }
    // Where the host reads `base_ptr_`, `max_ptr_`, `end_ptr_` and `live_`,
    // then `releases_` and `allocs_` as 32-bit words, in place and in that
    // order. The order of the fields below is part of the contract with the
    // host.
    const std::uintptr_t *stat() const noexcept { return &base_ptr_; }
    std::size_t allocs() const noexcept { return allocs_; }

    std::size_t largest_free() const noexcept {
        if (base_ptr_ == 0) return 0;
        for (std::size_t b = bucket_limit_; b < BUCKET_CNT; b++) {
            if (buckets_[b].next != &buckets_[b]) return std::size_t{1} << (R::MAX_LOG2 - b);
        }
        return 0;
    }

    // Returns null when no block can be had. A block's offset from the
    // range's base is a multiple of its own size, so a caller can find a
    // block's start from any address inside it by masking the offset.
    void *malloc(std::size_t request) noexcept {
        // The first call sets the tree up as one free node of the smallest
        // size; more of the range is taken only as larger blocks are asked
        // for.
        if (base_ptr_ == 0) {
            base_ptr_ = R::base();
            max_ptr_ = base_ptr_;
            end_ptr_ = R::end();
            bucket_limit_ = BUCKET_CNT - 1;
            update_max_ptr(base_ptr_ + sizeof(List));
            list_init(BUCKET_CNT - 1);
            list_push(BUCKET_CNT - 1, reinterpret_cast<List *>(base_ptr_));
        }

        if (request > std::size_t{1} << R::MAX_LOG2) return nullptr;

        std::size_t original_bucket = bucket_for_request(request);
        std::size_t bucket = original_bucket;

        // The loop looks for a free block at least as large as needed, moving
        // to larger buckets when there is no exact match; a larger block is
        // split below.
        for (;;) {
            if (!lower_bucket_limit(bucket)) return nullptr;

            List *ptr = list_pop(bucket);
            if (ptr == nullptr) {
                // When this is not the root's bucket, the search goes on to
                // the next larger one; at bucket 0 there is no larger one, and
                // the request fails.
                if (bucket != bucket_limit_ || bucket == 0) {
                    if (bucket == 0) return nullptr;
                    bucket--;
                    continue;
                }
                // This bucket is the root's and the root is in use. Growing
                // the tree one level puts the root's new right sibling on
                // this bucket's free list.
                if (!lower_bucket_limit(bucket - 1)) return nullptr;
                ptr = list_pop(bucket);
            }

            // The block's memory is reserved before anything is written to
            // it; when the range has no room, the block goes back and the
            // request fails.
            std::size_t sz = std::size_t{1} << (R::MAX_LOG2 - bucket);
            std::size_t need_sz = bucket < original_bucket ? sz / 2 + sizeof(List) : sz;
            if (!update_max_ptr(reinterpret_cast<std::uintptr_t>(ptr) + need_sz)) {
                list_push(bucket, ptr);
                return nullptr;
            }

            // A parent's bit is the XOR of its children's UNUSED flags, so
            // taking this block flips its parent's bit. The grandparent's bit
            // stays: this block's buddy is USED, so the parent was never
            // UNUSED.
            std::size_t i = node_for_ptr(ptr, bucket);
            if (i != 0) flip_parent_is_split(i);

            // A block larger than needed is split down to size. Each step
            // keeps the left half and puts the right half on its bucket's free
            // list.
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

    void free(void *at, std::size_t request) noexcept {
        if (at == nullptr) return;
        List *ptr = static_cast<List *>(at);
        std::size_t bucket = bucket_for_request(request);
        live_ -= std::size_t{1} << (R::MAX_LOG2 - bucket);
        releases_++;
        std::size_t i = node_for_ptr(ptr, bucket);

        // The freed block becomes UNUSED, and while its buddy is UNUSED too the
        // two merge into their parent, up toward the root.
        while (i != 0) {
            flip_parent_is_split(i);
            // After the flip, the parent's bit says SPLIT exactly when the
            // buddy is still USED, and the two cannot merge. The tree's
            // current root has no buddy to merge with.
            if (parent_is_split(i) || bucket == bucket_limit_) break;
            list_remove(ptr_for_node(((i - 1) ^ 1) + 1, bucket));
            i = (i - 1) / 2;
            bucket--;
        }

        // The block goes on the back of its bucket's list, and `malloc` takes
        // from the back, so a free followed by a malloc of the same size hands
        // back the same address.
        list_push(bucket, ptr_for_node(i, bucket));
    }

  private:
    // This stands in for the C's `brk`: the reached memory grows up to the
    // range's end and is refused past it.
    bool update_max_ptr(std::uintptr_t new_val) noexcept {
        if (new_val > max_ptr_) {
            if (new_val > end_ptr_) return false;
            max_ptr_ = new_val;
        }
        return true;
    }

    // A bucket's first node is `(1 << bucket) - 1`, so `idx + 1` is taken
    // before the subtraction, which then never goes below zero.
    List *ptr_for_node(std::size_t idx, std::size_t bucket) const noexcept {
        return reinterpret_cast<List *>(base_ptr_ + ((idx + 1 - (std::size_t{1} << bucket)) << (R::MAX_LOG2 - bucket)));
    }

    std::size_t node_for_ptr(List *ptr, std::size_t bucket) const noexcept {
        return ((reinterpret_cast<std::uintptr_t>(ptr) - base_ptr_) >> (R::MAX_LOG2 - bucket)) + (std::size_t{1} << bucket) - 1;
    }

    bool parent_is_split(std::size_t idx) const noexcept {
        const std::size_t parent = (idx - 1) / 2;
        return (node_is_split_[parent / 8] >> (parent % 8) & 1) == 1;
    }

    void flip_parent_is_split(std::size_t idx) noexcept {
        const std::size_t parent = (idx - 1) / 2;
        node_is_split_[parent / 8] ^= static_cast<std::uint8_t>(1 << (parent % 8));
    }

    std::size_t bucket_for_request(std::size_t request) const noexcept {
        std::size_t bucket = BUCKET_CNT - 1;
        std::size_t sz = std::size_t{1} << R::MIN_LOG2;
        while (sz < request) {
            bucket--;
            sz *= 2;
        }
        return bucket;
    }

    void list_init(std::size_t bucket) noexcept {
        buckets_[bucket].prev = &buckets_[bucket];
        buckets_[bucket].next = &buckets_[bucket];
    }

    void list_push(std::size_t bucket, List *entry) noexcept {
        List *list = &buckets_[bucket];
        List *prev = list->prev;
        entry->prev = prev;
        entry->next = list;
        prev->next = entry;
        list->prev = entry;
    }

    static void list_remove(List *entry) noexcept {
        entry->prev->next = entry->next;
        entry->next->prev = entry->prev;
    }

    List *list_pop(std::size_t bucket) noexcept {
        List *list = &buckets_[bucket];
        List *back = list->prev;
        if (back == list) return nullptr;
        list_remove(back);
        return back;
    }

    // Doubles the tree until its root is at `bucket`. The root is at
    // `bucket_limit_`, and each doubling lowers it by one. Returns false when
    // the range has no room for the links of a new free block.
    bool lower_bucket_limit(std::size_t bucket) noexcept {
        while (bucket < bucket_limit_) {
            std::size_t root = node_for_ptr(reinterpret_cast<List *>(base_ptr_), bucket_limit_);
            // The root is UNUSED, so the whole tree is free and the new,
            // larger root becomes one free block.
            if (!parent_is_split(root)) {
                list_remove(reinterpret_cast<List *>(base_ptr_));
                bucket_limit_--;
                list_init(bucket_limit_);
                list_push(bucket_limit_, reinterpret_cast<List *>(base_ptr_));
                continue;
            }
            // The root is in use, so the new root is SPLIT: its left child is
            // the old root and its right child a new free block, whose links
            // are reserved before they are written.
            List *right_child = ptr_for_node(root + 1, bucket_limit_);
            if (!update_max_ptr(reinterpret_cast<std::uintptr_t>(right_child) + sizeof(List))) return false;
            list_push(bucket_limit_, right_child);
            bucket_limit_--;
            list_init(bucket_limit_);
            // The bit in the new root's parent is flipped, so that a further
            // doubling sees the new root as in use.
            root = (root - 1) / 2;
            if (root != 0) flip_parent_is_split(root);
        }
        return true;
    }

    std::array<List, BUCKET_CNT> buckets_{};
    std::size_t bucket_limit_ = 0;
    // A tree laid out in an array: node `i` has its parent at `(i - 1) / 2`,
    // its children at `2i + 1` and `2i + 2`, and its buddy at
    // `((i - 1) ^ 1) + 1`. Each parent keeps one bit, the XOR of its two
    // children's UNUSED flags. A node's state (UNUSED, SPLIT or USED) is not
    // stored; the walks tell it from these bits.
    std::array<std::uint8_t, (std::size_t{1} << (BUCKET_CNT - 1)) / 8> node_is_split_{};
    std::uintptr_t base_ptr_ = 0;
    std::uintptr_t max_ptr_ = 0;
    std::uintptr_t end_ptr_ = 0;
    std::size_t live_ = 0;
    std::uint32_t releases_ = 0;
    std::uint32_t allocs_ = 0;
};

} // namespace xpute
