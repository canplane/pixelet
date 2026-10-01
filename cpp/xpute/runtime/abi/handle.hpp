// cpp/xpute/runtime/abi/handle.hpp

// Handles to what another side keeps: the number is the table's, the thing
// it names is the holder's — the way an OS hands out a descriptor and keeps
// the file. A handle is `slot | generation << 16`, the far side reading only
// the slot (ARCHITECTURE.xpute.md, the contract's rule 6), so a handle kept
// past its release does not name whatever took the slot after it. Slot 0 is
// never handed out, so 0 is never a handle.
//
// The table is fixed at N slots and never grows: past them `acquire`
// refuses. `Slots` is the same handle over values this side keeps: it grows,
// and a removed value's handle names nothing whatever takes its slot next.
// In both, a slot given back is the next one taken.

#pragma once

#include <array>
#include <cstddef>
#include <cstdint>
#include <optional>
#include <utility>
#include <vector>

#include "../../core/status/bug.hpp"

namespace xpute {

// Bits of a handle that are its slot; the rest are the slot's generation.
// Half of the u32 each way, and the generation is 16 bits to match: the two
// are one number written twice, not a parameter.
inline constexpr std::uint32_t HANDLE_SLOT_BITS = 16;

constexpr std::uint32_t handle_slot(std::uint32_t handle) noexcept {
    return handle & ((1u << HANDLE_SLOT_BITS) - 1);
}

// A slot's next generation: never 0, so a bare slot number — a count or an
// index taken for a handle — names nothing.
constexpr std::uint16_t next_generation(std::uint16_t g) noexcept {
    auto n = static_cast<std::uint16_t>(g + 1);
    return n == 0 ? 1 : n;
}

template <std::size_t N> class HandleTable {
    static_assert(N >= 2 && N <= (std::size_t{1} << HANDLE_SLOT_BITS), "a slot is 16 bits, and slot 0 is never handed out");

  public:
    // A handle to a slot of its own, or none when every slot is taken.
    std::optional<std::uint32_t> acquire() noexcept {
        std::uint32_t slot;
        if (free_ != NONE) {
            slot = free_;
            free_ = next_[slot];
        } else if (fresh_ < N) {
            slot = fresh_++;
        } else {
            return std::nullopt;
        }
        generation_[slot] = next_generation(generation_[slot]);
        live_[slot] = true;
        count_++;
        return slot | std::uint32_t{generation_[slot]} << HANDLE_SLOT_BITS;
    }

    // Gives the handle's slot back; false for a handle that is not live.
    bool release(std::uint32_t handle) noexcept {
        if (!live(handle)) return false;
        std::uint32_t slot = handle_slot(handle);
        live_[slot] = false;
        next_[slot] = free_;
        free_ = slot;
        count_--;
        return true;
    }

    // Whether the handle names its slot now.
    bool live(std::uint32_t handle) const noexcept {
        std::uint32_t slot = handle_slot(handle);
        return slot != 0 && slot < N && live_[slot] && generation_[slot] == handle >> HANDLE_SLOT_BITS;
    }

    // Handles live now.
    std::uint32_t count() const noexcept { return count_; }

  private:
    // No slot: the end of the free list.
    static constexpr std::uint32_t NONE = UINT32_MAX;

    std::array<std::uint16_t, N> generation_{};
    std::array<bool, N> live_{};
    // The free list's links, by slot.
    std::array<std::uint32_t, N> next_{};
    std::uint32_t free_ = NONE;
    // Slots never handed out start here.
    std::uint32_t fresh_ = 1;
    std::uint32_t count_ = 0;
};

// Values held by handle: what a vector of optional values indexed by
// position would be, except that a handle kept past its value's removal names
// nothing rather than whatever was put in its slot after. It grows as values
// are put in and never shrinks.
template <class T> class Slots {
  public:
    // Puts `value` in a slot of its own: its handle.
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

    // The value the handle names, or none once it was removed.
    T *get(std::uint32_t handle) noexcept {
        Entry *e = entry(handle);
        return e != nullptr ? &*e->value : nullptr;
    }
    const T *get(std::uint32_t handle) const noexcept { return const_cast<Slots *>(this)->get(handle); }

    // Takes the value out and gives its slot back; none for a handle that
    // names nothing.
    std::optional<T> remove(std::uint32_t handle) {
        Entry *e = entry(handle);
        if (e == nullptr) return std::nullopt;
        free_.push_back(handle_slot(handle));
        return std::exchange(e->value, std::nullopt);
    }

    // Every value held, with its handle, to `f(handle, value)`.
    template <class F> void each(F &&f) {
        for (std::uint32_t slot = 0; slot < entries_.size(); slot++) {
            Entry &e = entries_[slot];
            if (e.value) f(slot | std::uint32_t{e.generation} << HANDLE_SLOT_BITS, *e.value);
        }
    }

  private:
    // A slot's generation, and its value while one is in it. Slot 0 is held
    // empty so no handle is 0.
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
