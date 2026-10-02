// cpp/xpute/conformance/guest.hpp

// The C++ conformance guest, held to the same traces as the others. These
// calls are for tests, not an API. Natively the host hands it a region and a
// clock; as a wasm module the memory starts at 0 and the clock is `env.now`.

#pragma once

#include <cstdint>

extern "C" {

typedef double (*xpute_conformance_now)(void);

#ifdef __wasm__

// Answers the directory's offset.
std::uint32_t xpute_conformance_boot(void);

// Answers 0 while a submission waits, -1 otherwise.
double xpute_conformance_interrupt(double quota_ms);

#else

// `mem` is MEMORY_BYTES long and 8-aligned. Answers the directory's offset.
std::uint32_t xpute_conformance_boot(std::uint8_t *mem, xpute_conformance_now now);

// Answers 0 while a submission waits, -1 otherwise.
double xpute_conformance_interrupt(std::uint8_t *mem, double quota_ms);

#endif
}
