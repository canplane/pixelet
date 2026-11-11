// cpp/xpute/core/wasm.hpp

// This is the core built as one WebAssembly module: what it reads of the module
// itself, the host it reaches its runtime through (`host`), and its entry,
// `run`, which xpute names and the program defines. It is linked as every
// module core is (cpp/Makefile `WASM_LINK`).

#pragma once

#include <cstdint>

#include "xpute/core/host.hpp"

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
__attribute__((import_module("env"), import_name("receive"))) std::int32_t xpute_wasm_receive(std::uint32_t at, std::uint32_t cap);
__attribute__((import_module("env"), import_name("send"))) void xpute_wasm_send(std::uint32_t at, std::uint32_t named_at, std::uint32_t named_len);
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
inline std::uint64_t mem_sz() noexcept {
    return std::uint64_t{__builtin_wasm_memory_size(0)} * WASM_PAGE_BYTES;
}

inline double now() noexcept {
    return xpute_wasm_now();
}

// The host the module's runtime gives as its imports (xpute/core/host.hpp).
inline Host host() noexcept {
    return Host{.receive = xpute_wasm_receive, .send = xpute_wasm_send, .call = xpute_wasm_call};
}

} // namespace xpute::wasm

#endif
