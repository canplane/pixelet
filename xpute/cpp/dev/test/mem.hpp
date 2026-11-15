// cpp/dev/test/mem.hpp

// The guest's memory for the tests: what a test lays its frames and packets
// in, taken from the process's own (xpute/link/mem.hpp).

#pragma once

#include <cstdint>

namespace xpute::test {

// Takes `len` zeroed bytes, on a word, given to no other test, for the run,
// and answers their address.
std::uint64_t alloc(std::uint32_t len) noexcept;

} // namespace xpute::test
