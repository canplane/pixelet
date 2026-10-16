// cpp/guest/abi/handle.hpp

// A handle is `slot
// generation << 16`, the far side reading only the slot,
// so a handle kept past its release names nothing. Slot 0 is never handed out.

#pragma once

#include <array>
#include <cstddef>
#include <cstdint>
#include <optional>
#include <utility>
#include <vector>

#include "../../kit/status/bug.hpp"

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

    // Every handle taken since `begin` and not yet released, released: what a
    // transaction that did not commit made, which the other side never saw.
    void rollback() noexcept {
        for (std::uint32_t slot = 1; slot <= top_; slot++) {
            if (made_[slot] == epoch_ + 1) release(slot | std::uint32_t{generation_[slot]} << HANDLE_SLOT_BITS);
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

// Grows and never shrinks; a handle kept past its value's removal names
// nothing rather than whatever took its slot.
template <class T> class Slots {
  public:
    std::uint32_t insert(T value) {
        if (entries_.empty()) entries_.emplace_back();
        std::uint32_t slot;
        if (!free_.empty()) {
            slot = free_.back();
            free_.pop_back();
        } else {
            ensure(entries_.size() < (std::size_t{1} << HANDLE_SLOT_BITS), Errno::enospc, HANDLE_SLOT_BITS);
            entries_.emplace_back();
            slot = static_cast<std::uint32_t>(entries_.size() - 1);
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

    std::optional<T> remove(std::uint32_t handle) {
        Entry *e = entry(handle);
        if (e == nullptr) return std::nullopt;
        free_.push_back(handle_slot(handle));
        return std::exchange(e->value, std::nullopt);
    }

    template <class F> void each(F &&f) {
        for (std::uint32_t slot = 0; slot < entries_.size(); slot++) {
            Entry &e = entries_[slot];
            if (e.value) f(slot | std::uint32_t{e.generation} << HANDLE_SLOT_BITS, *e.value);
        }
    }

  private:
    // Slot 0 is held empty so no handle is 0.
    struct Entry {
        std::uint16_t generation = 0;
        std::optional<T> value;
    };

    Entry *entry(std::uint32_t handle) noexcept {
        std::uint32_t slot = handle_slot(handle);
        if (slot >= entries_.size()) return nullptr;
        Entry &e = entries_[slot];
        return e.value && e.generation == handle >> HANDLE_SLOT_BITS ? &e : nullptr;
    }

    std::vector<Entry> entries_;
    std::vector<std::uint32_t> free_;
};

} // namespace xpute
