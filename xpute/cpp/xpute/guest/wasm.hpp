// cpp/xpute/guest/wasm.hpp

// The guest built as one WebAssembly module: what it reads of the module
// itself, what it asks of its host (`hostcall`), and its entry, `run`, which the floor names and the program
// defines. Linked as every module guest is (cpp/Makefile `WASM_LINK`).

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

// Imports of `env`, the module the host instantiates with.
__attribute__((import_module("env"), import_name("now"))) double xpute_wasm_now(void);
__attribute__((import_module("env"), import_name("hostcall"))) std::int32_t xpute_wasm_hostcall(std::uint32_t at);

// The entry: a turn of quota_ms, and when the guest wants the next, in ms
// (0 for the next turn, NO_WAKE in xpute/guest/sched/wake.hpp for none).
double run(double quota_ms);
}

namespace xpute::wasm {

// A WebAssembly page, the unit a memory is declared and grown in.
inline constexpr std::uint64_t WASM_PAGE_BYTES = 1 << 16;

inline std::uint32_t data_end() noexcept {
    return static_cast<std::uint32_t>(reinterpret_cast<std::uintptr_t>(&__data_end));
}

// Its size, with the stack first.
inline std::uint32_t stack_high() noexcept {
    return static_cast<std::uint32_t>(reinterpret_cast<std::uintptr_t>(&__stack_high));
}

// The bytes of memory the host has grown the module to.
inline std::uint64_t given() noexcept {
    return std::uint64_t{__builtin_wasm_memory_size(0)} * WASM_PAGE_BYTES;
}

inline double now() noexcept {
    return xpute_wasm_now();
}

// What the record names, the host reads and writes before it returns.
inline std::int32_t hostcall(std::uint32_t at) noexcept {
    return xpute_wasm_hostcall(at);
}

} // namespace xpute::wasm

#endif
