// cpp/xpute/conformance/guest.hpp

// The C++ conformance guest: guest.spec.hpp's commands, over a runtime
// written from the contract (ARCHITECTURE.xpute.md) rather than from another
// language's guest, and held to the same traces (spec/xpute/conformance/).
//
// These two calls are how a test reaches it, not an API a program embeds.
// They are C's so that any host can call them, and they come in two forms by
// where the guest runs. Built as a native library, a guest is its memory:
// the host hands it a region and a clock, and a call names the region, so
// two guests never share a thing. Built as a WebAssembly module, the memory
// is the one the module imports, begun at address 0, and the clock is the
// module's `env.now` import. Either way `boot` answers where the directory
// is, the one offset a host learns outside the memory (the contract's rule
// 0): where a guest's range begins is its own, and on WebAssembly it begins
// past what the linker placed.

#pragma once

#include <cstdint>

extern "C" {

typedef double (*xpute_conformance_now)(void);

#ifdef __wasm__

// Lays the guest out past the module's own stack and data and writes its
// directory: what a guest's first turn does, here before it. Answers the
// directory's offset.
std::uint32_t xpute_conformance_boot(void);

// A turn of `quota_ms`, and when the guest wants the next: 0 while a
// submission waits, -1 (none) otherwise.
double xpute_conformance_interrupt(double quota_ms);

#else

// Lays the guest out over `mem`, guest.spec.hpp's MEMORY_BYTES of it and
// 8-aligned, and writes its directory: what a guest's first turn does, here
// before it. `now` is the host's clock. Answers the directory's offset.
std::uint32_t xpute_conformance_boot(std::uint8_t *mem, xpute_conformance_now now);

// A turn of `quota_ms` over the memory `boot` laid out, and when the guest
// wants the next: 0 while a submission waits, -1 (none) otherwise.
double xpute_conformance_interrupt(std::uint8_t *mem, double quota_ms);

#endif
}
