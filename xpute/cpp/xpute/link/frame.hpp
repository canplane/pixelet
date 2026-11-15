// cpp/xpute/link/frame.hpp

// A frame's words are laid out by `link/spec/frame.json5`, as xpute-link's
// `frame.rs` lays them: the request's tag, its command, the result, and the
// packet's length. The command is the `RES` bit, the handle the callee issued
// and the method's place among its object's. The packet, the method's args or
// its answer's value in the Canonical ABI's layout (`cabi.hpp`), lies right
// past the words. The words are little-endian, as both forms' memory is.

#pragma once

#include <cstdint>
#include <optional>
#include <span>

#include "xpute/kit/status/error.hpp"
#include "frame.spec.hpp"
#include "mem.hpp"

namespace xpute::link {

// The size of a frame's words, its header before its packet.
inline constexpr std::uint32_t HDR_SZ = frame::WORDS * 4;

// `NAMED` is a handle's top bit.
static_assert(frame::NAMED == frame::RES >> frame::METHOD_BITS >> 1);

struct Frame {
    std::uint32_t tag;
    // An answer, to the receiver's request `tag`.
    bool res;
    std::uint32_t handle;
    std::uint32_t method;
    std::int32_t result;
    std::uint32_t pkt_len;

    // The frame laid at `at`.
    static Frame at(std::uint64_t at) noexcept {
        auto w = [at](std::uint32_t k) { return mem::word(at + 4 * std::uint64_t{k}); };
        std::uint32_t cmd = w(frame::CMD);
        return Frame{
            .tag = w(frame::TAG),
            .res = (cmd & frame::RES) != 0,
            .handle = (cmd & ~frame::RES) >> frame::METHOD_BITS,
            .method = cmd & ((1u << frame::METHOD_BITS) - 1),
            .result = static_cast<std::int32_t>(w(frame::RESULT)),
            .pkt_len = w(frame::PKT),
        };
    }

    // Lays the frame's words at `at`.
    void lay(std::uint64_t at) const noexcept {
        auto set = [at](std::uint32_t k, std::uint32_t v) { mem::set_word(at + 4 * std::uint64_t{k}, v); };
        set(frame::TAG, tag);
        set(frame::CMD, (res ? frame::RES : 0) | handle << frame::METHOD_BITS | method);
        set(frame::RESULT, static_cast<std::uint32_t>(result));
        set(frame::PKT, pkt_len);
    }

    // The packet of the frame of `cap` bytes laid at `at`, none where it has
    // none; a packet past the cap is EBADMSG.
    Result<std::optional<std::span<const std::uint8_t>>> pkt(std::uint64_t at, std::uint32_t cap) const noexcept {
        if (pkt_len == 0) return std::optional<std::span<const std::uint8_t>>();
        if (std::uint64_t{HDR_SZ} + pkt_len > cap) return make_error(Errno::ebadmsg);
        return std::optional<std::span<const std::uint8_t>>(mem::bytes(at + HDR_SZ, pkt_len));
    }
};

} // namespace xpute::link
