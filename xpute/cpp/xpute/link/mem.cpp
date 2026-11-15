// cpp/xpute/link/mem.cpp

#include "mem.hpp"

#include <atomic>
#include <cstring>

#include "xpute/kit/status/bug.hpp"

// The runtime's: the memory's first byte, its size through `len` and the
// program's share of it through `share_sz`. A module imports it under `env`
// from its page's runtime.
#ifdef __wasm__
extern "C" __attribute__((import_module("env"), import_name("xpute_mem"))) std::uint8_t *xpute_mem(std::uint32_t *len, std::uint32_t *share_sz);
#else
extern "C" std::uint8_t *xpute_mem(std::uint32_t *len, std::uint32_t *share_sz);
#endif

#ifdef __wasm__
// Where the module's data ends, as its linker places it.
extern "C" unsigned char __heap_base;
#endif

namespace xpute::link::mem {

namespace {

constexpr std::uintptr_t page_up(std::uintptr_t at) noexcept {
    return (at + PAGE_SZ - 1) & ~std::uintptr_t{PAGE_SZ - 1};
}

// The memory's parts, once read; a share of 0 is unread. No constructor of a
// static runs for them: a module's would link the libc's, which reach WASI.
Parts kept_parts{0, 0, 0};

#ifdef __wasm__
std::uintptr_t base() noexcept {
    return 0;
}
#else
// The region's base and size, once read: the region never moves once
// reserved.
std::atomic<std::uintptr_t> kept_base{0};
std::atomic<std::uint32_t> kept_len{0};

// The region's base and size. The first call asks the runtime; two first
// calls at once read the same region.
std::pair<std::uintptr_t, std::uint32_t> kept() noexcept {
    std::uintptr_t b = kept_base.load(std::memory_order_acquire);
    if (b != 0) return {b, kept_len.load(std::memory_order_relaxed)};
    Region r = region();
    b = reinterpret_cast<std::uintptr_t>(r.base);
    kept_len.store(r.len, std::memory_order_relaxed);
    kept_base.store(b, std::memory_order_release);
    return {b, r.len};
}

std::uintptr_t base() noexcept {
    return kept().first;
}
#endif

// The aligned word at `at` as an atomic.
std::atomic_ref<std::uint32_t> atomic(std::uint32_t at) noexcept {
    std::uint8_t *p = bytes(at, 4).data();
    ensure(reinterpret_cast<std::uintptr_t>(p) % std::atomic_ref<std::uint32_t>::required_alignment == 0, Errno::efault, at);
    return std::atomic_ref<std::uint32_t>(*reinterpret_cast<std::uint32_t *>(p));
}

} // namespace

Region region() noexcept {
    Region r{};
    r.base = xpute_mem(&r.len, &r.share_sz);
    return r;
}

const Parts &parts() noexcept {
    if (kept_parts.share == 0) {
        Region r = region();
#ifdef __wasm__
        std::uintptr_t share = page_up(reinterpret_cast<std::uintptr_t>(&__heap_base));
#else
        std::uintptr_t share = reinterpret_cast<std::uintptr_t>(r.base);
#endif
        std::uintptr_t heap = page_up(share + r.share_sz), end = reinterpret_cast<std::uintptr_t>(r.base) + r.len;
        ensure(heap <= end, Errno::enomem, heap, end);
        kept_parts = Parts{share, heap, end};
    }
    return kept_parts;
}

std::span<std::uint8_t> bytes(std::uint32_t at, std::uint32_t len) noexcept {
#ifndef __wasm__
    ensure(std::uint64_t{at} + len <= kept().second, Errno::efault, at, len);
#endif
    return {reinterpret_cast<std::uint8_t *>(base() + at), len};
}

std::uint32_t detail::off(const void *p) noexcept {
    std::uintptr_t o = reinterpret_cast<std::uintptr_t>(p) - base();
    ensure(o <= UINT32_MAX, Errno::efault);
    return static_cast<std::uint32_t>(o);
}

void detail::held(const void *p, std::size_t len) noexcept {
    const Parts &m = parts();
    std::uintptr_t at = reinterpret_cast<std::uintptr_t>(p);
    ensure(at >= m.heap && at <= m.end && len <= m.end - at, Errno::efault, at, len);
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
