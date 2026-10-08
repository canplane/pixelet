// cpp/guest/abi/cmd.spec.hpp
//
// GENERATED from spec/abi/cmd.json — do not edit.
//
// Every program's commands, by number (kit abi/cmd: `major << 8 | minor`), and the
// packet each carries — an XTP branch whose children are its fields, in order. A
// command with no fields carries no packet. The majors below PROGRAM_MAJOR are xpute's;
// a program numbers its own from there, in a table of its own that names this one as
// its base, so the two never give out one number twice.
//
// The direction is the ring: `packet` is what the host submits, `signal` what the guest
// raises on the completion ring.

#pragma once

#include <array>
#include <cstdint>
#include <optional>
#include <span>
#include <string_view>

#include "../../kit/abi/cmd.hpp"
#include "../../kit/math/scalar.hpp"
#include "../../kit/status/error.hpp"
#include "../../kit/wire/xtp/cursor.hpp"
#include "../../kit/wire/xtp/encode.hpp"

namespace xpute::abi {

// What a command is about.
enum class Major : std::uint32_t {
    NOP = 0x00,
    SYS = 0x01,
};

} // namespace xpute::abi

template <> struct xpute::Members<xpute::abi::Major> {
    static constexpr std::array<xpute::abi::Major, 2> ALL{xpute::abi::Major::NOP, xpute::abi::Major::SYS};
};

namespace xpute::abi {

// The system calls' answers (ipc/sys.json).
enum class SysMinor : std::uint32_t {
    OPENED = 0x01,
};

} // namespace xpute::abi

template <> struct xpute::Members<xpute::abi::SysMinor> {
    static constexpr std::array<xpute::abi::SysMinor, 1> ALL{xpute::abi::SysMinor::OPENED};
};

namespace xpute::abi {

// Every program's commands.
enum class Command : std::uint32_t {
    // Replies with its packet's value, or 0 where it has none — the rings' own test.
    NOP = 0x0000,
    // An OPENAT answered: the file's size, or -errno — ENOENT where there is no such file.
    // Never sent for a descriptor already closed.
    SYS_OPENED = 0x0101,
};

} // namespace xpute::abi

template <> struct xpute::Members<xpute::abi::Command> {
    static constexpr std::array<xpute::abi::Command, 2> ALL{xpute::abi::Command::NOP, xpute::abi::Command::SYS_OPENED};
};

namespace xpute::abi {

// Command::NOP's packet.
struct Nop {
    std::uint32_t value{};

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
            out.value = saturate_as<std::uint32_t>(*v);
        }
        return out;
    }
};

// Command::SYS_OPENED's packet.
struct SysOpened {
    std::uint32_t fd{};
    std::int32_t res{};
    // Whatever number the host refused with — for HTTP its status — kept for a reader to print and for nothing to branch on.
    std::uint32_t detail{};

    static Result<SysOpened> read(std::span<const std::uint8_t> pkt) noexcept {
        Result<xtp::TreeReader> t = xtp::TreeReader::make(pkt);
        if (!t) return t.error();
        Result<xtp::BranchCursor> f = t->read_branch();
        if (!f) return f.error();
        SysOpened out;
        {
            Result<xtp::Cursor> c = f->at(0);
            if (!c) return c.error();
            auto v = c->get_number();
            if (!v) return v.error();
            out.fd = saturate_as<std::uint32_t>(*v);
        }
        {
            Result<xtp::Cursor> c = f->at(1);
            if (!c) return c.error();
            auto v = c->get_number();
            if (!v) return v.error();
            out.res = saturate_as<std::int32_t>(*v);
        }
        {
            Result<xtp::Cursor> c = f->at(2);
            if (!c) return c.error();
            auto v = c->get_number();
            if (!v) return v.error();
            out.detail = saturate_as<std::uint32_t>(*v);
        }
        return out;
    }
};

} // namespace xpute::abi
