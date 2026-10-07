// cpp/guest/ipc/directory.spec.hpp
//
// GENERATED from spec/ipc/directory.json — do not edit.
//
// The directory: where a host finds what its guest laid out in the memory. The guest's
// first turn writes it, and the host reads it once, at the one offset it knows outside
// the memory (xpute-guest ipc/directory.rs, and the contract's rule 0 in
// ARCHITECTURE.md).
//
// Its words: MAGIC, VERSION, how many entries follow, and a word kept at 0; then that
// many entries, a key and a value each, no key twice. A reader looks up the keys it
// knows and passes over the rest, the way a process reads its ELF auxiliary vector, so
// a directory that grows an entry is still read by a host that predates it.

#pragma once

#include <cstdint>

namespace xpute::directory {

// "XPUT" as four bytes read little-endian: what tells a directory from memory nothing wrote.
inline constexpr std::uint32_t MAGIC = 1414877272u;

// Raised when a key comes to mean something else. A new key does not raise it.
inline constexpr std::uint32_t VERSION = 1u;

// Words before the first entry.
inline constexpr std::uint32_t HEAD = 4u;

// What a quantum policy's number is multiplied by to cross as a word: it crosses in thousandths.
inline constexpr std::uint32_t QUANTUM_SCALE = 1000u;

// What an entry's value is.
namespace key {

// Where the submission ring's words begin.
inline constexpr std::uint32_t SUBMISSION = 1u;

// Where the completion ring's words begin.
inline constexpr std::uint32_t COMPLETION = 2u;

// The quantum policy's margin share (sched/quantum.rs), in QUANTUM_SCALE.
inline constexpr std::uint32_t MARGIN_SHARE = 3u;

// The quantum policy's batch frames, in QUANTUM_SCALE.
inline constexpr std::uint32_t BATCH_FRAMES = 4u;

// The quantum policy's settle time, in QUANTUM_SCALE of a millisecond.
inline constexpr std::uint32_t SETTLE_MS = 5u;

// Where the system-call stream's words begin (ipc/sys.json).
inline constexpr std::uint32_t SYS = 6u;

// The bit every key a program numbers for itself carries: xpute gives none of
// them a meaning, and a host that is not the program's passes over them.
inline constexpr std::uint32_t PROGRAM = 2147483648u;
} // namespace key

} // namespace xpute::directory
