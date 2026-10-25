// cpp/xpute/kit/collection/arena.hpp

// A pool of constructed slots, bump-allocated and rewound whole. `truncate`
// destroys nothing, so a slot arrives in whatever state its last user left it.
// `Alloc` has no default, since a default would be the system's heap.

#pragma once

#include <algorithm>
#include <cstdint>
#include <limits>
#include <span>
#include <vector>

#include "xpute/kit/status/bug.hpp"
#include "xpute/kit/status/error.hpp"

namespace xpute {

template <class T, class Alloc> class ArenaGuard;

struct ArenaOptions {
    std::uint32_t init_cap;
    std::uint32_t max_cap;
};

template <class T, class Alloc> class Arena {
  public:
    static Result<Arena> make(ArenaOptions opts, const Alloc &alloc = Alloc()) {
        Arena arena(opts.max_cap, alloc);
        if (Result<void> r = arena.grow(std::min(opts.init_cap, opts.max_cap)); !r) return r.error();
        return arena;
    }

    std::uint32_t cap() const noexcept { return cap_; }
    std::uint32_t len() const noexcept { return len_; }
    bool is_empty() const noexcept { return len_ == 0; }
    std::uint32_t max_cap() const noexcept { return max_cap_; }

    T &get(std::uint32_t idx) noexcept {
        ensure(idx < mem_.size(), Errno::enotrecoverable, idx, mem_.size());
        return mem_[idx];
    }
    std::span<T> mem() noexcept { return mem_; }
    void set_max_cap(std::uint32_t cap) noexcept { max_cap_ = cap; }

    Result<T *> alloc() {
        if (len_ >= cap_) {
            if (Result<void> r = grow(len_ + 1); !r) return r.error();
        }
        return &mem_[len_++];
    }

    Result<void> reserve(std::uint32_t cnt) {
        std::uint32_t req = len_ > std::numeric_limits<std::uint32_t>::max() - cnt ? std::numeric_limits<std::uint32_t>::max() : len_ + cnt;
        return grow(req);
    }

    // Past the cursor is a broken invariant, not a resize request.
    void truncate(std::uint32_t new_len) noexcept {
        ensure(new_len <= len_, Errno::einval, new_len, len_);
        len_ = new_len;
    }

    // Marks here and rewinds to the mark when the guard goes.
    ArenaGuard<T, Alloc> scope() noexcept;

  private:
    Arena(std::uint32_t max_cap, const Alloc &alloc) : mem_(alloc), max_cap_(max_cap) {}

    // Saturates, so one past u32 is refused at max_cap rather than wrapping under it.
    // Doubling stops at the cap, so the cap refuses only what does not fit.
    Result<void> grow(std::uint32_t req) {
        if (req <= cap_) return {};
        if (req > max_cap_) return marshal_error(Errno::eoverflow);
        std::uint32_t new_cap = std::clamp(cap_ > max_cap_ / 2 ? max_cap_ : cap_ * 2, req, max_cap_);
        mem_.resize(new_cap);
        cap_ = new_cap;
        return {};
    }

    std::vector<T, Alloc> mem_;
    std::uint32_t max_cap_;
    std::uint32_t cap_ = 0;
    std::uint32_t len_ = 0;
};

template <class T, class Alloc> class ArenaGuard {
  public:
    explicit ArenaGuard(Arena<T, Alloc> &arena) noexcept : arena_(arena), mark_(arena.len()) {}
    ~ArenaGuard() { arena_.truncate(mark_); }
    ArenaGuard(const ArenaGuard &) = delete;
    ArenaGuard &operator=(const ArenaGuard &) = delete;
    Arena<T, Alloc> &operator*() noexcept { return arena_; }
    Arena<T, Alloc> *operator->() noexcept { return &arena_; }

  private:
    Arena<T, Alloc> &arena_;
    std::uint32_t mark_;
};

template <class T, class Alloc> ArenaGuard<T, Alloc> Arena<T, Alloc>::scope() noexcept {
    return ArenaGuard<T, Alloc>(*this);
}

} // namespace xpute
