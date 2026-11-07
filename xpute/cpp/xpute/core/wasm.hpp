// cpp/xpute/core/wasm.hpp

// The core built as one WebAssembly module: what it reads of the module
// itself, what it asks of its runtime (`call`), and its entry, `run`, which the floor names and the program
// defines. Linked as every module core is (cpp/Makefile `WASM_LINK`).

#pragma once

#include <cstdint>

#ifdef __wasm__

extern "C" {

// Defined by wasm-ld. The end of .rodata, .data and .bss, where a core may
// begin placing its own ranges: not __heap_base, which a C runtime's sbrk
// starts at, since a core's heap is where the core says.
extern const std::uint8_t __data_end;
// The top of the stack: -z stack-size past its start, which --stack-first
// puts at the bottom of the memory.
extern const std::uint8_t __stack_high;

// Imports of `env`, the module the runtime instantiates with.
__attribute__((import_module("env"), import_name("now"))) double xpute_wasm_now(void);
__attribute__((import_module("env"), import_name("call"))) std::int32_t xpute_wasm_call(std::uint32_t at);

// The entry: a turn of deadline_ms, and when the core wants the next, in ms
// (0 for the next turn, NO_WAKE in xpute/core/sched/wake.hpp for none).
double run(double deadline_ms);
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

// The bytes of memory the runtime has grown the module to.
inline std::uint64_t given() noexcept {
    return std::uint64_t{__builtin_wasm_memory_size(0)} * WASM_PAGE_BYTES;
}

inline double now() noexcept {
    return xpute_wasm_now();
}

// What the record names, the runtime reads and writes before it returns.
inline std::int32_t call(std::uint32_t at) noexcept {
    return xpute_wasm_call(at);
}

} // namespace xpute::wasm

#endif
