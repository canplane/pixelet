// cpp/conformance/guest.hpp

// The C++ conformance guest, held to the same traces as the others. These
// calls are for tests, not an API. Natively the host hands it a region, a
// clock and its hostcall; as a wasm module the memory starts at 0, the clock
// is `env.now`, the hostcall `env.hostcall`, and its turn is the module's
// entry, `run` (xpute/guest/wasm.hpp).

#pragma once

#include <cstdint>

extern "C" {

typedef double (*xpute_conformance_now)(void);
typedef int32_t (*xpute_conformance_hostcall)(uint32_t at);

#ifdef __wasm__

// Lays the guest out by `seed` (LAYOUTS) and answers the toc's offset.
// `run` answers 0 while a submission waits, -1 otherwise.
std::uint32_t xpute_conformance_boot(std::uint32_t seed);

#else

// `mem` is MEM_BYTES long and 8-aligned. Lays the guest out by `seed`
// (LAYOUTS) and answers the toc's offset.
std::uint32_t xpute_conformance_boot(std::uint8_t *mem, xpute_conformance_now now, xpute_conformance_hostcall hostcall, std::uint32_t seed);

// Answers 0 while a submission waits, -1 otherwise.
double xpute_conformance_run(std::uint8_t *mem, double quota_ms);

#endif
}
