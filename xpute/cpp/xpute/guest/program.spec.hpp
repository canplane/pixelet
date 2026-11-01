// cpp/xpute/guest/program.spec.hpp
//
// GENERATED from spec/program.json
//
// A program's defaults (xpute-guest program.rs `program!`, cpp/xpute/guest/program.hpp
// `XPUTE_PROGRAM`): the port its host speaks as, what its range holds and the stack it
// is linked with, each where the program says nothing else; and the files it may have
// open. The range's offsets follow from the counts, as each guest end computes them.

#pragma once

#include <cstdint>

namespace xpute::program {

// The port a program's host speaks as unless it says otherwise, and the one a handler's
// events are raised for: any but the host's, 0 (ipc/frame.json HOST).
inline constexpr std::uint32_t PORT = 1u;

// Each ring's slots: how many commands can wait, and replies and events be left,
// between two turns.
inline constexpr std::uint32_t CAP = 64u;

// A slot's bytes: the most a command's packet, or an event's, may be.
inline constexpr std::uint32_t SLOT_BYTES = 1024u;

// The hostcalls' stream, in words: what one turn may ask the host to run after it.
inline constexpr std::uint32_t SYS_WORDS = 4096u;

// The events one command may raise beside its reply.
inline constexpr std::uint32_t MOST_EVENTS = 4u;

// The stack a module is linked with (`-z stack-size`), placed first.
inline constexpr std::uint32_t STACK_BYTES = 1048576u;

// The slots of the table its open files are held in, each a read awaited: one fewer may
// be open at once, and a read past them is refused with EMFILE.
inline constexpr std::uint32_t MOST_FILES = 64u;

} // namespace xpute::program
