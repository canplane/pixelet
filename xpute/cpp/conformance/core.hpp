// cpp/conformance/core.hpp

// The C++ conformance core is held to the same traces as the other
// conformance cores. These calls are for tests, not an API. Natively, the
// runtime hands the core a region, a clock and its call. As a wasm module, its
// memory starts at 0, the clock is `env.now`, the call is `env.call`, and its
// turn is the module's entry, `run` (xpute/core/wasm.hpp).

#pragma once

#include <cstdint>

extern "C" {

typedef double (*xpute_conformance_now)(void);
typedef int32_t (*xpute_conformance_call)(uint32_t at);

#ifdef __wasm__

// Lays out the core by `seed` (LAYOUTS), writes the front's header as a
// runtime does for a machine, and answers the header's offset. The module's
// `run` answers 0 while a submission waits, and -1 otherwise.
std::uint32_t xpute_conformance_boot(std::uint32_t seed);

#else

// Lays out the core by `seed` (LAYOUTS) in `mem`, which must be MEM_BYTES
// long and 8-aligned, writes the front's header as a runtime does for a
// machine, and answers the header's offset.
std::uint32_t xpute_conformance_boot(std::uint8_t *mem, xpute_conformance_now now, xpute_conformance_call call, std::uint32_t seed);

// Answers 0 while a submission waits, -1 otherwise.
double xpute_conformance_run(std::uint8_t *mem, double deadline_ms);

#endif
}
