// cpp/conformance/guest.hpp

// The C++ conformance guest, held to the same traces as the others. These
// calls are for tests, not an API. Natively the host hands it a region and a
// clock; as a wasm module the memory starts at 0, the clock is `env.now`,
// and its turn is the module's entry, `intr` (xpute/guest/wasm.hpp).

#pragma once

#include <cstdint>

extern "C" {

typedef double (*xpute_conformance_now)(void);

#ifdef __wasm__

// Answers the directory's offset. `intr` answers 0 while a submission
// waits, -1 otherwise.
std::uint32_t xpute_conformance_boot(void);

#else

// `mem` is MEMORY_BYTES long and 8-aligned. Answers the directory's offset.
std::uint32_t xpute_conformance_boot(std::uint8_t *mem, xpute_conformance_now now);

// Answers 0 while a submission waits, -1 otherwise.
double xpute_conformance_intr(std::uint8_t *mem, double quota_ms);

#endif
}
