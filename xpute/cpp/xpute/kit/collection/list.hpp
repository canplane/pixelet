// cpp/xpute/kit/collection/list.hpp

// A list of at most `N`, held inline: it never grows, and a push past `N` is a
// bug (ENOSPC). For what a program bounds by a constant, where a vector would
// grow to that bound anyway. Its first `len` slots are live.

#pragma once

#include <algorithm>
#include <cstddef>
#include <memory>
#include <span>
#include <utility>

#include "xpute/kit/status/bug.hpp"

namespace xpute {

template <class T, std::size_t N> class List {
  public:
    constexpr List() noexcept {}
    List(const List &) = delete;
    List &operator=(const List &) = delete;

    // The values move over, and `o` is left empty.
    List(List &&o) noexcept { take(o); }
    List &operator=(List &&o) noexcept {
        if (this != &o) {
            clear();
            take(o);
        }
        return *this;
    }

    ~List() { clear(); }

    std::size_t len() const noexcept { return len_; }
    bool is_empty() const noexcept { return len_ == 0; }

    void push(T val) noexcept {
        ensure(len_ < N, Errno::enospc, N);
        std::construct_at(slot(len_), std::move(val));
        len_++;
    }

    std::span<const T> as_slice() const noexcept { return {slot(0), len_}; }
    std::span<T> as_mut_slice() noexcept { return {slot(0), len_}; }

    // A slice's reads, as a Rust list derefs to its slice.
    T &operator[](std::size_t at) noexcept { return as_mut_slice()[at]; }
    const T &operator[](std::size_t at) const noexcept { return as_slice()[at]; }
    T *begin() noexcept { return slot(0); }
    T *end() noexcept { return slot(len_); }
    const T *begin() const noexcept { return slot(0); }
    const T *end() const noexcept { return slot(len_); }

    // The value at `at`, those after it moved down one: the order kept.
    T remove(std::size_t at) noexcept {
        ensure(at < len_, Errno::einval, at);
        T val = std::move(*slot(at));
        std::destroy_at(slot(at));
        for (std::size_t i = at; i + 1 < len_; i++) move_down(i + 1, i);
        len_--;
        return val;
    }

    // Those `keep` refuses dropped, the order of the rest kept, in one pass.
    template <class Keep> void retain(Keep &&keep) noexcept {
        std::size_t kept = 0;
        for (std::size_t at = 0; at < len_; at++) {
            // A slot below `at` and past `kept` is already empty: refused, or
            // moved down from.
            if (keep(std::as_const(*slot(at)))) {
                if (kept != at) move_down(at, kept);
                kept++;
            } else {
                std::destroy_at(slot(at));
            }
        }
        len_ = kept;
    }

    void clear() noexcept {
        std::destroy(slot(0), slot(len_));
        len_ = 0;
    }

  private:
    T *slot(std::size_t at) noexcept { return reinterpret_cast<T *>(bytes_) + at; }
    const T *slot(std::size_t at) const noexcept { return reinterpret_cast<const T *>(bytes_) + at; }

    // Into the empty slot `to` from the live `from`, which is left empty.
    void move_down(std::size_t from, std::size_t to) noexcept {
        std::construct_at(slot(to), std::move(*slot(from)));
        std::destroy_at(slot(from));
    }

    void take(List &o) noexcept {
        for (std::size_t i = 0; i < o.len_; i++) std::construct_at(slot(i), std::move(*o.slot(i)));
        len_ = o.len_;
        o.clear();
    }

    // Uninitialized; a value is built into a slot by `push` and ended by what
    // takes it out. One byte where `N` is 0, as an array is never empty.
    alignas(T) std::byte bytes_[std::max<std::size_t>(N, 1) * sizeof(T)];
    std::size_t len_ = 0;
};

} // namespace xpute
