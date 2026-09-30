// cpp/xpute/runtime/abi/handle.hpp

// Handles to what the guest keeps: `slot | generation << 16`, the far side
// reading only the slot (ARCHITECTURE.xpute.md, the contract's rule 6). The
// table checks the generation, so a handle kept past its release does not
// name what took its slot next; slot 0 is never handed out, so 0 is never a
// handle.

#pragma once

#include <array>
#include <cstddef>
#include <cstdint>

namespace xpute {

inline constexpr std::uint32_t HANDLE_SLOT_BITS = 16;

constexpr std::uint32_t handle_slot(std::uint32_t handle) noexcept {
    return handle & ((1u << HANDLE_SLOT_BITS) - 1);
}

template <std::size_t N> class HandleTable {
    static_assert(N >= 2 && N <= (std::size_t{1} << HANDLE_SLOT_BITS), "a slot is 16 bits, and slot 0 is never handed out");

  public:
    // A handle to a free slot, or 0 when every slot is taken. The lowest free
    // slot: a table this size is searched rather than linked, and which free
    // slot comes first is the table's own.
    std::uint32_t acquire() noexcept {
        for (std::uint32_t slot = 1; slot < N; slot++) {
            if (live_[slot]) continue;
            // Generation 0 is never handed out, so a bare slot number — a
            // count or an index taken for a handle — names nothing.
            auto g = static_cast<std::uint16_t>(generation_[slot] + 1);
            generation_[slot] = g == 0 ? 1 : g;
            live_[slot] = true;
            return slot | std::uint32_t{generation_[slot]} << HANDLE_SLOT_BITS;
        }
        return 0;
    }

    // Gives the handle's slot back; false for a handle that is not live.
    bool release(std::uint32_t handle) noexcept {
        if (!live(handle)) return false;
        live_[handle_slot(handle)] = false;
        return true;
    }

    // Whether the handle names its slot now.
    bool live(std::uint32_t handle) const noexcept {
        std::uint32_t slot = handle_slot(handle);
        return slot != 0 && slot < N && live_[slot] && generation_[slot] == handle >> HANDLE_SLOT_BITS;
    }

  private:
    std::array<std::uint16_t, N> generation_{};
    std::array<bool, N> live_{};
};

} // namespace xpute
