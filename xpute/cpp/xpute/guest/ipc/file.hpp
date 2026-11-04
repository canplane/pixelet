// cpp/xpute/guest/ipc/file.hpp

// A guest's open files. A descriptor is a table handle, so an answer to one
// already closed reaches no one.

#pragma once

#include <cstddef>
#include <cstdint>
#include <optional>
#include <span>
#include <string_view>
#include <utility>
#include <vector>

#include "xpute/guest/abi/fs.spec.hpp"
#include "xpute/guest/abi/handle.hpp"
#include "xpute/kit/status/bug.hpp"
#include "xpute/kit/status/error.hpp"
#include "door.hpp"
#include "fs.hpp"
#include "xpute/guest/hostcall.hpp"

namespace xpute {

// What an open came to, read once from the host's answer.
struct Opened {
    enum class Kind : std::uint8_t {
        // The file's size is `bytes`; the descriptor is its owner's to read and close.
        bytes,
        // No such file; the descriptor is closed (`Files::complete_open`).
        absent,
        // Every other failure, `errnum`, and the number the host refused with,
        // `detail`; the descriptor is closed.
        failed,
    };

    Kind kind;
    std::uint32_t bytes = 0;
    Errno errnum = Errno::ok;
    std::uint32_t detail = 0;

    bool operator==(const Opened &) const = default;

    static Opened of(const abi::fs::FsOpened &opened) noexcept {
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
        return held_.remove(fd);
    }

    // Opened, written and closed in one turn, so no answer comes.
    void write_whole(const Door &door, std::uint32_t root, std::string_view path, std::vector<std::uint8_t> bytes, O owner) {
        Fd fd = take(std::move(owner));
        door.openat(fd, root, fs::flag::O_WRONLY, path);
        door.write(fd, std::move(bytes));
        close(door, fd);
    }

    // The owner of the open `opened` completes, or none where its descriptor was
    // closed since; a descriptor whose open failed is closed here.
    std::optional<O> complete_open(const Door &door, const abi::fs::FsOpened &opened) {
        const O *held = held_.get(opened.fd);
        if (held == nullptr) return std::nullopt;
        O owner = *held;
        if (opened.res < 0) close(door, opened.fd);
        return owner;
    }

    // The file an open answered with `bytes`, read whole into the heap and
    // closed: its reading, whose bytes `Reading::take` hands over once the host's
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

    const O *owner(Fd fd) const noexcept { return held_.get(fd); }

    template <class Which> std::optional<Fd> held_for(Which &&which) {
        std::optional<Fd> found;
        held_.each([&](std::uint32_t fd, O &o) {
            if (!found && which(o)) found = fd;
        });
        return found;
    }

  private:
    Fd take(O owner) noexcept {
        std::optional<Fd> fd = held_.insert(std::move(owner));
        ensure(fd.has_value(), Errno::enospc, N);
        return *fd;
    }

    abi::Table<O, N> held_;
};

} // namespace xpute
