// cpp/xpute/conformance/guest.hpp

// The C++ conformance guest: guest.spec.hpp's commands, over a runtime
// written from the contract (ARCHITECTURE.xpute.md) rather than from another
// language's guest, and held to the same traces
// (spec/xpute/conformance/).
//
// These two calls are how a test reaches it, not an API a program embeds.
// They are C's so that any host can call them. A guest is its memory:
// everything it keeps lies in the range the host reserved, so a call names
// the memory and two guests never share a thing.

#pragma once

#include <cstdint>

extern "C" {

typedef double (*xpute_conformance_now)(void);

// Lays the guest out over `mem`, guest.spec.hpp's MEMORY_BYTES of it and
// 8-aligned, and writes its directory at DIRECTORY_AT: what a guest's first
// turn does, here before it. `now` is the host's clock.
void xpute_conformance_boot(std::uint8_t *mem, xpute_conformance_now now);

// A turn of `quota_ms`, and when the guest wants the next: 0 while a
// submission waits, -1 (none) otherwise.
double xpute_conformance_interrupt(std::uint8_t *mem, double quota_ms);
}
