// cpp/xpute/core/abi/sys.spec.hpp
//
// GENERATED from spec/abi/sys.json
//
// xpute's own protocol on majors NOP and SYS: the core's life as its runtime tells it, its
// turns, and its end. NOP is the rings' own test. SYS_ATTACH, SYS_SUSPEND and SYS_RESUME
// are the core's life as its runtime sees it: a runtime (re)attached, the core about to be
// frozen, and woken; what each means to keep or reset is the core's. SYS_TURN marks a turn
// the runtime's scheduling policy grants, with the wall clock. SYS_ABORT is the core's end, the
// broken invariant it reports as it stops, which the runtime hears as a VMM hears a core's
// exit. Each command is `major << 8 | minor` (kit abi/cmd) and carries an XTP branch whose
// children are its fields, in order; one with no fields carries no packet. The GPU and the
// files are devices of their own (gpu.json, fs.json). Majors 0x00 to 0x0F are xpute's
// (ipc/frame.json); a program numbers its own protocols' commands past them.

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

namespace xpute::abi::sys {

// What a command is about.
enum class Major : std::uint32_t {
    NOP = 0x00,
    SYS = 0x01,
};

} // namespace xpute::abi::sys

template <> struct xpute::Members<xpute::abi::sys::Major> {
    static constexpr std::array<xpute::abi::sys::Major, 2> ALL{xpute::abi::sys::Major::NOP, xpute::abi::sys::Major::SYS};
};

namespace xpute::abi::sys {

// The core's life as its runtime tells it, its turns, and its end.
enum class SysMinor : std::uint32_t {
    ABORT = 0x01,
    ATTACH = 0x02,
    SUSPEND = 0x03,
    RESUME = 0x04,
    TURN = 0x05,
};

} // namespace xpute::abi::sys

template <> struct xpute::Members<xpute::abi::sys::SysMinor> {
    static constexpr std::array<xpute::abi::sys::SysMinor, 5> ALL{xpute::abi::sys::SysMinor::ABORT, xpute::abi::sys::SysMinor::ATTACH, xpute::abi::sys::SysMinor::SUSPEND, xpute::abi::sys::SysMinor::RESUME, xpute::abi::sys::SysMinor::TURN};
};

namespace xpute::abi::sys {

// Every command, by the side that sends it.
enum class Command : std::uint32_t {

    // ---- sys, export ----
    // Replies with its packet's value, or 0 where it has none — the rings' own test.
    NOP = 0x0000,
    // A runtime has attached, the first or again after one before it went: the mount sends it
    // once the front is laid. What the old runtime's requests leave behind is the core's to let go.
    SYS_ATTACH = 0x0102,
    // The core is about to be frozen, its turns stopped for as long as its runtime is hidden:
    // its last sure moment to save. A call, rung at once.
    SYS_SUSPEND = 0x0103,
    // The core woken from SYS_SUSPEND. A call, rung at once.
    SYS_RESUME = 0x0104,
    // A turn the scheduling policy grants, sent before it rings after what the program
    // submits for it, as against a turn rung for a command alone: the wall clock, in Unix
    // milliseconds. What else the policy says of the turn is the program's to send.
    SYS_TURN = 0x0105,

    // ---- sys, import ----
    // The broken invariant the core reports as it stops (xpute-kit status/bug.rs).
    SYS_ABORT = 0x0101,
};

} // namespace xpute::abi::sys

template <> struct xpute::Members<xpute::abi::sys::Command> {
    static constexpr std::array<xpute::abi::sys::Command, 6> ALL{xpute::abi::sys::Command::NOP, xpute::abi::sys::Command::SYS_ATTACH, xpute::abi::sys::Command::SYS_SUSPEND, xpute::abi::sys::Command::SYS_RESUME, xpute::abi::sys::Command::SYS_TURN, xpute::abi::sys::Command::SYS_ABORT};
};

namespace xpute::abi::sys {

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

// Command::SYS_TURN's packet.
struct SysTurn {
    double unix_ms{};

    static Result<SysTurn> read(std::span<const std::uint8_t> pkt) noexcept {
        Result<xtp::TreeReader> t = xtp::TreeReader::make(pkt);
        if (!t) return t.error();
        Result<xtp::BranchCursor> f = t->read_branch();
        if (!f) return f.error();
        SysTurn out;
        {
            Result<xtp::Cursor> c = f->at(0);
            if (!c) return c.error();
            auto v = c->get_number();
            if (!v) return v.error();
            auto n = finite(*v);
            if (!n) return n.error();
            out.unix_ms = *n;
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
    static constexpr std::uint32_t NUMBER = 257;
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

} // namespace xpute::abi::sys
