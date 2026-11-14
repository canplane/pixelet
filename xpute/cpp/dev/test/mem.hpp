// cpp/dev/test/mem.hpp

// The guest's memory for the tests, as a native guest sets its own
// (xpute/link/mem.hpp `set`): one region for the run, set at its first use.

#pragma once

#include <cstdint>

namespace xpute::test {

// Takes `len` bytes of the guest's memory, given to no other test, and
// answers where they lie.
std::uint32_t alloc(std::uint32_t len) noexcept;

} // namespace xpute::test
