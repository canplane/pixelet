// cpp/xpute/guest/abi/fs.spec.hpp
//
// GENERATED from spec/abi/fs.json
//
// The file device, on major FS: a guest's files, opened below a root the host bound, as
// POSIX's `openat` and io_uring's submission and completion name them (ipc/fs.json).
// The guest posts its calls (`import`), FS_OPENAT and FS_CLOSE with ACKREQ, tagged with
// the descriptor; the host completes each of those on a later turn, a frame of the same
// tag and command flagged RES on the submission ring, and does the rest in the gap.
// xpute ships both ends: the guest's calls (xpute-guest ipc/door.rs, ipc/file.rs) and the
// host's server (@xpute/host). A program binds only its roots to what backs them.

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

namespace xpute::abi::fs {

// What a command is about.
enum class Major : std::uint32_t {
    FS = 0x03,
};

} // namespace xpute::abi::fs

template <> struct xpute::Members<xpute::abi::fs::Major> {
    static constexpr std::array<xpute::abi::fs::Major, 1> ALL{xpute::abi::fs::Major::FS};
};

namespace xpute::abi::fs {

// A file's calls.
enum class FsMinor : std::uint32_t {
    OPENAT = 0x02,
    READ = 0x03,
    WRITE = 0x04,
    CLOSE = 0x05,
};

} // namespace xpute::abi::fs

template <> struct xpute::Members<xpute::abi::fs::FsMinor> {
    static constexpr std::array<xpute::abi::fs::FsMinor, 4> ALL{xpute::abi::fs::FsMinor::OPENAT, xpute::abi::fs::FsMinor::READ, xpute::abi::fs::FsMinor::WRITE, xpute::abi::fs::FsMinor::CLOSE};
};

namespace xpute::abi::fs {

// Every command, by the side that sends it.
enum class Command : std::uint32_t {

    // ---- fs, import ----
    // The file at `path` below `root` opened as `fd`, the frame's tag too. Completed with
    // the file's size, or -errno: ENOENT where there is no such file, ECANCELED where FS_CLOSE
    // came first, ENODEV where the machine has no file device.
    FS_OPENAT = 0x0302,
    // The opened file's first `len` bytes copied to `at` in the guest's memory.
    FS_READ = 0x0303,
    // The `len` bytes at `at` added to what the file opened for writing holds. The host
    // reads them at FS_CLOSE, and the guest keeps them unchanged until FS_CLOSE is completed.
    FS_WRITE = 0x0304,
    // The descriptor ended, the frame's tag too. A file opened for writing is replaced by
    // what was written; an open not yet completed is completed first, with ECANCELED. Completed
    // with 0, or -errno where the file could not be replaced.
    FS_CLOSE = 0x0305,
};

} // namespace xpute::abi::fs

template <> struct xpute::Members<xpute::abi::fs::Command> {
    static constexpr std::array<xpute::abi::fs::Command, 4> ALL{xpute::abi::fs::Command::FS_OPENAT, xpute::abi::fs::Command::FS_READ, xpute::abi::fs::Command::FS_WRITE, xpute::abi::fs::Command::FS_CLOSE};
};

namespace xpute::abi::fs {

// Command::FS_OPENAT's completion's packet.
struct FsOpenatReply {
    // Whatever number the host refused with — for HTTP its status — kept for a reader to print and for nothing to branch on.
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

// Command::FS_OPENAT's event.
struct FsOpenatEvent {
    std::uint32_t fd{};
    std::uint32_t root{};
    // ipc/fs.json `flag`.
    std::uint32_t flags{};
    std::string_view path{};

    // Its number, and the children it is written as.
    static constexpr std::uint32_t NUMBER = 770;
    static constexpr std::uint32_t FIELDS = 4;

    void write(xtp::PacketWriter &w) const noexcept {
        w.u32(fd);
        w.u32(root);
        w.u32(flags);
        w.str(path);
    }
};

// Command::FS_READ's event.
struct FsReadEvent {
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

// Command::FS_WRITE's event.
struct FsWriteEvent {
    std::uint32_t fd{};
    std::uint32_t at{};
    std::uint32_t len{};

    // Its number, and the children it is written as.
    static constexpr std::uint32_t NUMBER = 772;
    static constexpr std::uint32_t FIELDS = 3;

    void write(xtp::PacketWriter &w) const noexcept {
        w.u32(fd);
        w.u32(at);
        w.u32(len);
    }
};

// Command::FS_CLOSE's event.
struct FsCloseEvent {
    std::uint32_t fd{};

    // Its number, and the children it is written as.
    static constexpr std::uint32_t NUMBER = 773;
    static constexpr std::uint32_t FIELDS = 1;

    void write(xtp::PacketWriter &w) const noexcept {
        w.u32(fd);
    }
};

} // namespace xpute::abi::fs
