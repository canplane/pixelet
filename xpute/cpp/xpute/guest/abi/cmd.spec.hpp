// cpp/xpute/guest/abi/cmd.spec.hpp
//
// GENERATED from spec/abi/cmd.json
//
// xpute's own commands, by number (kit abi/cmd: `major << 8 | minor`), and the packet
// each carries — an XTP branch whose children are its fields, in order. A command with
// no fields carries no packet. The host sends them, port 0 (ipc/frame.json HOST); a
// program numbers its own as it likes, and the sender tells the two apart.
//
// The direction is the ring: `pkt` is what the host submits, `event` what the guest
// posts on the completion ring for the host's port, its hostcalls (ipc/sys.json).

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
};

} // namespace xpute::abi

template <> struct xpute::Members<xpute::abi::Major> {
    static constexpr std::array<xpute::abi::Major, 2> ALL{xpute::abi::Major::NOP, xpute::abi::Major::SYS};
};

namespace xpute::abi {

// The hostcalls and their answers (ipc/sys.json).
enum class SysMinor : std::uint32_t {
    OPENED = 0x01,
    OPENAT = 0x02,
    READ = 0x03,
    WRITE = 0x04,
    CLOSE = 0x05,
    ABORT = 0x06,
};

} // namespace xpute::abi

template <> struct xpute::Members<xpute::abi::SysMinor> {
    static constexpr std::array<xpute::abi::SysMinor, 6> ALL{xpute::abi::SysMinor::OPENED, xpute::abi::SysMinor::OPENAT, xpute::abi::SysMinor::READ, xpute::abi::SysMinor::WRITE, xpute::abi::SysMinor::CLOSE, xpute::abi::SysMinor::ABORT};
};

namespace xpute::abi {

// xpute's own commands.
enum class Command : std::uint32_t {
    // Replies with its packet's value, or 0 where it has none — the rings' own test.
    NOP = 0x0000,
    // An OPENAT answered: the file's size, or -errno — ENOENT where there is no such file.
    // Never sent for a descriptor already closed.
    SYS_OPENED = 0x0101,
    // The file at `path` below `root` opened as `fd`, answered by SYS_OPENED.
    SYS_OPENAT = 0x0102,
    // The opened file's first `len` bytes copied to `at` in the guest's memory.
    SYS_READ = 0x0103,
    // The `len` bytes at `at` added to what the file opened for writing holds.
    SYS_WRITE = 0x0104,
    // The descriptor ended. A file opened for writing is replaced by what was written; an
    // open not yet answered is canceled, and never answered.
    SYS_CLOSE = 0x0105,
    // The broken invariant the guest reports as it stops (xpute-kit status/bug.rs).
    SYS_ABORT = 0x0106,
};

} // namespace xpute::abi

template <> struct xpute::Members<xpute::abi::Command> {
    static constexpr std::array<xpute::abi::Command, 7> ALL{xpute::abi::Command::NOP, xpute::abi::Command::SYS_OPENED, xpute::abi::Command::SYS_OPENAT, xpute::abi::Command::SYS_READ, xpute::abi::Command::SYS_WRITE, xpute::abi::Command::SYS_CLOSE, xpute::abi::Command::SYS_ABORT};
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
            auto n = integer<std::uint32_t>(*v);
            if (!n) return n.error();
            out.fd = *n;
        }
        {
            Result<xtp::Cursor> c = f->at(1);
            if (!c) return c.error();
            auto v = c->get_number();
            if (!v) return v.error();
            auto n = integer<std::int32_t>(*v);
            if (!n) return n.error();
            out.res = *n;
        }
        {
            Result<xtp::Cursor> c = f->at(2);
            if (!c) return c.error();
            auto v = c->get_number();
            if (!v) return v.error();
            auto n = integer<std::uint32_t>(*v);
            if (!n) return n.error();
            out.detail = *n;
        }
        return out;
    }
};

// Command::SYS_OPENAT's event.
struct SysOpenatEvent {
    std::uint32_t fd{};
    std::uint32_t root{};
    // ipc/sys.json `flag`.
    std::uint32_t flags{};
    std::string_view path{};

    // Its number, and the children it is written as.
    static constexpr std::uint32_t NUMBER = 258;
    static constexpr std::uint32_t FIELDS = 4;

    void write(xtp::PacketWriter &w) const noexcept {
        w.u32(fd);
        w.u32(root);
        w.u32(flags);
        w.str(path);
    }
};

// Command::SYS_READ's event.
struct SysReadEvent {
    std::uint32_t fd{};
    std::uint32_t at{};
    std::uint32_t len{};

    // Its number, and the children it is written as.
    static constexpr std::uint32_t NUMBER = 259;
    static constexpr std::uint32_t FIELDS = 3;

    void write(xtp::PacketWriter &w) const noexcept {
        w.u32(fd);
        w.u32(at);
        w.u32(len);
    }
};

// Command::SYS_WRITE's event.
struct SysWriteEvent {
    std::uint32_t fd{};
    std::uint32_t at{};
    std::uint32_t len{};

    // Its number, and the children it is written as.
    static constexpr std::uint32_t NUMBER = 260;
    static constexpr std::uint32_t FIELDS = 3;

    void write(xtp::PacketWriter &w) const noexcept {
        w.u32(fd);
        w.u32(at);
        w.u32(len);
    }
};

// Command::SYS_CLOSE's event.
struct SysCloseEvent {
    std::uint32_t fd{};

    // Its number, and the children it is written as.
    static constexpr std::uint32_t NUMBER = 261;
    static constexpr std::uint32_t FIELDS = 1;

    void write(xtp::PacketWriter &w) const noexcept {
        w.u32(fd);
    }
};

// Command::SYS_ABORT's event.
struct SysAbortEvent {
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
