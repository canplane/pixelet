// cpp/dev/test/mem.cpp

#include "mem.hpp"

#include <cstdint>
#include <vector>

#include "xpute/link/mem.hpp"

namespace xpute::test {

std::uint64_t alloc(std::uint32_t len) noexcept {
    // Each block lives as long as the run, so no test's memory moves.
    auto *words = new std::vector<std::uint64_t>((len + 7) / 8 + 1);
    return link::mem::addr((*words)[0]);
}

} // namespace xpute::test
