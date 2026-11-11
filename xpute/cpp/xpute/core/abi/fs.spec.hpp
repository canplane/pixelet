// cpp/xpute/core/abi/fs.spec.hpp
//
// GENERATED from spec/abi/fs.json
//
// The file device's protocol on major FS: a core's files, read whole into its own buffers
// and written whole, through a descriptor, as WASI's path_open, fd_read, fd_write and
// fd_close do. A file's name is an absolute path, a tuple of tokens, each a u32
// or a string, as many as the program uses. xpute gives no token a meaning: the program
// numbers its fixed names and keeps variable data in its own type, and its one binding
// answers every path (@xpute/runtime io/fs.ts).
//
// The core sends its calls as imports. The core issues each descriptor, a generational
// handle, and every frame of a descriptor carries it once, as its tag. FS_OPEN and FS_CLOSE
// go with ACKREQ, and the runtime completes each once, on a later turn, with a frame of the
// same tag and command, flagged RES. It serves a descriptor's frames in the order they came,
// so an FS_READ is done before its close is completed, and the memory an FS_READ names is the
// runtime's until then. An FS_WRITE's bytes go with its frame, and the file is replaced by
// them at FS_CLOSE. The runtime tells the core that a file changed under it with
// FS_CHANGED, an export. xpute ships both ends: the core's calls (xpute-core io/door.rs,
// io/file.rs) and the runtime's server (@xpute/runtime). A program only gives the binding
// that backs its paths.

#pragma once

#include <array>
#include <cstdint>
#include <optional>
#include <span>
#include <string_view>
#include <variant>

#include "xpute/kit/abi/cmd.hpp"
#include "xpute/kit/status/error.hpp"
#include "xpute/kit/wire/xtp/cursor.hpp"
#include "xpute/kit/wire/xtp/encode.hpp"
#include "xpute/core/abi/number.hpp"
#include "xpute/core/host.hpp"
#include "xpute/core/io/frame.hpp"
#include "xpute/core/io/fs.hpp"

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
    OPEN = 0x01,
    READ = 0x02,
    WRITE = 0x03,
    CLOSE = 0x04,
    CHANGED = 0x05,
};

} // namespace xpute::abi::fs

template <> struct xpute::EnumInfo<xpute::abi::fs::FsMinor> {
    static constexpr std::array<xpute::abi::fs::FsMinor, 5> ALL{xpute::abi::fs::FsMinor::OPEN, xpute::abi::fs::FsMinor::READ, xpute::abi::fs::FsMinor::WRITE, xpute::abi::fs::FsMinor::CLOSE, xpute::abi::fs::FsMinor::CHANGED};
};

namespace xpute::abi::fs {

// Every command, by the side that sends it.
enum class Command : std::uint32_t {

    // ---- fs, export ----
    // The file at `path` changed under the core, as reported by what backs it. What the core
    // has read of the file is stale, and to read it again the core opens it again.
    FS_CHANGED = 0x0305,

    // ---- fs, import ----
    // Opens the file at `path` as the descriptor the frame's tag carries, as WASI's path_open
    // does. The runtime completes an open to read once the file can be read whole, with its size,
    // and an open to write with 0; or either with -errno: ENOENT when there is no such file to
    // read, EINVAL when `flags` is not one of io/fs.json's `flag`, which the descriptor's close
    // then completes with too, ECANCELED when FS_CLOSE came first, and ENODEV when the manifest has
    // no file device.
    FS_OPEN = 0x0301,
    // Copies the first `len` bytes of the opened file to `at` in the core's memory, as WASI's
    // fd_read reads into a caller's buffer. A read of a file opened to write, or of one whose
    // open has not completed, copies nothing, and the descriptor's close completes with EBADF.
    // Where the manifest has no file device, it is refused with ENODEV.
    FS_READ = 0x0302,
    // Appends the `len` bytes at `at` to what the file, opened to write, will hold, as WASI's
    // fd_write does. The bytes go with the frame, so the core keeps nothing. Nothing completes
    // it: the file's close says whether what was written replaced it. A write to a file opened
    // to read keeps nothing, and the descriptor's close completes with EBADF.
    // Where the manifest has no file device, it is refused with ENODEV.
    FS_WRITE = 0x0303,
    // Ends the descriptor, as WASI's fd_close does. A file opened to write is replaced by what
    // was written to it, handed to the program's binding whole. An open not yet completed is
    // completed first, with ECANCELED. The close is completed with: 0; the errno the binding
    // refused the written file with; EINVAL where its open was refused for its flags; EBADF for a
    // descriptor the runtime does not hold, or one read while opened to write or before its open
    // completed, or written while opened to read; EFAULT where a read of it named memory outside
    // the core's or at offset 0; ENODEV where the manifest has no file device; or ECANCELED where a
    // reload of the program left the file unserved.
    FS_CLOSE = 0x0304,
};

} // namespace xpute::abi::fs

template <> struct xpute::EnumInfo<xpute::abi::fs::Command> {
    static constexpr std::array<xpute::abi::fs::Command, 5> ALL{xpute::abi::fs::Command::FS_CHANGED, xpute::abi::fs::Command::FS_OPEN, xpute::abi::fs::Command::FS_READ, xpute::abi::fs::Command::FS_WRITE, xpute::abi::fs::Command::FS_CLOSE};
};

namespace xpute::abi::fs {

// Command::FS_CHANGED's packet.
struct FsChanged {
    // The file's path, as the core opened it.
    xpute::Path path{};

    static Result<FsChanged> read(std::span<const std::uint8_t> pkt) noexcept {
        Result<xtp::TreeReader> t = xtp::TreeReader::make(pkt);
        if (!t) return t.error();
        Result<xtp::BranchCursor> f = t->read_branch();
        if (!f) return f.error();
        FsChanged out;
        {
            Result<xtp::Cursor> c = f->at(0);
            if (!c) return c.error();
            auto v = xpute::Path::read(*c);
            if (!v) return v.error();
            out.path = *v;
        }
        return out;
    }
};

// Command::FS_OPEN's completion's packet.
struct FsOpenReply {
    // The number the runtime refused the open with, such as an HTTP status. It is kept to be
    // printed, and nothing should branch on it.
    std::uint32_t detail{};

    static Result<FsOpenReply> read(std::span<const std::uint8_t> pkt) noexcept {
        Result<xtp::TreeReader> t = xtp::TreeReader::make(pkt);
        if (!t) return t.error();
        Result<xtp::BranchCursor> f = t->read_branch();
        if (!f) return f.error();
        FsOpenReply out;
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

// Command::FS_OPEN's packet.
struct FsOpen {
    std::span<const xpute::Token> path{};
    // One of the `flag` values of io/fs.json.
    std::uint32_t flags{};

    // Its number, the children it is written as, and whether it goes
    // with ACKREQ.
    static constexpr std::uint32_t NUMBER = 769;
    static constexpr std::uint32_t FIELDS = 2;
    static constexpr bool ACKREQ = true;

    void write(xtp::PacketWriter &w) const noexcept {
        xpute::Path::write(w, path);
        w.u32(flags);
    }

    std::array<std::uint32_t, 2> named() const noexcept {
        return {0, 0};
    }
};

// Command::FS_READ's packet.
struct FsRead {
    std::uint32_t at{};
    std::uint32_t len{};

    // Its number, the children it is written as, and whether it goes
    // with ACKREQ.
    static constexpr std::uint32_t NUMBER = 770;
    static constexpr std::uint32_t FIELDS = 2;
    static constexpr bool ACKREQ = false;

    void write(xtp::PacketWriter &w) const noexcept {
        w.u32(at);
        w.u32(len);
    }

    std::array<std::uint32_t, 2> named() const noexcept {
        return {0, 0};
    }
};

// Command::FS_WRITE's packet.
struct FsWrite {
    std::uint32_t at{};
    std::uint32_t len{};

    // Its number, the children it is written as, and whether it goes
    // with ACKREQ.
    static constexpr std::uint32_t NUMBER = 771;
    static constexpr std::uint32_t FIELDS = 2;
    static constexpr bool ACKREQ = false;

    void write(xtp::PacketWriter &w) const noexcept {
        w.u32(at);
        w.u32(len);
    }

    std::array<std::uint32_t, 2> named() const noexcept {
        return {at, len};
    }
};

// Command::FS_CLOSE's packet.
struct FsClose {

    // Its number, the children it is written as, and whether it goes
    // with ACKREQ.
    static constexpr std::uint32_t NUMBER = 772;
    static constexpr std::uint32_t FIELDS = 0;
    static constexpr bool ACKREQ = true;

    void write(xtp::PacketWriter &) const noexcept {
    }

    std::array<std::uint32_t, 2> named() const noexcept {
        return {0, 0};
    }
};

// Command::FS_OPEN, completed on a later turn.
template <class D> void fs_open(const D &door, std::uint32_t fd, const FsOpen &e) noexcept { door.import(fd, e); }

// Command::FS_READ, posted for the runtime.
template <class D> void fs_read(const D &door, std::uint32_t fd, const FsRead &e) noexcept { door.import(fd, e); }

// Command::FS_WRITE, posted for the runtime.
template <class D> void fs_write(const D &door, std::uint32_t fd, const FsWrite &e) noexcept { door.import(fd, e); }

// Command::FS_CLOSE, completed on a later turn.
template <class D> void fs_close(const D &door, std::uint32_t fd) noexcept { door.import(fd, FsClose{}); }

// `cmd` read and handed to the member of `h` of its name; ENOSYS for one this
// protocol does not carry.
template <class H> Outcome apply(H &h, std::uint32_t cmd, std::span<const std::uint8_t> pkt) noexcept {
    if (cmd == static_cast<std::uint32_t>(Command::FS_CHANGED)) {
        Result<FsChanged> c = FsChanged::read(pkt);
        if (!c) return {.err = c.error().errnum};
        return h.fs_changed(*c);
    }
    return {.err = Errno::enosys};
}

// Command::FS_OPEN's completion, under the handle it was made with, and its result.
struct FsOpenCompletion {
    std::uint32_t fd;
    Result<std::uint32_t> res;
    std::optional<FsOpenReply> reply;
};

// Command::FS_CLOSE's completion, under the handle it was made with, and its result.
struct FsCloseCompletion {
    std::uint32_t fd;
    Result<std::uint32_t> res;
};

using Completion = std::variant<FsOpenCompletion, FsCloseCompletion>;

// `c` read as one of this protocol's completions, or none for another's.
inline std::optional<Completion> completion(const ::xpute::Completion &c) noexcept {
    Result<std::uint32_t> res = answer_of(c.res);
    if (c.cmd == static_cast<std::uint32_t>(Command::FS_OPEN)) {
        std::optional<FsOpenReply> reply;
        if (!c.pkt.empty()) {
            if (Result<FsOpenReply> r = FsOpenReply::read(c.pkt)) reply = *r;
        }
        return Completion{FsOpenCompletion{.fd = c.tag, .res = res, .reply = reply}};
    }
    if (c.cmd == static_cast<std::uint32_t>(Command::FS_CLOSE)) {
        return Completion{FsCloseCompletion{.fd = c.tag, .res = res}};
    }
    return std::nullopt;
}

} // namespace xpute::abi::fs
