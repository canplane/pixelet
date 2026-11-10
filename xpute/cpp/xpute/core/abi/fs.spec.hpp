// cpp/xpute/core/abi/fs.spec.hpp
//
// GENERATED from spec/abi/fs.json
//
// The file device's protocol on major FS: a core's files, each opened below a root the
// runtime has bound, in the terms of POSIX's `openat` and of io_uring's submissions and
// completions (ipc/fs.json). The core posts its calls as imports. It posts FS_OPENAT and
// FS_CLOSE with ACKREQ, tagged with the descriptor, and the runtime completes each of them
// on a later turn with a frame of the same tag and command, flagged RES, on the submission
// ring. The runtime does the rest of the work in the gap between turns. It tells the core
// that a file changed under it with FS_CHANGED, an export. xpute ships both ends: the core's
// calls (xpute-core ipc/door.rs, ipc/file.rs) and the runtime's server (@xpute/runtime). A
// program only binds its roots to what backs them.

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
#include "xpute/core/abi/number.hpp"

namespace xpute::abi::fs {

// A command's major, the high byte of its number, by which the runtime routes the frame.
enum class Major : std::uint32_t {
    FS = 0x03,
};

} // namespace xpute::abi::fs

template <> struct xpute::EnumInfo<xpute::abi::fs::Major> {
    static constexpr std::array<xpute::abi::fs::Major, 1> ALL{xpute::abi::fs::Major::FS};
};

namespace xpute::abi::fs {

enum class FsMinor : std::uint32_t {
    OPENAT = 0x01,
    READ = 0x02,
    WRITE = 0x03,
    CLOSE = 0x04,
    CHANGED = 0x05,
};

} // namespace xpute::abi::fs

template <> struct xpute::EnumInfo<xpute::abi::fs::FsMinor> {
    static constexpr std::array<xpute::abi::fs::FsMinor, 5> ALL{xpute::abi::fs::FsMinor::OPENAT, xpute::abi::fs::FsMinor::READ, xpute::abi::fs::FsMinor::WRITE, xpute::abi::fs::FsMinor::CLOSE, xpute::abi::fs::FsMinor::CHANGED};
};

namespace xpute::abi::fs {

// Every command, by the side that sends it.
enum class Command : std::uint32_t {

    // ---- fs, export ----
    // The file at `path` below `root` changed under the core, as reported by what backs the
    // root. What the core has read of the file is stale, and to read it again the core opens
    // it again.
    FS_CHANGED = 0x0305,

    // ---- fs, import ----
    // Opens the file at `path` below `root` as descriptor `fd`, which is also the frame's tag.
    // The runtime completes it with the file's size, or with -errno: ENOENT when there is no
    // such file, ECANCELED when FS_CLOSE came first, and ENODEV when the machine has no file
    // device.
    FS_OPENAT = 0x0301,
    // Copies the first `len` bytes of the opened file to `at` in the core's memory.
    FS_READ = 0x0302,
    // Appends the `len` bytes at `at` to what the file, opened for writing, will hold. The
    // runtime reads the bytes at FS_CLOSE, so the core keeps them unchanged until FS_CLOSE is
    // completed.
    FS_WRITE = 0x0303,
    // Ends the descriptor, which is also the frame's tag. A file opened for writing is replaced
    // by what was written to it. An open not yet completed is completed first, with ECANCELED.
    // The close is completed with 0, or with -errno when the file could not be replaced.
    FS_CLOSE = 0x0304,
};

} // namespace xpute::abi::fs

template <> struct xpute::EnumInfo<xpute::abi::fs::Command> {
    static constexpr std::array<xpute::abi::fs::Command, 5> ALL{xpute::abi::fs::Command::FS_CHANGED, xpute::abi::fs::Command::FS_OPENAT, xpute::abi::fs::Command::FS_READ, xpute::abi::fs::Command::FS_WRITE, xpute::abi::fs::Command::FS_CLOSE};
};

namespace xpute::abi::fs {

// Command::FS_CHANGED's packet.
struct FsChanged {
    std::uint32_t root{};
    // The file's path below the root, spelled as the core opened it.
    std::string_view path{};

    static Result<FsChanged> read(std::span<const std::uint8_t> pkt) noexcept {
        Result<xtp::TreeReader> t = xtp::TreeReader::make(pkt);
        if (!t) return t.error();
        Result<xtp::BranchCursor> f = t->read_branch();
        if (!f) return f.error();
        FsChanged out;
        {
            Result<xtp::Cursor> c = f->at(0);
            if (!c) return c.error();
            auto v = c->get_number();
            if (!v) return v.error();
            auto n = integer<std::uint32_t>(*v);
            if (!n) return n.error();
            out.root = *n;
        }
        {
            Result<xtp::Cursor> c = f->at(1);
            if (!c) return c.error();
            auto v = c->get_str_ref();
            if (!v) return v.error();
            out.path = *v;
        }
        return out;
    }
};

// Command::FS_OPENAT's completion's packet.
struct FsOpenatReply {
    // The number the runtime refused the open with, such as an HTTP status. It is kept to be
    // printed, and nothing should branch on it.
    std::uint32_t detail{};

    static Result<FsOpenatReply> read(std::span<const std::uint8_t> pkt) noexcept {
        Result<xtp::TreeReader> t = xtp::TreeReader::make(pkt);
        if (!t) return t.error();
        Result<xtp::BranchCursor> f = t->read_branch();
        if (!f) return f.error();
        FsOpenatReply out;
        {
            Result<xtp::Cursor> c = f->at(0);
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

// Command::FS_OPENAT's packet.
struct FsOpenat {
    std::uint32_t fd{};
    std::uint32_t root{};
    // One of the `flag` values of ipc/fs.json.
    std::uint32_t flags{};
    std::string_view path{};

    // Its number, and the children it is written as.
    static constexpr std::uint32_t NUMBER = 769;
    static constexpr std::uint32_t FIELDS = 4;

    void write(xtp::PacketWriter &w) const noexcept {
        w.u32(fd);
        w.u32(root);
        w.u32(flags);
        w.str(path);
    }
};

// Command::FS_READ's packet.
struct FsRead {
    std::uint32_t fd{};
    std::uint32_t at{};
    std::uint32_t len{};

    // Its number, and the children it is written as.
    static constexpr std::uint32_t NUMBER = 770;
    static constexpr std::uint32_t FIELDS = 3;

    void write(xtp::PacketWriter &w) const noexcept {
        w.u32(fd);
        w.u32(at);
        w.u32(len);
    }
};

// Command::FS_WRITE's packet.
struct FsWrite {
    std::uint32_t fd{};
    std::uint32_t at{};
    std::uint32_t len{};

    // Its number, and the children it is written as.
    static constexpr std::uint32_t NUMBER = 771;
    static constexpr std::uint32_t FIELDS = 3;

    void write(xtp::PacketWriter &w) const noexcept {
        w.u32(fd);
        w.u32(at);
        w.u32(len);
    }
};

// Command::FS_CLOSE's packet.
struct FsClose {
    std::uint32_t fd{};

    // Its number, and the children it is written as.
    static constexpr std::uint32_t NUMBER = 772;
    static constexpr std::uint32_t FIELDS = 1;

    void write(xtp::PacketWriter &w) const noexcept {
        w.u32(fd);
    }
};

} // namespace xpute::abi::fs
