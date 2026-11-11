// cpp/xpute/core/io/frame.spec.hpp
//
// GENERATED from spec/io/frame.json
//
// A frame is one message between the core and its runtime, four 32-bit words; TAG, CMD,
// RESULT and PKT are their indices. The tag is the sender's id, and a reply carries the same
// tag, so the reply reaches the call that sent it. On an import the tag says what the import
// is about instead. The command shares its word with the flags. A reply carries a value or a
// negative errno. A packet, when the frame has one, lies right past the frame's words, and
// PKT says how long it is.
//
// All commands share one number space, `major << 8 | minor`. Majors 0x00 to 0x0F are
// xpute's own (abi/sys.json, abi/fs.json). Every
// other major is the program's to assign. The door applies xpute's own, and hands the program every other command. A runtime hands each frame to
// whatever answers its command's major.
//
// Flag bits 2 and up, and the result word of a submission, are reserved, so that a later
// use of them moves no other word.

#pragma once

#include <cstdint>

namespace xpute::frame {

// The sender's id, which the reply carries back; on an import, what the import is about.
inline constexpr std::uint32_t TAG = 0u;

// The command in the low FLAG_SHIFT bits, and the flags above them.
inline constexpr std::uint32_t CMD = 1u;

// On a reply, a value or a negative errno; on any other frame, 0.
inline constexpr std::uint32_t RESULT = 2u;

// The length in bytes of the frame's XTP packet, which lies right past its words; 0 when it has none.
inline constexpr std::uint32_t PKT = 3u;

// The frame's length in words.
inline constexpr std::uint32_t WORDS = 4u;

// The first major a program's commands may take; the majors below it are xpute's to give.
inline constexpr std::uint32_t PROGRAM_MAJOR = 16u;

// The bit at which the flags start in the CMD word.
inline constexpr std::uint32_t FLAG_SHIFT = 16u;

// A flag that marks the frame as a reply; a frame the core sends without it is an import.
// The door that sends the reply sets it, and a command's handler never does.
inline constexpr std::uint32_t RES = 1u;

// A flag that asks for a reply in every case; without it, only a failure is replied to.
inline constexpr std::uint32_t ACKREQ = 2u;

} // namespace xpute::frame
