// cpp/dev/test/mem.hpp

// The guest's memory for the tests, which are its runtime here
// (xpute/link/mem.hpp `region`): one region for the run, reserved at its
// first use, all of it the tests' share.

#pragma once

#include <cstdint>

namespace xpute::test {

// Takes `len` bytes of the guest's memory, given to no other test, and
// answers where they lie.
std::uint32_t alloc(std::uint32_t len) noexcept;

} // namespace xpute::test
