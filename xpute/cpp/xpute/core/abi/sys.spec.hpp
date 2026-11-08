// cpp/xpute/core/abi/sys.spec.hpp
//
// GENERATED from spec/abi/sys.json
//
// xpute's own protocol on majors NOP and SYS. Through it the runtime tells the core about
// the core's life and its turns, and the core reports its end. NOP is the rings' own test.
// SYS_ATTACH, SYS_SUSPEND and SYS_RESUME tell the core that a runtime attached, for the
// first time or again, that the core is about to be frozen, and that it was woken; what
// each one means the core keeps or resets is the core's to decide. SYS_TURN marks a turn
// that the runtime's scheduling policy grants, and carries the wall clock. SYS_ABORT
// reports the broken invariant the core stops on, and the runtime hears it as a VMM hears
// its guest exit.
//
// Each command is `major << 8 | minor` (kit abi/cmd). Its packet is an XTP branch whose
// children are the command's fields, in order; a command with no fields carries no packet.
// The GPU and the files are devices with protocols of their own (gpu.json, fs.json).
// Majors 0x00 to 0x0F are xpute's (ipc/frame.json), and a program numbers the commands of
// its own protocols past them.

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

// A command's major, the high byte of its number, by which the runtime routes the frame.
enum class Major : std::uint32_t {
    NOP = 0x00,
    SYS = 0x01,
};

} // namespace xpute::abi::sys

template <> struct xpute::Members<xpute::abi::sys::Major> {
    static constexpr std::array<xpute::abi::sys::Major, 2> ALL{xpute::abi::sys::Major::NOP, xpute::abi::sys::Major::SYS};
};

namespace xpute::abi::sys {

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
    // The rings' own test. The reply carries the packet's value, or 0 when the frame has no
    // packet.
    NOP = 0x0000,
    // A runtime has attached, for the first time or after an earlier runtime went away. The
    // runtime sends it once it has laid the front of the core's memory (ipc/head.json). The core
    // lets go of whatever the earlier runtime's requests left behind.
    SYS_ATTACH = 0x0102,
    // The core is about to be frozen: its turns stop for as long as its runtime is hidden. This
    // is the core's last sure moment to save. It is a call, and the runtime rings the core at
    // once to have it answered.
    SYS_SUSPEND = 0x0103,
    // The core has been woken after SYS_SUSPEND. It is a call, rung at once as SYS_SUSPEND is.
    SYS_RESUME = 0x0104,
    // Marks a turn that the scheduling policy grants, as opposed to a turn rung only because a
    // command arrived. The runtime submits it after what the program submits for that turn,
    // and before it rings. It carries the wall clock in Unix milliseconds. Anything else the
    // policy has to say about the turn, the program sends in commands of its own.
    SYS_TURN = 0x0105,

    // ---- sys, import ----
    // Reports the broken invariant the core stops on (xpute-kit status/bug.rs).
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
