// cpp/xpute/gpu/guest/handle.hpp

// The encoder's table of handles, as `xpute-gpu-guest`'s `handle.rs` keeps
// it, over the link's handle (xpute/gpu/link/handle.hpp). 0 is never a
// handle, and slot 0 holds no value: slots from 1 hold the objects the
// encoder made.

#pragma once

#include <array>
#include <cstddef>
#include <cstdint>
#include <optional>
#include <utility>

#include "xpute/gpu/link/handle.hpp"

namespace xpute::gpu {

// A table has N slots, each holding a `T` while its handle is live. A slot
// given back is the next one taken.
template <class T, std::size_t N> class Table {
    static_assert(N >= 2 && N <= (std::size_t{1} << SLOT_BITS), "a slot is 16 bits, and slot 0 holds no value");

  public:
    // `insert` answers none when every slot is taken.
    std::optional<std::uint32_t> insert(T val) noexcept {
        std::optional<std::uint32_t> slot = take();
        if (!slot) return std::nullopt;
        Slot &s = slots_[*slot];
        s.taken_epoch = epoch_ + 1;
        s.val = std::move(val);
        cnt_++;
        return handle_of(*slot, s.generation);
    }

    bool live(std::uint32_t handle) const noexcept {
        std::uint32_t slot = handle_slot(handle);
        return slot != 0 && slot < N && slots_[slot].taken_epoch != 0 && slots_[slot].generation == handle >> SLOT_BITS;
    }

    T *get(std::uint32_t handle) noexcept { return live(handle) ? &slots_[handle_slot(handle)].val : nullptr; }

    std::optional<T> remove(std::uint32_t handle) noexcept {
        if (!live(handle)) return std::nullopt;
        std::uint32_t slot = handle_slot(handle);
        Slot &s = slots_[slot];
        s.taken_epoch = 0;
        s.next = free_;
        free_ = slot;
        cnt_--;
        return std::exchange(s.val, T{});
    }

    // Begins a transaction: the slots taken from here on are the ones
    // `rollback` gives back.
    void begin() noexcept { epoch_++; }

    // Removes every handle taken since `begin` and not yet removed, and hands
    // each to `release`. These are what a transaction that did not commit
    // made, which the other side never saw.
    template <class F> void rollback(F &&release) noexcept {
        for (std::uint32_t slot = 1; slot <= top_; slot++) {
            std::uint32_t handle = handle_of(slot, slots_[slot].generation);
            if (slots_[slot].taken_epoch != epoch_ + 1) continue;
            if (std::optional<T> val = remove(handle)) release(handle, std::move(*val));
        }
    }

    std::uint32_t cnt() const noexcept { return cnt_; }

  private:
    // Takes the next free slot under its next generation.
    std::optional<std::uint32_t> take() noexcept {
        std::uint32_t slot;
        if (free_ != 0) {
            slot = free_;
            free_ = slots_[slot].next;
        } else if (std::size_t{top_} + 1 < N) {
            slot = ++top_;
        } else {
            return std::nullopt;
        }
        Slot &s = slots_[slot];
        // The generation is never 0, so no handle is 0.
        auto generation = static_cast<std::uint16_t>(s.generation + 1);
        s.generation = generation == 0 ? 1 : generation;
        return slot;
    }

    struct Slot {
        std::uint16_t generation = 0;
        // One more than the epoch in which the slot was taken, or 0 while the
        // slot is free. It has 64 bits so that no program lives through enough
        // epochs for the count to wrap round to a stale value.
        std::uint64_t taken_epoch = 0;
        // The next slot in the free list. Slot 0 holds no value, so 0 ends
        // the list.
        std::uint32_t next = 0;
        T val{};
    };

    static constexpr std::uint32_t handle_of(std::uint32_t slot, std::uint16_t generation) noexcept {
        return slot | std::uint32_t{generation} << SLOT_BITS;
    }

    std::array<Slot, N> slots_{};
    // The current transaction, advanced by `begin`. The slots taken since
    // then are the ones the transaction made.
    std::uint64_t epoch_ = 0;
    std::uint32_t free_ = 0;
    // The highest slot ever handed out; the slots past it never have been.
    std::uint32_t top_ = 0;
    std::uint32_t cnt_ = 0;
};

} // namespace xpute::gpu
