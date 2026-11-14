// cpp/dev/test/mem.cpp

#include "mem.hpp"

#include <vector>

#include "xpute/kit/status/bug.hpp"
#include "xpute/link/mem.hpp"

namespace xpute::test {

std::uint32_t alloc(std::uint32_t len) noexcept {
    constexpr std::uint32_t SZ = 1u << 20;
    static std::vector<std::uint64_t> words = [] {
        std::vector<std::uint64_t> w(SZ / 8);
        link::mem::set({reinterpret_cast<std::uint8_t *>(w.data()), SZ});
        return w;
    }();
    static std::uint32_t next = 0;
    std::uint32_t at = next;
    next += (len + 7) / 8 * 8;
    ensure(next <= SZ, Errno::enomem);
    return at;
}

} // namespace xpute::test
