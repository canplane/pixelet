// cpp/xpute/core/alloc/slab.hpp

// Size classes over buddy_tree.hpp, as SLUB: a class with no free slot borrows
// a run from the buddy and returns it when its last slot is freed. A slot finds
// its run by masking its offset, which holds because the buddy aligns a block
// to its own size.

#pragma once

#include <algorithm>
#include <array>
#include <bit>
#include <cstddef>
#include <cstdint>

#include "buddy_tree.hpp"

namespace xpute {

template <Range R> class SlabMalloc {
    // Below 16 bytes a class costs more in run headers than it saves.
    static constexpr std::size_t MIN_CLASS_LOG2 = 4;
    // So a run's header is at most a sixteenth of it.
    static constexpr std::size_t MIN_SLOTS_LOG2 = 4;
    static constexpr std::size_t CLASS_COUNT = R::MIN_LOG2 - MIN_CLASS_LOG2;

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
        for (std::size_t c = 0; c < CLASS_COUNT; c++) {
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

    std::size_t used() const noexcept { return buddy_.used(); }
    std::size_t live() const noexcept { return buddy_.live(); }
    std::size_t range_bytes() const noexcept { return buddy_.range_bytes(); }
    std::uint32_t releases() const noexcept { return buddy_.releases(); }
    std::size_t largest_free() const noexcept { return buddy_.largest_free(); }
    std::size_t allocs() const noexcept { return allocs_; }
    std::uint32_t refusals() const noexcept { return refusals_; }
    std::size_t run_live() const noexcept { return run_live_; }
    std::size_t slot_live() const noexcept { return slot_live_; }

    void *alloc(std::size_t bytes, std::size_t align) noexcept {
        std::size_t c;
        if (!class_for(bytes, align, c)) {
            // A page or more: the buddy's placement satisfies any alignment up
            // to a page and none past one.
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
        std::size_t run_bytes = std::size_t{1} << run_log2(c);
        std::uintptr_t base = R::base();
        Run *run = reinterpret_cast<Run *>(base + ((reinterpret_cast<std::uintptr_t>(at) - base) & ~(run_bytes - 1)));
        std::uint8_t *slot = static_cast<std::uint8_t *>(at);
        bool was_full = run->free == nullptr;
        set_next(slot, run->free);
        run->free = slot;
        run->inuse--;
        slot_live_ -= std::size_t{1} << class_log2(c);
        if (run->inuse == 0) {
            // The last slot: the run goes back, and every class gets the pages
            // rather than this one keeping them.
            if (!was_full) drop_partial(c, run);
            run_live_ -= run_bytes;
            buddy_.free(run, run_bytes);
        } else if (was_full) {
            push_partial(c, run);
        }
    }

    void *malloc(std::size_t bytes) noexcept { return alloc(bytes, 8); }
    void free(void *at, std::size_t bytes) noexcept { dealloc(at, bytes, 8); }

  private:
    // The class a request falls in, or false where it is the buddy's. The
    // alignment is folded into the size: a slot sits at a multiple of its
    // class from a run base that is a multiple of its own size.
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
        std::size_t run_bytes = std::size_t{1} << run_log2(c);
        auto *at = static_cast<std::uint8_t *>(buddy_.malloc(run_bytes));
        if (at == nullptr) return nullptr;
        std::size_t cls = std::size_t{1} << class_log2(c);
        std::uint8_t *next = nullptr;
        // From the back, so a run's first slots out are its lowest addresses.
        for (std::size_t i = slots(c); i > head_slots(c);) {
            i--;
            set_next(at + i * cls, next);
            next = at + i * cls;
        }
        run_live_ += run_bytes;
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
    std::array<Run *, CLASS_COUNT> partial_{};
    std::size_t allocs_ = 0;
    std::uint32_t refusals_ = 0;
    std::size_t run_live_ = 0;
    std::size_t slot_live_ = 0;
};

} // namespace xpute
