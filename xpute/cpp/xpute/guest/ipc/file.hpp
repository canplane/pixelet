// cpp/xpute/guest/ipc/file.hpp

// A guest's open files. A descriptor is a table handle, so an answer to one
// already closed reaches no one, which is the whole of a stale answer's handling.

#pragma once

#include <cstddef>
#include <optional>
#include <string_view>
#include <utility>
#include <vector>

#include "xpute/guest/abi/handle.hpp"
#include "sys.hpp"

namespace xpute {

// At most `N - 1` open at once (`Slots`).
template <class O, std::size_t N> class Files {
  public:
    Fd open(Sys &sys, std::uint32_t root, std::string_view path, O owner) {
        Fd fd = held_.insert(std::move(owner));
        sys.openat(fd, root, sys::flag::O_RDONLY, path);
        return fd;
    }

    // Answered or not.
    std::optional<O> close(Sys &sys, Fd fd) {
        sys.close(fd);
        return held_.remove(fd);
    }

    // Opened, written and closed in one turn, so no answer comes.
    void write_whole(Sys &sys, std::uint32_t root, std::string_view path, std::vector<std::uint8_t> bytes, O owner) {
        Fd fd = held_.insert(std::move(owner));
        sys.openat(fd, root, sys::flag::O_WRONLY, path);
        sys.write(fd, std::move(bytes));
        close(sys, fd);
    }

    // None for a descriptor already closed.
    const O *owner(Fd fd) const noexcept { return held_.get(fd); }

    template <class Which> std::optional<Fd> held_for(Which &&which) {
        std::optional<Fd> found;
        held_.each([&](std::uint32_t fd, O &o) {
            if (!found && which(o)) found = fd;
        });
        return found;
    }

  private:
    abi::Slots<O, N> held_;
};

} // namespace xpute
