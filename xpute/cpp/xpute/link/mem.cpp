// cpp/xpute/link/mem.cpp

#include "mem.hpp"

#include <atomic>
#include <cstring>

#include "xpute/kit/status/bug.hpp"

namespace xpute::link::mem {

namespace {

#ifdef __wasm__
std::uintptr_t base() noexcept {
    return 0;
}
#else
std::atomic<std::uint8_t *> region_base{nullptr};
std::atomic<std::uint32_t> region_len{0};

// The region's base; nothing reaches the memory before it is set.
std::uintptr_t base() noexcept {
    std::uint8_t *b = region_base.load(std::memory_order_relaxed);
    ensure(b != nullptr, Errno::enotrecoverable);
    return reinterpret_cast<std::uintptr_t>(b);
}
#endif

// The aligned word at `at` as an atomic.
std::atomic_ref<std::uint32_t> atomic(std::uint32_t at) noexcept {
    std::uint8_t *p = bytes(at, 4).data();
    ensure(reinterpret_cast<std::uintptr_t>(p) % std::atomic_ref<std::uint32_t>::required_alignment == 0, Errno::efault, at);
    return std::atomic_ref<std::uint32_t>(*reinterpret_cast<std::uint32_t *>(p));
}

} // namespace

#ifndef __wasm__
void set(Region r) noexcept {
    ensure(r.base != nullptr, Errno::einval);
    std::uint8_t *none = nullptr;
    ensure(region_base.compare_exchange_strong(none, r.base, std::memory_order_relaxed), Errno::ealready);
    region_len.store(r.len, std::memory_order_relaxed);
}

Region region() noexcept {
    return Region{region_base.load(std::memory_order_relaxed), region_len.load(std::memory_order_relaxed)};
}
#endif

std::span<std::uint8_t> bytes(std::uint32_t at, std::uint32_t len) noexcept {
#ifndef __wasm__
    ensure(std::uint64_t{at} + len <= region().len, Errno::efault, at, len);
#endif
    return {reinterpret_cast<std::uint8_t *>(base() + at), len};
}

std::uint32_t detail::off(const void *p) noexcept {
    std::uintptr_t o = reinterpret_cast<std::uintptr_t>(p) - base();
    ensure(o <= UINT32_MAX, Errno::efault);
    return static_cast<std::uint32_t>(o);
}

std::uint32_t word(std::uint32_t at) noexcept {
    std::uint32_t v;
    std::memcpy(&v, bytes(at, 4).data(), 4);
    return v;
}

void set_word(std::uint32_t at, std::uint32_t v) noexcept {
    std::memcpy(bytes(at, 4).data(), &v, 4);
}

std::uint32_t load_acquire(std::uint32_t at) noexcept {
    return atomic(at).load(std::memory_order_acquire);
}

void store_release(std::uint32_t at, std::uint32_t v) noexcept {
    atomic(at).store(v, std::memory_order_release);
}

} // namespace xpute::link::mem
