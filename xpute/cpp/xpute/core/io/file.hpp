// cpp/xpute/core/io/file.hpp

// `FileTable` holds a core's open files and the requests it awaits, two
// things apart. An open file is a descriptor, a generational handle
// (abi/handle.hpp), with the owner `O` the program gives it. A descriptor is
// let go as its close is sent, as POSIX `close()` lets one go, and what then
// awaits the close's completion is a request `R`, found by its tag, the
// descriptor with its generation, as io_uring finds a completion by its
// `user_data`: a slot taken again issues another tag, so no old completion
// reaches its new holder. A read's request is held with the
// buffer it lent the runtime, `B`, whose bytes stay where they are while it
// moves, as a `Buffer`'s do; the table owns it until the close is completed.
// The file calls are fs's generated functions, over the core's door, and each
// completion, which fs's generated `completion` reads, hands back what it is
// owed: an open's to its owner with what the open came to (`complete_open`), a
// close's to its request with the buffer, filled, and the close's result
// (`complete_close`).

#pragma once

#include <cstddef>
#include <cstdint>
#include <new>
#include <optional>
#include <span>
#include <tuple>
#include <utility>
#include <vector>

#include "xpute/core/abi/fs.spec.hpp"
#include "xpute/core/abi/handle.hpp"
#include "xpute/kit/status/bug.hpp"
#include "xpute/kit/status/error.hpp"
#include "door.hpp"
#include "fs.hpp"
#include "xpute/core/host.hpp"

namespace xpute {

// A buffer a read fills, on the heap, so its bytes stay where they are while
// it moves, as a request lends one (`FileTable::read_whole`). They are given
// back to the heap with their size.
class Buffer {
  public:
    Buffer() noexcept = default;
    Buffer(Buffer &&o) noexcept : at_(std::exchange(o.at_, nullptr)), cnt_(std::exchange(o.cnt_, 0)) {}
    Buffer &operator=(Buffer &&o) noexcept {
        if (this != &o) {
            release();
            at_ = std::exchange(o.at_, nullptr);
            cnt_ = std::exchange(o.cnt_, 0);
        }
        return *this;
    }
    Buffer(const Buffer &) = delete;
    Buffer &operator=(const Buffer &) = delete;
    ~Buffer() { release(); }

    // `make` answers none when the heap has no room.
    static std::optional<Buffer> make(std::uint32_t cnt) noexcept {
        Buffer b;
        if (cnt == 0) return b;
        b.at_ = static_cast<std::uint8_t *>(::operator new(cnt, std::nothrow));
        if (b.at_ == nullptr) return std::nullopt;
        b.cnt_ = cnt;
        return b;
    }

    std::uint8_t *data() noexcept { return at_; }
    const std::uint8_t *data() const noexcept { return at_; }
    std::size_t size() const noexcept { return cnt_; }
    std::span<std::uint8_t> span() noexcept { return {at_, cnt_}; }
    std::span<const std::uint8_t> span() const noexcept { return {at_, cnt_}; }

  private:
    void release() noexcept {
        if (at_ != nullptr) ::operator delete(at_, cnt_);
        at_ = nullptr;
        cnt_ = 0;
    }

    std::uint8_t *at_ = nullptr;
    std::size_t cnt_ = 0;
};

// `OpenResult` is what an open to read came to, read once from the runtime's
// completion.
struct OpenResult {
    enum class Kind : std::uint8_t {
        // The file's size is `bytes`; the descriptor is its owner's to read
        // or close.
        bytes,
        // No such file; the descriptor is gone.
        absent,
        // Every other failure, `errnum`, and the number the runtime refused
        // with, `detail`; the descriptor is gone.
        failed,
    };

    Kind kind;
    std::uint32_t bytes = 0;
    Errno errnum = Errno::ok;
    std::uint32_t detail = 0;

    bool operator==(const OpenResult &) const = default;

    // What an open completed with `res` came to, and `detail`, the number the
    // runtime refused it with.
    static OpenResult of(const Result<std::uint32_t> &res, std::uint32_t detail) noexcept {
        if (res) return {.kind = Kind::bytes, .bytes = res.value()};
        if (res.error().errnum == Errno::enoent) return {.kind = Kind::absent};
        return {.kind = Kind::failed, .errnum = res.error().errnum, .detail = detail};
    }
};

// At most `N - 1` files are open at once; opening one more is a bug. A
// written file takes no slot.
template <class O, class R, class B, std::size_t N> class FileTable {
  public:
    // Opens the file at `path` to be read whole.
    Fd open(std::span<const Token> path, O owner) {
        std::optional<Fd> fd = fds_.insert(std::move(owner));
        ensure(fd.has_value(), Errno::emfile, N);
        abi::fs::fs_open(door::get(), *fd, abi::fs::FsOpen{.path = path, .flags = fs::flag::O_RDONLY});
        return *fd;
    }

    // Replaces the file at `path` with `bytes`: opens it to write, writes and
    // closes it in one turn, the bytes going with their frame. The descriptor
    // is let go as its close is sent, and `request` hears the close's result
    // (`complete_close`). A tag still awaited, as many writes on as a
    // generation counts, is a bug.
    Fd write_whole(std::span<const Token> path, std::span<const std::uint8_t> bytes, R request) {
        const Door &door = door::get();
        Fd fd = door.tag();
        for (const auto &pending : requests_) ensure(std::get<0>(pending) != fd, Errno::emfile, fd);
        abi::fs::fs_open(door, fd, abi::fs::FsOpen{.path = path, .flags = fs::flag::O_WRONLY});
        abi::fs::fs_write(door, fd, abi::fs::FsWrite{.at = door.mem().off_of(bytes), .len = static_cast<std::uint32_t>(bytes.size())});
        abi::fs::fs_close(door, fd);
        requests_.emplace_back(fd, std::move(request), std::nullopt);
        return fd;
    }

    // Reads the open file `fd` whole into `buf` and closes it. The descriptor
    // is let go as its close is sent, and `request` is held with the buffer
    // until the close is completed, which hands both back, the buffer filled;
    // the buffer's bytes stay where they are while it moves. A descriptor not
    // open is let be, and `buf` dropped.
    void read_whole(Fd fd, B buf, R request) {
        if (!fds_.remove(fd)) return;
        const Door &door = door::get();
        std::span<std::uint8_t> bytes = buf.span();
        requests_.emplace_back(fd, std::move(request), std::move(buf));
        abi::fs::fs_read(door, fd, abi::fs::FsRead{.at = door.mem().off_of(bytes), .len = static_cast<std::uint32_t>(bytes.size())});
        abi::fs::fs_close(door, fd);
    }

    // Closes the open file `fd`, whose owner hears nothing more of it; an open
    // not yet completed is completed with ECANCELED. Answers EBADF for a
    // descriptor not open.
    Result<void> close(Fd fd) {
        if (!fds_.remove(fd)) return make_error(ErrorClass::fault, Errno::ebadf);
        abi::fs::fs_close(door::get(), fd);
        return {};
    }

    // Hands the owner of the open of `fd`, completed with `res` and the
    // runtime's `detail`, what it came to, or none where the descriptor is no
    // longer open. A failed open leaves no descriptor to close, so its handle
    // is let go here.
    std::optional<std::pair<O, OpenResult>> complete_open(Fd fd, const Result<std::uint32_t> &res, std::uint32_t detail) {
        const O *found = fds_.get(fd);
        if (found == nullptr) return std::nullopt;
        O owner = *found;
        if (!res) fds_.remove(fd);
        return std::pair{std::move(owner), OpenResult::of(res, detail)};
    }

    // Hands back the request whose close of `fd` completed with `res`, with a
    // read's buffer, or none for a close no request awaits.
    std::optional<std::tuple<R, std::optional<B>, Errno>> complete_close(Fd fd, const Result<std::uint32_t> &res) {
        for (std::size_t k = 0; k < requests_.size(); k++) {
            if (std::get<0>(requests_[k]) != fd) continue;
            auto [tag, request, buf] = std::move(requests_[k]);
            requests_[k] = std::move(requests_.back());
            requests_.pop_back();
            return std::tuple{std::move(request), std::move(buf), res ? Errno::ok : res.error().errnum};
        }
        return std::nullopt;
    }

    // The open file whose owner `which` picks.
    template <class Which> std::optional<Fd> find(Which &&which) {
        std::optional<Fd> found;
        fds_.each([&](std::uint32_t fd, O &o) {
            if (!found && which(o)) found = fd;
        });
        return found;
    }

  private:
    abi::Table<O, N> fds_;
    // Each request whose close is not yet completed, by its tag, with the
    // buffer a read lent the runtime.
    std::vector<std::tuple<Fd, R, std::optional<B>>> requests_;
};

} // namespace xpute
