// cpp/xpute/guest/abi/handle.hpp

// A handle is `slot | generation << 16`, the far side reading only the slot,
// so a handle kept past its release names nothing. Slot 0 is never handed out.
// `HandleTable` is fixed at N slots for what another side keeps; `Slots` is
// fixed at N slots too, and holds values this side keeps.

#pragma once

#include <array>
#include <cstddef>
#include <cstdint>
#include <optional>
#include <utility>

#include "xpute/kit/status/bug.hpp"

namespace xpute {

// Half the u32 each way, matching the 16-bit generation.
inline constexpr std::uint32_t HANDLE_SLOT_BITS = 16;

constexpr std::uint32_t handle_slot(std::uint32_t handle) noexcept {
    return handle & ((1u << HANDLE_SLOT_BITS) - 1);
}

// Never 0, so a bare slot number taken for a handle names nothing.
constexpr std::uint16_t next_generation(std::uint16_t g) noexcept {
    auto n = static_cast<std::uint16_t>(g + 1);
    return n == 0 ? 1 : n;
}

// All-zero bytes are an empty table, so one may live in memory that starts
// zeroed, or be emptied by zeroing it, and is never built and moved.
template <std::size_t N> class HandleTable {
    static_assert(N >= 2 && N <= (std::size_t{1} << HANDLE_SLOT_BITS), "a slot is 16 bits, and slot 0 is never handed out");

  public:
    std::optional<std::uint32_t> acquire() noexcept {
        std::uint32_t slot;
        if (free_ != 0) {
            slot = free_;
            free_ = next_[slot];
        } else if (std::size_t{top_} + 1 < N) {
            slot = ++top_;
        } else {
            return std::nullopt;
        }
        generation_[slot] = next_generation(generation_[slot]);
        made_[slot] = epoch_ + 1;
        count_++;
        return slot | std::uint32_t{generation_[slot]} << HANDLE_SLOT_BITS;
    }

    // False for a handle that is not live.
    bool release(std::uint32_t handle) noexcept {
        if (!live(handle)) return false;
        std::uint32_t slot = handle_slot(handle);
        made_[slot] = 0;
        next_[slot] = free_;
        free_ = slot;
        count_--;
        return true;
    }

    bool live(std::uint32_t handle) const noexcept {
        std::uint32_t slot = handle_slot(handle);
        return slot != 0 && slot < N && made_[slot] != 0 && generation_[slot] == handle >> HANDLE_SLOT_BITS;
    }

    // From here, what is taken is what `rollback` gives back.
    void begin() noexcept { epoch_++; }

    // Every handle taken since `begin` and not yet released, released and
    // handed to `gone`: what a transaction that did not commit made, which
    // the other side never saw.
    template <class F> void rollback(F &&gone) noexcept {
        for (std::uint32_t slot = 1; slot <= top_; slot++) {
            std::uint32_t handle = slot | std::uint32_t{generation_[slot]} << HANDLE_SLOT_BITS;
            if (made_[slot] == epoch_ + 1 && release(handle)) gone(handle);
        }
    }

    std::uint32_t count() const noexcept { return count_; }

  private:
    std::array<std::uint16_t, N> generation_{};
    // One past the epoch a live slot was taken in; 0 for a free one. 64 bits so
    // no count of epochs a program lives through brings a stale one round.
    std::array<std::uint64_t, N> made_{};
    // A free slot's next free slot; 0, which is never handed out, ends the list.
    std::array<std::uint32_t, N> next_{};
    std::uint32_t free_ = 0;
    // Advanced by `begin`: the slots taken since are those made in it.
    std::uint64_t epoch_ = 0;
    // The highest slot handed out; those past it never have been.
    std::uint32_t top_ = 0;
    std::uint32_t count_ = 0;
};

// Fixed at N slots, slot 0 never handed out, as `HandleTable`; a slot given
// back is the next one taken.
template <class T, std::size_t N> class Slots {
    static_assert(N >= 2 && N <= (std::size_t{1} << HANDLE_SLOT_BITS), "a slot is 16 bits, and slot 0 is never handed out");

  public:
    std::uint32_t insert(T value) noexcept {
        std::uint32_t slot;
        if (free_ != 0) {
            slot = free_;
            free_ = next_[slot];
        } else {
            ensure(std::size_t{top_} + 1 < N, Errno::enospc, N);
            slot = ++top_;
        }
        Entry &e = entries_[slot];
        e.generation = next_generation(e.generation);
        e.value.emplace(std::move(value));
        return slot | std::uint32_t{e.generation} << HANDLE_SLOT_BITS;
    }

    T *get(std::uint32_t handle) noexcept {
        Entry *e = entry(handle);
        return e != nullptr ? &*e->value : nullptr;
    }
    const T *get(std::uint32_t handle) const noexcept { return const_cast<Slots *>(this)->get(handle); }

    std::optional<T> remove(std::uint32_t handle) noexcept {
        Entry *e = entry(handle);
        if (e == nullptr) return std::nullopt;
        std::uint32_t slot = handle_slot(handle);
        next_[slot] = free_;
        free_ = slot;
        return std::exchange(e->value, std::nullopt);
    }

    template <class F> void each(F &&f) {
        for (std::uint32_t slot = 1; slot <= top_; slot++) {
            Entry &e = entries_[slot];
            if (e.value) f(slot | std::uint32_t{e.generation} << HANDLE_SLOT_BITS, *e.value);
        }
    }

  private:
    struct Entry {
        std::uint16_t generation = 0;
        std::optional<T> value;
    };

    Entry *entry(std::uint32_t handle) noexcept {
        std::uint32_t slot = handle_slot(handle);
        if (slot == 0 || slot > top_) return nullptr;
        Entry &e = entries_[slot];
        return e.value && e.generation == handle >> HANDLE_SLOT_BITS ? &e : nullptr;
    }

    std::array<Entry, N> entries_{};
    // A free slot's next free slot; 0, which is never handed out, ends the list.
    std::array<std::uint32_t, N> next_{};
    std::uint32_t free_ = 0;
    // The highest slot handed out; those past it never have been.
    std::uint32_t top_ = 0;
};

} // namespace xpute
