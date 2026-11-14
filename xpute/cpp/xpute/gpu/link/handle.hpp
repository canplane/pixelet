// cpp/xpute/gpu/link/handle.hpp

// A handle is a slot and that slot's generation, so a handle kept past its
// release names nothing that takes the slot later; the host reads such a
// handle's slot alone (`handle_slot`), as `xpute-gpu-link`'s `handle.rs`
// says.

#pragma once

#include <cstdint>
#include <limits>

#include "api.spec.hpp"

namespace xpute::gpu {

static_assert(SLOT_BITS + std::numeric_limits<std::uint16_t>::digits == 32, "a slot and a generation fill a handle");

constexpr std::uint32_t handle_slot(std::uint32_t handle) noexcept {
    return handle & ((1u << SLOT_BITS) - 1);
}

} // namespace xpute::gpu
