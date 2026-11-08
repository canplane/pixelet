// cpp/xpute/core/wasm.hpp

// This is the core built as one WebAssembly module: what it reads of the module
// itself, what it asks of its runtime (`call`), and its entry, `run`, which
// xpute names and the program defines. It is linked as every module core is
// (cpp/Makefile `WASM_LINK`).

#pragma once

#include <cstdint>

#ifdef __wasm__

extern "C" {

// The end of .rodata, .data and .bss, defined by wasm-ld. A core may begin
// placing its own ranges here. It is not __heap_base, which lies eight bytes
// further on: that is where a C runtime's sbrk starts, and a core places its
// heap where it says.
extern const std::uint8_t __data_end;
// The top of the stack, defined by wasm-ld: -z stack-size bytes past the
// stack's start, which --stack-first puts at the bottom of the memory.
extern const std::uint8_t __stack_high;

// Imports of `env`, the module the runtime instantiates the core with.
__attribute__((import_module("env"), import_name("now"))) double xpute_wasm_now(void);
__attribute__((import_module("env"), import_name("call"))) std::int32_t xpute_wasm_call(std::uint32_t at);

// `run` runs one turn that may last `deadline_ms`, and answers in how many ms
// the core wants the next: 0 for as soon as the scheduling policy allows,
// NO_WAKE (xpute/core/sched/wake.hpp) for none.
double run(double deadline_ms);
}

namespace xpute::wasm {

// This is the size of a WebAssembly page, the unit a memory is declared and
// grown in.
inline constexpr std::uint64_t WASM_PAGE_BYTES = 1 << 16;

inline std::uint32_t data_end() noexcept {
    return static_cast<std::uint32_t>(reinterpret_cast<std::uintptr_t>(&__data_end));
}

// The stack's size, since the stack starts at address 0.
inline std::uint32_t stack_high() noexcept {
    return static_cast<std::uint32_t>(reinterpret_cast<std::uintptr_t>(&__stack_high));
}

// This is the size in bytes of the memory the runtime has given the module.
inline std::uint64_t given() noexcept {
    return std::uint64_t{__builtin_wasm_memory_size(0)} * WASM_PAGE_BYTES;
}

inline double now() noexcept {
    return xpute_wasm_now();
}

// The runtime reads and writes the memory the frame names before `call`
// returns.
inline std::int32_t call(std::uint32_t at) noexcept {
    return xpute_wasm_call(at);
}

} // namespace xpute::wasm

#endif
