// cpp/xpute/core/abi/cmd.spec.hpp
//
// GENERATED from spec/abi/cmd.json
//
// xpute's own protocol, the runtime's, on majors NOP, SYS and GPU: the core's life as its runtime tells it.
// NOP is the rings' own test. SYS_ATTACH, SYS_SUSPEND and SYS_RESUME are the core's
// life as its runtime sees it: a runtime (re)attached, the core about to be frozen, and woken;
// what each means to keep or reset is the core's. GPU_READY is the GPU grant's state:
// the device the machine asked for answered. SYS_ABORT is the core's end, the broken
// invariant it reports as it stops, which the runtime hears as a VMM hears a core's exit.
// Each command is `major << 8 | minor` (kit abi/cmd) and carries an XTP branch whose
// children are its fields, in order; one with no fields carries no packet. Files are a
// device of their own (fs.json). Majors 0x00 to 0x0F are xpute's (ipc/frame.json); a
// program numbers its own protocols' commands past them.

#pragma once

#include <array>
#include <cstdint>
#include <optional>
#include <span>
#include <string_view>

#include "xpute/kit/abi/cmd.hpp"
#include "xpute/kit/status/error.hpp"
#include "xpute/kit/wire/xtp/cursor.hpp"
#include "xpute/kit/wire/xtp/encode.hpp"
#include "number.hpp"

namespace xpute::abi {

// What a command is about.
enum class Major : std::uint32_t {
    NOP = 0x00,
    SYS = 0x01,
    GPU = 0x02,
};

} // namespace xpute::abi

template <> struct xpute::Members<xpute::abi::Major> {
    static constexpr std::array<xpute::abi::Major, 3> ALL{xpute::abi::Major::NOP, xpute::abi::Major::SYS, xpute::abi::Major::GPU};
};

namespace xpute::abi {

// The core's life as its runtime tells it, and its end.
enum class SysMinor : std::uint32_t {
    ABORT = 0x06,
    ATTACH = 0x07,
    SUSPEND = 0x08,
    RESUME = 0x09,
};

} // namespace xpute::abi

template <> struct xpute::Members<xpute::abi::SysMinor> {
    static constexpr std::array<xpute::abi::SysMinor, 4> ALL{xpute::abi::SysMinor::ABORT, xpute::abi::SysMinor::ATTACH, xpute::abi::SysMinor::SUSPEND, xpute::abi::SysMinor::RESUME};
};

namespace xpute::abi {

// The GPU grant's state.
enum class GpuMinor : std::uint32_t {
    READY = 0x01,
};

} // namespace xpute::abi

template <> struct xpute::Members<xpute::abi::GpuMinor> {
    static constexpr std::array<xpute::abi::GpuMinor, 1> ALL{xpute::abi::GpuMinor::READY};
};

namespace xpute::abi {

// Every command, by the side that sends it.
enum class Command : std::uint32_t {

    // ---- runtime, export ----
    // Replies with its packet's value, or 0 where it has none — the rings' own test.
    NOP = 0x0000,
    // A runtime has attached, the first or again after one before it went: the mount sends it
    // once the front is laid. What the old runtime's requests leave behind is the core's to let go.
    SYS_ATTACH = 0x0107,
    // The core is about to be frozen, its turns stopped for as long as its runtime is hidden:
    // its last sure moment to save. A call, rung at once.
    SYS_SUSPEND = 0x0108,
    // The core woken from SYS_SUSPEND. A call, rung at once.
    SYS_RESUME = 0x0109,
    // The GPU the machine asked for answered, the first time or again after a loss, whose
    // sign is in the stream's device block.
    GPU_READY = 0x0201,

    // ---- runtime, import ----
    // The broken invariant the core reports as it stops (xpute-kit status/bug.rs).
    SYS_ABORT = 0x0106,
};

} // namespace xpute::abi

template <> struct xpute::Members<xpute::abi::Command> {
    static constexpr std::array<xpute::abi::Command, 6> ALL{xpute::abi::Command::NOP, xpute::abi::Command::SYS_ATTACH, xpute::abi::Command::SYS_SUSPEND, xpute::abi::Command::SYS_RESUME, xpute::abi::Command::GPU_READY, xpute::abi::Command::SYS_ABORT};
};

namespace xpute::abi {

// Command::NOP's packet.
struct Nop {
    std::uint32_t val{};

    static Result<Nop> read(std::span<const std::uint8_t> pkt) noexcept {
        Result<xtp::TreeReader> t = xtp::TreeReader::make(pkt);
        if (!t) return t.error();
        Result<xtp::BranchCursor> f = t->read_branch();
        if (!f) return f.error();
        Nop out;
        {
            Result<xtp::Cursor> c = f->at(0);
            if (!c) return c.error();
            auto v = c->get_number();
            if (!v) return v.error();
            auto n = integer<std::uint32_t>(*v);
            if (!n) return n.error();
            out.val = *n;
        }
        return out;
    }
};

// Command::SYS_ABORT's packet.
struct SysAbort {
    std::int32_t errnum{};
    std::uint32_t line{};
    std::uint32_t column{};
    std::uint32_t a_low{};
    std::uint32_t a_high{};
    std::uint32_t b_low{};
    std::uint32_t b_high{};
    std::string_view file{};

    // Its number, and the children it is written as.
    static constexpr std::uint32_t NUMBER = 262;
    static constexpr std::uint32_t FIELDS = 8;

    void write(xtp::PacketWriter &w) const noexcept {
        w.i32(errnum);
        w.u32(line);
        w.u32(column);
        w.u32(a_low);
        w.u32(a_high);
        w.u32(b_low);
        w.u32(b_high);
        w.str(file);
    }
};

} // namespace xpute::abi
