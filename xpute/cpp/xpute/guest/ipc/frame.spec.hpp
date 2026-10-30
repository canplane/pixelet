// cpp/xpute/guest/ipc/frame.spec.hpp
//
// GENERATED from spec/ipc/frame.json
//
// A frame in a ring (ring.json): four words. The tag names the port at the guest's
// far side in its high bits — who sent a frame the guest takes, who is to have one it
// posts — and below them an id, so a reply, which keeps its submission's tag, goes back
// to its sender; the command shares its word with the flags; a reply carries a value or
// a negative errno; and a packet, where there is one, lies in the ring's payload slot.
//
// The host is port 0, and a command it sends is xpute's own (abi/cmd.json); every
// other number is the program's to give, and so is what its commands mean. The host
// hands a frame to the port it names where one is attached, and drops it where none
// is.
//
// Reserved for later, so taking them moves no word: flag bits 2 and up, and the result
// word on a submission.

#pragma once

#include <cstdint>

namespace xpute::frame {

// The far port from PORT_SHIFT up; below it the sender's id, echoed on the
// reply, or an event's subject.
inline constexpr std::uint32_t TAG = 0u;

// The command in the low FLAG_SHIFT bits, the flags above them.
inline constexpr std::uint32_t CMD = 1u;

// On a reply, a value or a negative errno; else 0.
inline constexpr std::uint32_t RESULT = 2u;

// Where its XTP packet is, as the transport names it; 0 for none.
inline constexpr std::uint32_t PKT = 3u;

// Words a frame takes.
inline constexpr std::uint32_t WORDS = 4u;

// Where the far port starts in the TAG word: one of 256.
inline constexpr std::uint32_t PORT_SHIFT = 24u;

// The host's port, whose commands are xpute's own.
inline constexpr std::uint32_t HOST = 0u;

// Where the flags start in the CMD word.
inline constexpr std::uint32_t FLAG_SHIFT = 16u;

// A flag: a reply, as against an event. Set by the sender, never by a handler.
inline constexpr std::uint32_t RES = 1u;

// A flag: always reply; without it, only a failure is replied to.
inline constexpr std::uint32_t ACKREQ = 2u;

} // namespace xpute::frame
