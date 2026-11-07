// cpp/xpute/core/ipc/frame.spec.hpp
//
// GENERATED from spec/ipc/frame.json
//
// A frame in a ring (ring.json): four words. The tag is the sender's id, which a reply
// keeps, so it goes back to the call that sent it, or an import's subject; the command
// shares its word with the flags; a reply carries a value or a negative errno; and a
// packet, where there is one, lies in the ring's payload slot.
//
// Commands share one number space, `major << 8 | minor`: majors 0x00 to 0x0F are
// xpute's own (abi/cmd.json, abi/fs.json), and every other is the program's to give,
// and so is what its commands mean. A runtime hands a frame to what answers its command's
// major.
//
// Reserved for later, so taking them moves no word: flag bits 2 and up, and the result
// word on a submission.

#pragma once

#include <cstdint>

namespace xpute::frame {

// The sender's id, echoed on the reply, or an import's subject.
inline constexpr std::uint32_t TAG = 0u;

// The command in the low FLAG_SHIFT bits, the flags above them.
inline constexpr std::uint32_t CMD = 1u;

// On a reply, a value or a negative errno; else 0.
inline constexpr std::uint32_t RESULT = 2u;

// Where its XTP packet is, as the transport names it; 0 for none.
inline constexpr std::uint32_t PKT = 3u;

// Words a frame takes.
inline constexpr std::uint32_t WORDS = 4u;

// The first major a program's commands take; those below it are xpute's own.
inline constexpr std::uint32_t PROGRAM_MAJOR = 16u;

// Where the flags start in the CMD word.
inline constexpr std::uint32_t FLAG_SHIFT = 16u;

// A flag: a reply's frame, as against an import's. Set by the sender, never by a handler.
inline constexpr std::uint32_t RES = 1u;

// A flag: always reply; without it, only a failure is replied to.
inline constexpr std::uint32_t ACKREQ = 2u;

} // namespace xpute::frame
