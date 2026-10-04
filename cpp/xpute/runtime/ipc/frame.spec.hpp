// cpp/xpute/runtime/ipc/frame.spec.hpp
//
// GENERATED from spec/xpute/ipc/frame.json — do not edit.
//
// A message in a ring (ring.json): four words, a frame. The submitter's tag comes back on
// the reply; the command shares its word with the flags; a reply carries a value or a
// negative errno; and a packet, where there is one, lies in the ring's payload slot.
//
// Reserved for later, so taking them moves no word: the tag's high bits (a sender id for
// routing), flag bits 2 and up, and the result word on a submission.

#pragma once

#include <cstdint>

namespace xpute::frame {

// The submitter's, echoed on the reply; a signal's subject.
inline constexpr std::uint32_t TAG = 0u;

// The command in the low FLAG_SHIFT bits, the flags above them.
inline constexpr std::uint32_t CMD = 1u;

// On a reply, a value or a negative errno; else 0.
inline constexpr std::uint32_t RESULT = 2u;

// Where its XTP packet is, as the transport names it; 0 for none.
inline constexpr std::uint32_t PACKET = 3u;

// Words a frame takes.
inline constexpr std::uint32_t WORDS = 4u;

// Where the flags start in the CMD word.
inline constexpr std::uint32_t FLAG_SHIFT = 16u;

// A flag: a reply, as against a signal. Set by the sender, never by a handler.
inline constexpr std::uint32_t RES = 1u;

// A flag: always reply; without it, only a failure is replied to.
inline constexpr std::uint32_t ACKREQ = 2u;

} // namespace xpute::frame
