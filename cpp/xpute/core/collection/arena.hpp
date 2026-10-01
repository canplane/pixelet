// cpp/xpute/core/collection/arena.hpp

// A pool of constructed slots, bump-allocated and rewound whole.
//
// Slots are built once and live as long as the arena: `truncate` moves the
// cursor and destroys nothing, so whatever a slot owns is still allocated when
// the slot is handed out again. For work whose lifetime is one pass, that is
// one rewind against a free and an allocation per object. The cost is the
// caller's: a slot arrives in whatever state its last user left it.
//
// The slots' memory comes from `Alloc`, which the caller names and which has
// no default: a guest's comes from its own heap, and a default would be the
// system's.

#pragma once

#include <algorithm>
#include <cstdint>
#include <limits>
#include <vector>

#include "../status/bug.hpp"
#include "../status/error.hpp"

namespace xpute {

struct ArenaOptions {
    // Slots built at construction; growth passes it.
    std::uint32_t init_cap = 1 << 10;
    // Growth past it is refused.
    std::uint32_t max_cap = 1 << 24;
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

    T &get(std::uint32_t idx) noexcept { return mem_[idx]; }

    // Hands out the next slot.
    Result<T *> alloc() {
        if (len_ >= cap_) {
            if (Result<void> r = grow(std::max(saturating_mul2(cap_), 1u)); !r) return r.error();
        }
        return &mem_[len_++];
    }

    // Grows now so that the next `cnt` allocs do not.
    Result<void> reserve(std::uint32_t cnt) {
        std::uint32_t req = len_ > std::numeric_limits<std::uint32_t>::max() - cnt ? std::numeric_limits<std::uint32_t>::max() : len_ + cnt;
        if (req > cap_) return grow(std::max(saturating_mul2(cap_), req));
        return {};
    }

    // Rewinds the cursor. Slots above `new_len` stay constructed. One above
    // the cursor is a mistake rather than a resize request, so it is a
    // broken invariant and not a clamp.
    void truncate(std::uint32_t new_len) noexcept {
        ensure(new_len <= len_, Errno::einval, new_len, len_);
        len_ = new_len;
    }

    // Marks here and rewinds to the mark when the guard goes.
    class Scope {
      public:
        explicit Scope(Arena &arena) noexcept : arena_(arena), mark_(arena.len()) {}
        ~Scope() { arena_.truncate(mark_); }
        Scope(const Scope &) = delete;
        Scope &operator=(const Scope &) = delete;
        Arena &operator*() noexcept { return arena_; }
        Arena *operator->() noexcept { return &arena_; }

      private:
        Arena &arena_;
        std::uint32_t mark_;
    };

    Scope scope() noexcept { return Scope(*this); }

  private:
    Arena(std::uint32_t max_cap, const Alloc &alloc) : mem_(alloc), max_cap_(max_cap) {}

    // Doubling that saturates, so one past u32 lands above max_cap and is
    // refused there rather than wrapping under it and passing.
    static std::uint32_t saturating_mul2(std::uint32_t x) noexcept {
        return x > std::numeric_limits<std::uint32_t>::max() / 2 ? std::numeric_limits<std::uint32_t>::max() : x * 2;
    }

    Result<void> grow(std::uint32_t new_cap) {
        if (new_cap <= cap_) return {};
        if (new_cap > max_cap_) return marshal_error(Errno::eoverflow);
        mem_.resize(new_cap);
        cap_ = new_cap;
        return {};
    }

    std::vector<T, Alloc> mem_;
    std::uint32_t max_cap_;
    std::uint32_t cap_ = 0;
    std::uint32_t len_ = 0;
};

} // namespace xpute
