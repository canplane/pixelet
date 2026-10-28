// cpp/xpute/guest/abi/handle.hpp

// A handle is `slot | generation << 16`, the far side reading only the slot,
// so a handle kept past its release names nothing. Slot 0 is never handed out.

#pragma once

#include <array>
#include <cstddef>
#include <cstdint>
#include <limits>
#include <optional>
#include <utility>

namespace xpute::abi {

// The rest is the generation, hence its 16 bits. The host uses the same 16.
inline constexpr std::uint32_t SLOT_BITS = 16;
static_assert(SLOT_BITS + std::numeric_limits<std::uint16_t>::digits == 32, "a slot and a generation fill a handle");

constexpr std::uint32_t handle_slot(std::uint32_t handle) noexcept {
    return handle & ((1u << SLOT_BITS) - 1);
}

// N slots, each holding a `T` while its handle is live; a slot given back is
// the next one taken. All-zero bytes are an empty table (for a `T` whose zero
// is its default), so one may lie in memory that starts zeroed, or be emptied
// by zeroing it, never built and moved.
template <class T, std::size_t N> class Table {
    static_assert(N >= 2 && N <= (std::size_t{1} << SLOT_BITS), "a slot is 16 bits, and slot 0 is never handed out");

  public:
    // None when every slot is taken.
    std::optional<std::uint32_t> insert(T value) noexcept {
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
        // Never 0, so no handle is 0.
        auto generation = static_cast<std::uint16_t>(s.generation + 1);
        s.generation = generation == 0 ? 1 : generation;
        s.made = epoch_ + 1;
        s.value = std::move(value);
        count_++;
        return handle_of(slot, s.generation);
    }

    bool live(std::uint32_t handle) const noexcept {
        std::uint32_t slot = handle_slot(handle);
        return slot != 0 && slot < N && slots_[slot].made != 0 && slots_[slot].generation == handle >> SLOT_BITS;
    }

    T *get(std::uint32_t handle) noexcept { return live(handle) ? &slots_[handle_slot(handle)].value : nullptr; }
    const T *get(std::uint32_t handle) const noexcept { return const_cast<Table *>(this)->get(handle); }

    std::optional<T> remove(std::uint32_t handle) noexcept {
        if (!live(handle)) return std::nullopt;
        std::uint32_t slot = handle_slot(handle);
        Slot &s = slots_[slot];
        s.made = 0;
        s.next = free_;
        free_ = slot;
        count_--;
        return std::exchange(s.value, T{});
    }

    // From here, what is taken is what `rollback` gives back.
    void begin() noexcept { epoch_++; }

    // Every handle taken since `begin` and not yet removed, removed and handed
    // to `gone`: what a transaction that did not commit made, which the other
    // side never saw.
    template <class F> void rollback(F &&gone) noexcept {
        for (std::uint32_t slot = 1; slot <= top_; slot++) {
            std::uint32_t handle = handle_of(slot, slots_[slot].generation);
            if (slots_[slot].made != epoch_ + 1) continue;
            if (std::optional<T> value = remove(handle)) gone(handle, std::move(*value));
        }
    }

    std::uint32_t count() const noexcept { return count_; }

    template <class F> void each(F &&f) {
        for (std::uint32_t slot = 1; slot <= top_; slot++) {
            std::uint32_t handle = handle_of(slot, slots_[slot].generation);
            if (live(handle)) f(handle, slots_[slot].value);
        }
    }

  private:
    struct Slot {
        std::uint16_t generation = 0;
        // One past the epoch the slot was taken in; 0 while free. 64 bits, so
        // no count of epochs a program lives through brings a stale one round.
        std::uint64_t made = 0;
        // A free slot's next free slot; 0, never handed out, ends the list.
        std::uint32_t next = 0;
        T value{};
    };

    static constexpr std::uint32_t handle_of(std::uint32_t slot, std::uint16_t generation) noexcept {
        return slot | std::uint32_t{generation} << SLOT_BITS;
    }

    std::array<Slot, N> slots_{};
    // Advanced by `begin`: the slots taken since are those made in it.
    std::uint64_t epoch_ = 0;
    std::uint32_t free_ = 0;
    // The highest slot handed out; those past it never have been.
    std::uint32_t top_ = 0;
    std::uint32_t count_ = 0;
};

} // namespace xpute::abi
