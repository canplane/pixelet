// cpp/xpute/runtime/ipc/file.hpp

// A guest's files: the descriptors it holds, each issued from one table with
// whatever the program says it is for, over the system calls (sys.hpp).
//
// The descriptor is the table's handle (abi/handle.hpp), so the host's answer
// to an open (SYS_OPENED) is looked up by it and reaches the descriptor's
// owner; an answer to a descriptor already closed reaches no one, since its
// generation is gone from the table and a descriptor issued since in the same
// slot has another. Nothing is said of a closed descriptor after, so this is
// the whole of a stale answer's handling.
//
// What an owner is, and where an answer goes once it is found, is the
// program's: `O` is its own.

#pragma once

#include <optional>
#include <string_view>
#include <utility>
#include <vector>

#include "../abi/handle.hpp"
#include "sys.hpp"

namespace xpute {

template <class O> class Files {
  public:
    // Opens `path` below `root` to read, for `owner`.
    Fd open(Sys &sys, std::uint32_t root, std::string_view path, O owner) {
        Fd fd = held_.insert(std::move(owner));
        sys.openat(fd, root, sys::flag::O_RDONLY, path);
        return fd;
    }

    // Ends `fd`, answered or not, and hands back its owner.
    std::optional<O> close(Sys &sys, Fd fd) {
        sys.close(fd);
        return held_.remove(fd);
    }

    // `bytes` replace the file at `path` below `root`: opened, written and
    // closed in one turn, so no answer is waited for and none comes.
    void write_whole(Sys &sys, std::uint32_t root, std::string_view path, std::vector<std::uint8_t> bytes, O owner) {
        Fd fd = held_.insert(std::move(owner));
        sys.openat(fd, root, sys::flag::O_WRONLY, path);
        sys.write(fd, std::move(bytes));
        close(sys, fd);
    }

    // Whose an answer to an open is: none for a descriptor already closed.
    const O *owner(Fd fd) const noexcept { return held_.get(fd); }

    // The descriptor held for an owner `which` picks, if one is.
    template <class Which> std::optional<Fd> held_for(Which &&which) {
        std::optional<Fd> found;
        held_.each([&](std::uint32_t fd, O &o) {
            if (!found && which(o)) found = fd;
        });
        return found;
    }

  private:
    Slots<O> held_;
};

} // namespace xpute
