// cpp/xpute/guest/mem.cpp

#include "mem.hpp"

namespace xpute::mem {

namespace {
// Two words rather than a `Memory`, so they are set before any constructor
// runs and a static that allocates finds them.
std::uintptr_t core_base = 0;
std::uintptr_t core_bytes = UINTPTR_MAX;
} // namespace

void set(std::uintptr_t base, std::uintptr_t bytes) noexcept {
    core_base = base;
    core_bytes = bytes;
}

Memory core() noexcept {
    return Memory(core_base, core_bytes);
}

} // namespace xpute::mem
