// cpp/xpute/gpu/guest/admit.hpp

// Admission says whether the GPU's host can take another frame now without
// blocking, as `xpute-gpu-guest`'s `admit.rs` says it. Every host holds a frame
// to one rule (`admits`): a guest draws a frame only while fewer of its frames
// than its frame latency are unfinished on the GPU. Every host writes the
// frame latency its program chose and its count into the device's block (spec
// `DEVICE`), and `frame` applies the rule there.

#pragma once

#include <cstdint>
#include <span>

#include "xpute/gpu/link/admit.hpp"

namespace xpute::gpu::admit {

// Keeps the block the guest lends the host, for `frame` and `block` to read.
// Its words have one writer, the host, so the guest keeps the block in place
// for as long as it is lent and never writes it.
void ask(std::span<const std::uint32_t> block) noexcept;

// The first word of the block `ask` kept, `DEVICE::WORDS` long and in place
// while it is lent, its words to load with acquire (`link::mem::load_acquire`);
// null before the guest has asked.
const std::uint32_t *block() noexcept;

// This is the admission of a guest that has the GPU. It admits a frame while
// the block's `STATUS` is `READY` and `admits` holds for the block's
// unfinished frames and its frame latency. It admits none before the guest
// has asked for the block, and none once the device is lost.
bool frame() noexcept;

} // namespace xpute::gpu::admit
