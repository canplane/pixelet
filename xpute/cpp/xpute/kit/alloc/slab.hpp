// cpp/xpute/kit/alloc/slab.hpp

// This is a slab allocator of size classes for requests smaller than a page,
// over the buddy allocator of buddy_tree.hpp, whose smallest block is a page.
// Each class borrows a run of pages from the buddy and carves it into slots
// as SLUB does: the free slots form a list threaded through themselves, and
// the run's header sits in its leading slots. A run goes back to the buddy
// when its last slot is freed. A request of a page or more goes straight to
// the buddy.
//
// A slot finds its run by masking its offset from the range's base, which
// works because the buddy aligns every block to its own size from the base.
// Masking the address would not: the base is aligned only to a page, and a run
// can be larger than a page.

#pragma once

#include <algorithm>
#include <array>
#include <bit>
#include <cstddef>
#include <cstdint>

#include "buddy_tree.hpp"

namespace xpute {

template <Range R> class SlabMalloc {
    // The smallest class is sixteen bytes: below that, a class costs more in
    // run headers than it saves.
    static constexpr std::size_t MIN_CLASS_LOG2 = 4;
    // A run holds at least sixteen slots, so its header takes a small share of
    // the run.
    static constexpr std::size_t MIN_SLOTS_LOG2 = 4;
    static constexpr std::size_t CLASS_CNT = R::MIN_LOG2 - MIN_CLASS_LOG2;

    struct Run {
        Run *prev;
        Run *next;
        std::uint8_t *free;
        std::uint32_t inuse;
    };

    static_assert((std::size_t{1} << MIN_CLASS_LOG2) >= sizeof(void *), "a free slot holds the address of the next");

    static constexpr std::size_t class_log2(std::size_t c) noexcept { return MIN_CLASS_LOG2 + c; }

    static constexpr std::size_t run_log2(std::size_t c) noexcept { return std::max(class_log2(c) + MIN_SLOTS_LOG2, std::size_t{R::MIN_LOG2}); }

    static constexpr std::size_t head_slots(std::size_t c) noexcept { return (sizeof(Run) + (std::size_t{1} << class_log2(c)) - 1) >> class_log2(c); }

    static constexpr std::size_t slots(std::size_t c) noexcept { return std::size_t{1} << (run_log2(c) - class_log2(c)); }

    static constexpr bool every_run_has_a_slot() noexcept {
        for (std::size_t c = 0; c < CLASS_CNT; c++) {
            if (head_slots(c) >= slots(c)) return false;
        }
        return true;
    }
    static_assert(every_run_has_a_slot(), "a run has a slot to hand out past the ones its header takes");

  public:
    SlabMalloc() noexcept = default;
    SlabMalloc(const SlabMalloc &) = delete;
    SlabMalloc &operator=(const SlabMalloc &) = delete;

    BuddyMalloc<R> &pages() noexcept { return buddy_; }
    const BuddyMalloc<R> &pages() const noexcept { return buddy_; }
    const std::uintptr_t *stat() const noexcept { return buddy_.stat(); }

    std::size_t peak() const noexcept { return buddy_.peak(); }
    std::size_t live() const noexcept { return buddy_.live(); }
    std::size_t range_sz() const noexcept { return buddy_.range_sz(); }
    std::uint32_t releases() const noexcept { return buddy_.releases(); }
    std::size_t largest_free() const noexcept { return buddy_.largest_free(); }
    std::size_t allocs() const noexcept { return allocs_; }
    std::uint32_t refusals() const noexcept { return refusals_; }
    std::size_t run_live() const noexcept { return run_live_; }
    std::size_t slot_live() const noexcept { return slot_live_; }

    void *alloc(std::size_t bytes, std::size_t align) noexcept {
        std::size_t c;
        if (!class_for(bytes, align, c)) {
            // A page or more goes to the buddy. The range's base is aligned
            // only to a page, so no block is sure of a larger alignment.
            void *at = align > std::size_t{1} << R::MIN_LOG2 ? nullptr : buddy_.malloc(bytes);
            if (at == nullptr) refusals_++;
            else allocs_++;
            return at;
        }
        Run *run = partial_[c];
        if (run == nullptr) {
            run = open_run(c);
            if (run == nullptr) {
                refusals_++;
                return nullptr;
            }
        }
        std::uint8_t *slot = run->free;
        run->free = next_of(slot);
        run->inuse++;
        if (run->free == nullptr) drop_partial(c, run);
        slot_live_ += std::size_t{1} << class_log2(c);
        allocs_++;
        return slot;
    }

    void dealloc(void *at, std::size_t bytes, std::size_t align) noexcept {
        std::size_t c;
        if (!class_for(bytes, align, c)) {
            buddy_.free(at, bytes);
            return;
        }
        std::size_t run_sz = std::size_t{1} << run_log2(c);
        std::uintptr_t base = R::base();
        Run *run = reinterpret_cast<Run *>(base + ((reinterpret_cast<std::uintptr_t>(at) - base) & ~(run_sz - 1)));
        std::uint8_t *slot = static_cast<std::uint8_t *>(at);
        bool was_full = run->free == nullptr;
        set_next(slot, run->free);
        run->free = slot;
        run->inuse--;
        slot_live_ -= std::size_t{1} << class_log2(c);
        if (run->inuse == 0) {
            // The run's last slot is free, so the run goes back to the buddy,
            // where any class can have its pages, rather than staying with
            // this class.
            if (!was_full) drop_partial(c, run);
            run_live_ -= run_sz;
            buddy_.free(run, run_sz);
        } else if (was_full) {
            push_partial(c, run);
        }
    }

    void *malloc(std::size_t bytes) noexcept { return alloc(bytes, 8); }
    void free(void *at, std::size_t bytes) noexcept { dealloc(at, bytes, 8); }

  private:
    // `class_for` gives the class for a request, or false when the buddy
    // serves it. A slot is aligned to its own size, since it sits at a
    // multiple of its class from a run base aligned to the run's size, so a
    // request's alignment is met by raising its size to the alignment.
    static bool class_for(std::size_t bytes, std::size_t align, std::size_t &c) noexcept {
        std::size_t need = std::max({bytes, align, std::size_t{1}});
        if (need >= std::size_t{1} << R::MIN_LOG2) return false;
        std::size_t log2 = std::max<std::size_t>(std::countr_zero(std::bit_ceil(need)), MIN_CLASS_LOG2);
        c = log2 - MIN_CLASS_LOG2;
        return true;
    }

    static std::uint8_t *next_of(std::uint8_t *slot) noexcept { return *reinterpret_cast<std::uint8_t **>(slot); }
    static void set_next(std::uint8_t *slot, std::uint8_t *next) noexcept { *reinterpret_cast<std::uint8_t **>(slot) = next; }

    Run *open_run(std::size_t c) noexcept {
        std::size_t run_sz = std::size_t{1} << run_log2(c);
        auto *at = static_cast<std::uint8_t *>(buddy_.malloc(run_sz));
        if (at == nullptr) return nullptr;
        std::size_t class_sz = std::size_t{1} << class_log2(c);
        std::uint8_t *next = nullptr;
        // The free list is built from the last slot back, so the first slots
        // handed out have the lowest addresses.
        for (std::size_t i = slots(c); i > head_slots(c);) {
            i--;
            set_next(at + i * class_sz, next);
            next = at + i * class_sz;
        }
        run_live_ += run_sz;
        Run *run = reinterpret_cast<Run *>(at);
        run->free = next;
        run->inuse = 0;
        push_partial(c, run);
        return run;
    }

    void push_partial(std::size_t c, Run *run) noexcept {
        Run *head = partial_[c];
        run->prev = nullptr;
        run->next = head;
        if (head != nullptr) head->prev = run;
        partial_[c] = run;
    }

    void drop_partial(std::size_t c, Run *run) noexcept {
        if (run->prev == nullptr) partial_[c] = run->next;
        else run->prev->next = run->next;
        if (run->next != nullptr) run->next->prev = run->prev;
    }

    BuddyMalloc<R> buddy_;
    std::array<Run *, CLASS_CNT> partial_{};
    std::size_t allocs_ = 0;
    std::uint32_t refusals_ = 0;
    std::size_t run_live_ = 0;
    std::size_t slot_live_ = 0;
};

} // namespace xpute
