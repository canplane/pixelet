// cpp/xpute/gpu/guest/admit.cpp

#include "admit.hpp"

#include "xpute/link/mem.hpp"
#include "xpute/gpu/link/api.spec.hpp"

namespace xpute::gpu::admit {

namespace {

// The first word of the device's block the guest last lent
// (gpu/wit/gpu.wit `device.lost`).
const std::uint32_t *kept = nullptr;

} // namespace

void ask(std::span<const std::uint32_t> block) noexcept {
    kept = block.data();
}

const std::uint32_t *block() noexcept {
    return kept;
}

bool frame() noexcept {
    if (kept == nullptr) return false;
    // A word of the block `ask` kept, which stays live and in place; the host
    // stores into it only atomically, and writes the frame latency before
    // `STATUS` and never after.
    auto word = [](std::size_t k) { return link::mem::load_acquire(link::mem::off(kept[k])); };
    return word(DEVICE::STATUS_AT) == STATUS::READY && admits(word(DEVICE::UNFINISHED_AT), word(DEVICE::LATENCY_AT));
}

} // namespace xpute::gpu::admit
