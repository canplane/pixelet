// cpp/xpute/core/ipc/file.hpp

// A core's open files. A descriptor is a table handle, so a completion of one
// already closed reaches no one.

#pragma once

#include <cstddef>
#include <cstdint>
#include <optional>
#include <span>
#include <string_view>
#include <utility>
#include <vector>

#include "xpute/core/abi/fs.spec.hpp"
#include "xpute/core/abi/handle.hpp"
#include "xpute/kit/status/bug.hpp"
#include "xpute/kit/status/error.hpp"
#include "door.hpp"
#include "fs.hpp"
#include "xpute/core/call.hpp"

namespace xpute {

// What an open came to, read once from the runtime's completion.
struct Opened {
    enum class Kind : std::uint8_t {
        // The file's size is `bytes`; the descriptor is its owner's to read and close.
        bytes,
        // No such file; the descriptor is gone (`Files::complete_open`).
        absent,
        // Every other failure, `errnum`, and the number the runtime refused with,
        // `detail`; the descriptor is gone.
        failed,
    };

    Kind kind;
    std::uint32_t bytes = 0;
    Errno errnum = Errno::ok;
    std::uint32_t detail = 0;

    bool operator==(const Opened &) const = default;

    static Opened of(const FsOpened &opened) noexcept {
        Result<std::uint32_t> n = answer_of(opened.res);
        if (n) return {.kind = Kind::bytes, .bytes = *n};
        if (n.error().errnum == Errno::enoent) return {.kind = Kind::absent};
        return {.kind = Kind::failed, .errnum = n.error().errnum, .detail = opened.detail};
    }
};

// At most `N - 1` open at once; one more is a bug.
template <class O, std::size_t N> class Files {
  public:
    Fd open(const Door &door, std::uint32_t root, std::string_view path, O owner) {
        Fd fd = take(std::move(owner));
        door.openat(fd, root, fs::flag::O_RDONLY, path);
        return fd;
    }

    // Answered or not.
    std::optional<O> close(const Door &door, Fd fd) {
        door.close(fd);
        return fds_.remove(fd);
    }

    // Opened, written and closed in one turn; the completions reach no owner.
    void write_whole(const Door &door, std::uint32_t root, std::string_view path, std::vector<std::uint8_t> bytes, O owner) {
        Fd fd = take(std::move(owner));
        door.openat(fd, root, fs::flag::O_WRONLY, path);
        door.write(fd, std::move(bytes));
        close(door, fd);
    }

    // The owner of the open `opened` completes, or none where its descriptor was
    // closed since; a failed open leaves no descriptor to close, and its handle
    // is let go here.
    std::optional<O> complete_open(const FsOpened &opened) {
        const O *found = fds_.get(opened.fd);
        if (found == nullptr) return std::nullopt;
        O owner = *found;
        if (opened.res < 0) fds_.remove(opened.fd);
        return owner;
    }

    // The file an open completed with `bytes`, read whole into the heap and
    // closed: its reading, whose bytes `Reading::take` hands over once the runtime's
    // head has passed it, or ENOMEM where the heap has no room for them,
    // nothing read.
    Result<Reading> read_whole(const Door &door, Fd fd, std::uint32_t bytes) {
        std::optional<Bytes> buf = Bytes::make(bytes);
        if (!buf) {
            close(door, fd);
            return make_error(ErrorClass::fault, Errno::enomem);
        }
        Reading read = door.read_buf(fd, std::move(*buf));
        close(door, fd);
        return read;
    }

    const O *owner(Fd fd) const noexcept { return fds_.get(fd); }

    template <class Which> std::optional<Fd> find(Which &&which) {
        std::optional<Fd> found;
        fds_.each([&](std::uint32_t fd, O &o) {
            if (!found && which(o)) found = fd;
        });
        return found;
    }

  private:
    Fd take(O owner) noexcept {
        std::optional<Fd> fd = fds_.insert(std::move(owner));
        ensure(fd.has_value(), Errno::enospc, N);
        return *fd;
    }

    abi::Table<O, N> fds_;
};

} // namespace xpute
