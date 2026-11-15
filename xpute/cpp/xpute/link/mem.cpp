// cpp/xpute/link/mem.cpp

#include "mem.hpp"

#include <atomic>
#include <cstring>

#include "xpute/kit/status/bug.hpp"

namespace xpute::link::mem {

namespace {

// The byte at `at`, as a pointer.
std::uint8_t *ptr(std::uint64_t at) noexcept {
    ensure(at <= UINTPTR_MAX, Errno::efault);
    return reinterpret_cast<std::uint8_t *>(static_cast<std::uintptr_t>(at));
}

// The aligned word at `at` as an atomic.
std::atomic_ref<std::uint32_t> atomic(std::uint64_t at) noexcept {
    std::uint8_t *p = ptr(at);
    ensure(reinterpret_cast<std::uintptr_t>(p) % std::atomic_ref<std::uint32_t>::required_alignment == 0, Errno::efault, at);
    return std::atomic_ref<std::uint32_t>(*reinterpret_cast<std::uint32_t *>(p));
}

} // namespace

std::span<std::uint8_t> bytes(std::uint64_t at, std::uint32_t len) noexcept {
    return {ptr(at), len};
}

std::uint64_t detail::addr(const void *p) noexcept {
    return reinterpret_cast<std::uintptr_t>(p);
}

std::uint32_t word(std::uint64_t at) noexcept {
    std::uint32_t v;
    std::memcpy(&v, ptr(at), 4);
    return v;
}

void set_word(std::uint64_t at, std::uint32_t v) noexcept {
    std::memcpy(ptr(at), &v, 4);
}

std::uint32_t load_acquire(std::uint64_t at) noexcept {
    return atomic(at).load(std::memory_order_acquire);
}

void store_release(std::uint64_t at, std::uint32_t v) noexcept {
    atomic(at).store(v, std::memory_order_release);
}

} // namespace xpute::link::mem
