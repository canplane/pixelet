// cpp/dev/test/mem.cpp

#include "mem.hpp"

#include <vector>

#include "xpute/kit/status/bug.hpp"

namespace xpute::test {

namespace {

constexpr std::uint32_t SZ = 1u << 20;

} // namespace

std::uint32_t alloc(std::uint32_t len) noexcept {
    static std::uint32_t next = 0;
    std::uint32_t at = next;
    next += (len + 7) / 8 * 8;
    ensure(next <= SZ, Errno::enomem);
    return at;
}

} // namespace xpute::test

// The tests' region, as a runtime answers it: all of it the tests' share.
extern "C" std::uint8_t *xpute_mem(std::uint32_t *len, std::uint32_t *share_sz) {
    static std::vector<std::uint64_t> words(xpute::test::SZ / 8);
    if (len != nullptr) *len = xpute::test::SZ;
    if (share_sz != nullptr) *share_sz = xpute::test::SZ;
    return reinterpret_cast<std::uint8_t *>(words.data());
}
