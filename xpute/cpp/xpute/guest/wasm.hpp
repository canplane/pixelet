// cpp/xpute/guest/wasm.hpp

// The guest built as one WebAssembly module: what it reads of the module
// itself — where the linker stopped and its stack ends, the memory it was
// given, the host's clock — and its entry, `intr`, which the floor names as
// a C runtime names `main` and the program defines. The module is linked as
// every module guest is (cpp/Makefile `WASM_LINK`, as xpute-guest wasm
// `link` prints for Rust): its memory imported, its stack first, `intr`
// exported. Nothing here but on wasm.

#pragma once

#include <cstdint>

#ifdef __wasm__

extern "C" {

// Defined by wasm-ld. The end of .rodata, .data and .bss, where a guest may
// begin placing its own ranges: not __heap_base, which a C runtime's sbrk
// starts at, since a guest's heap is where the guest says.
extern const std::uint8_t __data_end;
// The top of the stack: -z stack-size past its start, which --stack-first
// puts at the bottom of the memory.
extern const std::uint8_t __stack_high;

// An import of `env`, the module the host instantiates with.
__attribute__((import_module("env"), import_name("now"))) double xpute_wasm_now(void);

// The entry: a turn of quota_ms, and when the guest wants the next, in ms
// (xpute/guest/sched/quantum.hpp: 0 for the next frame, NO_WAKE for none).
double intr(double quota_ms);
}

namespace xpute::wasm {

// A WebAssembly page, the unit a memory is declared and grown in.
inline constexpr std::uint64_t WASM_PAGE_BYTES = 1 << 16;

// Where the linker stopped, as an address.
inline std::uint32_t data_end() noexcept {
    return static_cast<std::uint32_t>(reinterpret_cast<std::uintptr_t>(&__data_end));
}

// Where the stack ends, as an address: its size, with the stack first.
inline std::uint32_t stack_high() noexcept {
    return static_cast<std::uint32_t>(reinterpret_cast<std::uintptr_t>(&__stack_high));
}

// The bytes of memory the host has grown the module to.
inline std::uint64_t given() noexcept {
    return std::uint64_t{__builtin_wasm_memory_size(0)} * WASM_PAGE_BYTES;
}

// The host's clock, in milliseconds: what a turn's quota is spent against.
inline double now() noexcept {
    return xpute_wasm_now();
}

} // namespace xpute::wasm

#endif
