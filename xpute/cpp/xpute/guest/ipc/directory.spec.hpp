// cpp/xpute/guest/ipc/directory.spec.hpp
//
// GENERATED from spec/ipc/directory.json
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

// What an entry's value is. 3 to 5 are retired and given to nothing else: they were the
// numbers of a host's scheduling policy, which the program now injects when the host
// boots (the host's sched/policy.ts).
namespace key {

// Where the submission ring's words begin.
inline constexpr std::uint32_t SQ = 1u;

// Where the completion ring's words begin.
inline constexpr std::uint32_t CQ = 2u;

// Where the system-call stream's words begin (ipc/sys.json).
inline constexpr std::uint32_t SYS = 6u;

// The bit every key a program numbers for itself carries: xpute gives none of
// them a meaning, and a host that is not the program's passes over them.
inline constexpr std::uint32_t PROGRAM = 2147483648u;
} // namespace key

} // namespace xpute::directory
