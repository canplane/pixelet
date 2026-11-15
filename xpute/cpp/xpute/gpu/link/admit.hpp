// cpp/xpute/gpu/link/admit.hpp

// Admission says whether the GPU's host can take another frame now without
// blocking, as `xpute-gpu-link`'s `admit.rs` says it. Every host holds a frame
// to one rule: a guest draws a frame only while fewer of its frames than its
// frame latency are unfinished on the GPU.

#pragma once

#include <cstdint>

namespace xpute::gpu::admit {

constexpr bool admits(std::uint32_t unfinished, std::uint32_t latency) noexcept {
    return unfinished < latency;
}

} // namespace xpute::gpu::admit
