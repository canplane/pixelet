// cpp/xpute/link/frame.spec.hpp
//
// GENERATED from link/spec/frame.json5 (xpute-gen data.rs)
//
// A frame is one method called across the link, four 32-bit words; TAG, CMD, RESULT and
// PKT are their indices, little-endian. The tag is the request, unique among the sender's
// pending ones, and a later answer carries it back. The command is the RES bit, the handle
// the callee issued above METHOD_BITS, and the method's place among its object's below
// them. An answer carries 0 or a negative errno, its value in its packet. The method's args, a packet in the Canonical ABI's layout,
// lie right past the words, and PKT says how long they are (link/wit/types.wit). ROOT
// is the handle every side implements, OPEN, CANCEL and OPEN_AT its methods, DROP the method
// every handle has, NAMED marks an interface's handle, SLOT_BITS splits an issued handle
// into its slot and its generation, and TRAP and TRAP_AT are a native guest's stop.

#pragma once

#include <cstddef>
#include <cstdint>

namespace xpute::link::frame {

// The request, which a later answer carries back.
inline constexpr std::uint32_t TAG = 0u;

// RES, the handle and the method.
inline constexpr std::uint32_t CMD = 1u;

// On an answer, 0 or a negative errno: the call refused, by the link or by the method as a caller's bug (EINVAL), or a method's failure where its WIT has no result; on a request, 0.
inline constexpr std::uint32_t RESULT = 2u;

// The length in bytes of the frame's packet, which lies right past its words; 0 when it has none.
inline constexpr std::uint32_t PKT = 3u;

// The frame's length in words.
inline constexpr std::uint32_t WORDS = 4u;

// The bit of the command that marks an answer.
inline constexpr std::uint32_t RES = 2147483648u;

// The bits of the command below the handle, which hold the method.
inline constexpr std::uint32_t METHOD_BITS = 8u;

// The root's handle, which every side implements.
inline constexpr std::uint32_t ROOT = 0u;

// The root's method that opens an object by its URI.
inline constexpr std::uint32_t OPEN = 0u;

// The root's method that cancels a call the sender has pending, named by its tag, the one
// arg. A call the callee can stop it stops, and answers ECANCELED; any other runs on. Either
// way it settles by its own answer, and what it lent stays lent until then.
inline constexpr std::uint32_t CANCEL = 1u;

// The root's method that opens an object under a handle the sender has issued for it:
// the handle, which of the receiver's mounted openers opens it, by its place among them, and
// the URI. A runtime that stands in for a UI on another thread answers the guest's open at
// once, issuing the handle, and carries the open to the UI's end as this. It takes no
// answer.
inline constexpr std::uint32_t OPEN_AT = 2u;

// The method every handle has, which lets it go.
inline constexpr std::uint32_t DROP = 255u;

// The handle's top bit, which marks it named: an interface's, its bits below the hash of
// the interface's name the generator writes (xpute-link-gen wit.rs). An interface with
// functions of its own is mounted under it with no open; one of resources alone names its
// openers by it. An issued handle has it clear.
inline constexpr std::uint32_t NAMED = 4194304u;

// The bits of a handle that name its slot, below its generation.
inline constexpr std::uint32_t SLOT_BITS = 20u;

// What a native guest's export answers where the guest stopped, caught at the export so nothing leaves into its caller: as a module's trap. The guest is over, and its host enters it no more.
inline constexpr std::int32_t TRAP = -2147483648;

// What a native guest's link_init answers in its layout's place where the guest stopped, as TRAP is for its other exports: an odd address, where no layout lies.
inline constexpr std::uint64_t TRAP_AT = 4294967295ull;

// The words of the link's layout, which a guest's link_init lays and answers the address
// of: the inbox's address, a u64 in two words, its low word first, then a frame's cap, then a
// word unused, so the inbox, which follows the layout's words, lies on 8 bytes. The outbox
// follows the inbox.
namespace LAYOUT {

inline constexpr std::size_t INBOX_AT = 0u;

inline constexpr std::size_t CAP_AT = 2u;

inline constexpr std::size_t PAD_AT = 3u;

inline constexpr std::size_t WORDS = 4u;
} // namespace LAYOUT

} // namespace xpute::link::frame
