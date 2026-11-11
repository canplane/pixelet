// cpp/xpute/core/io/frame.hpp

// A frame's four words are laid out by spec/io/frame.json, and its packet,
// when it has one, lies right past them. The door keeps two frames, the one
// the core receives into and the one it sends from (door.hpp).

#pragma once

#include <cstdint>
#include <span>

#include "xpute/kit/status/errno.spec.hpp"
#include "xpute/kit/status/error.hpp"
#include "xpute/core/mem.hpp"
#include "frame.spec.hpp"

namespace xpute::frame {

inline constexpr std::uint32_t CMD_MASK = (1u << FLAG_SHIFT) - 1;

// The bytes a frame's words take, before its packet.
inline constexpr std::uint32_t WORD_BYTES = WORDS * 4;

constexpr std::uint32_t cmd_word(std::uint32_t cmd, std::uint32_t flags) noexcept {
    return flags << FLAG_SHIFT | (cmd & CMD_MASK);
}

constexpr std::uint32_t cmd_of(std::uint32_t word) noexcept {
    return word & CMD_MASK;
}

constexpr std::uint32_t flags_of(std::uint32_t word) noexcept {
    return word >> FLAG_SHIFT;
}

} // namespace xpute::frame

namespace xpute {

// A frame's four words as read, with its command word split into the command
// and its flags.
struct Frame {
    std::uint32_t tag;
    std::uint32_t cmd;
    std::uint32_t flags;
    std::int32_t result;
    // The length in bytes of the frame's packet, 0 for none.
    std::uint32_t pkt_len;

    bool operator==(const Frame &) const = default;

    // The frame's words at `at`.
    static Frame at(const Memory &mem, std::uint32_t at) noexcept {
        auto word = [&](std::uint32_t k) { return mem.word(at + 4 * k); };
        std::uint32_t cmd = word(frame::CMD);
        return Frame{
            .tag = word(frame::TAG),
            .cmd = frame::cmd_of(cmd),
            .flags = frame::flags_of(cmd),
            .result = static_cast<std::int32_t>(word(frame::RESULT)),
            .pkt_len = word(frame::PKT),
        };
    }

    // Writes the frame's words at `at`.
    void lay(const Memory &mem, std::uint32_t at) const noexcept {
        mem.set_word(at + 4 * frame::TAG, tag);
        mem.set_word(at + 4 * frame::CMD, frame::cmd_word(cmd, flags));
        mem.set_word(at + 4 * frame::RESULT, static_cast<std::uint32_t>(result));
        mem.set_word(at + 4 * frame::PKT, pkt_len);
    }

    // The packet of the frame laid at `at`, which with the frame's words takes
    // no more than `room` bytes, empty for none. A packet longer than the room
    // is EBADMSG.
    Result<std::span<const std::uint8_t>> pkt(const Memory &mem, std::uint32_t at, std::uint32_t room) const noexcept {
        if (pkt_len == 0) return std::span<const std::uint8_t>();
        if (std::uint64_t{frame::WORD_BYTES} + pkt_len > room) return marshal_error(Errno::ebadmsg);
        return std::span<const std::uint8_t>(mem.at(at + frame::WORD_BYTES), pkt_len);
    }
};

// What the program answers a command: a value to reply with, or an errno. A
// command the program does not know answers Errno::enosys.
struct Outcome {
    Errno err = Errno::ok;
    std::int32_t val = 0;
};

// The runtime's completion of a request the core made, as the frame that
// completes it carries it, for the request's protocol to read (its generated
// `completion`): the command, the handle the request went under, the result,
// and the reply's packet.
struct Completion {
    std::uint32_t cmd = 0;
    std::uint32_t tag = 0;
    std::int32_t res = 0;
    std::span<const std::uint8_t> pkt;
};

} // namespace xpute
