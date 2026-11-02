// cpp/xpute/guest/ipc/file.hpp

// A guest's open files. A descriptor is a table handle, so an answer to one
// already closed reaches no one.

#pragma once

#include <cstddef>
#include <cstdint>
#include <new>
#include <optional>
#include <span>
#include <string_view>
#include <utility>
#include <vector>

#include "xpute/guest/abi/cmd.spec.hpp"
#include "xpute/guest/abi/handle.hpp"
#include "xpute/kit/status/bug.hpp"
#include "xpute/kit/status/error.hpp"
#include "sys.hpp"

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

    static Opened of(const abi::SysOpened &opened) noexcept {
        Result<std::uint32_t> n = answer_of(opened.res);
        if (n) return {.kind = Kind::bytes, .bytes = *n};
        if (n.error().errnum == Errno::enoent) return {.kind = Kind::absent};
        return {.kind = Kind::failed, .errnum = n.error().errnum, .detail = opened.detail};
    }
};

// Bytes of the heap, given back by their size: what a read fills.
class Bytes {
  public:
    Bytes() noexcept = default;
    Bytes(Bytes &&o) noexcept : at_(std::exchange(o.at_, nullptr)), cnt_(std::exchange(o.cnt_, 0)) {}
    Bytes &operator=(Bytes &&o) noexcept {
        if (this != &o) {
            release();
            at_ = std::exchange(o.at_, nullptr);
            cnt_ = std::exchange(o.cnt_, 0);
        }
        return *this;
    }
    Bytes(const Bytes &) = delete;
    Bytes &operator=(const Bytes &) = delete;
    ~Bytes() { release(); }

    // None where the heap has no room.
    static std::optional<Bytes> make(std::uint32_t cnt) noexcept {
        Bytes b;
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

// At most `N - 1` open at once; one more is a bug.
template <class O, std::size_t N> class Files {
  public:
    Fd open(Sys &sys, std::uint32_t root, std::string_view path, O owner) {
        Fd fd = take(std::move(owner));
        sys.openat(fd, root, sys::flag::O_RDONLY, path);
        return fd;
    }

    // `open`, or none where `N - 1` are open already.
    std::optional<Fd> try_open(Sys &sys, std::uint32_t root, std::string_view path, O owner) {
        std::optional<Fd> fd = held_.insert(std::move(owner));
        if (fd) sys.openat(*fd, root, sys::flag::O_RDONLY, path);
        return fd;
    }

    // Answered or not.
    std::optional<O> close(Sys &sys, Fd fd) {
        sys.close(fd);
        return held_.remove(fd);
    }

    // Opened, written and closed in one turn, so no answer comes.
    void write_whole(Sys &sys, std::uint32_t root, std::string_view path, std::vector<std::uint8_t> bytes, O owner) {
        Fd fd = take(std::move(owner));
        sys.openat(fd, root, sys::flag::O_WRONLY, path);
        sys.write(fd, std::move(bytes));
        close(sys, fd);
    }

    // The owner of the open `opened` completes, or none where its descriptor was
    // closed since; a descriptor whose open failed is closed here.
    std::optional<O> complete_open(Sys &sys, const abi::SysOpened &opened) {
        const O *held = held_.get(opened.fd);
        if (held == nullptr) return std::nullopt;
        O owner = *held;
        if (opened.res < 0) close(sys, opened.fd);
        return owner;
    }

    // The file an open answered with `bytes`, read whole into the heap and
    // closed: its bytes, filled from the next turn on and kept until then, or
    // ENOMEM where the heap has no room for them, nothing read.
    Result<Bytes> read_whole(Sys &sys, Fd fd, std::uint32_t bytes) {
        std::optional<Bytes> held = Bytes::make(bytes);
        if (held) sys.read(fd, held->span());
        close(sys, fd);
        if (!held) return make_error(ErrorClass::fault, Errno::enomem);
        return std::move(*held);
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
