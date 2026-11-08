// cpp/xpute/kit/collection/list.hpp

// A list holds at most `N` values, inline. It never grows: a push past `N` is a
// bug (ENOSPC). Its first `len` slots hold values, and the rest are
// uninitialized.

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

    // Moving a list moves its values over and leaves `o` empty.
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

    // Indexing and iteration reach the live values, as a Rust `List` derefs to
    // its slice.
    T &operator[](std::size_t at) noexcept { return as_mut_slice()[at]; }
    const T &operator[](std::size_t at) const noexcept { return as_slice()[at]; }
    T *begin() noexcept { return slot(0); }
    T *end() noexcept { return slot(len_); }
    const T *begin() const noexcept { return slot(0); }
    const T *end() const noexcept { return slot(len_); }

    // Takes out the value at `at` and moves every value after it down one
    // slot, so the order is kept.
    T remove(std::size_t at) noexcept {
        ensure(at < len_, Errno::einval, at);
        T val = std::move(*slot(at));
        std::destroy_at(slot(at));
        for (std::size_t i = at; i + 1 < len_; i++) move_down(i + 1, i);
        len_--;
        return val;
    }

    // Destroys every value `keep` refuses and keeps the order of the rest, in
    // one pass.
    template <class Keep> void retain(Keep &&keep) noexcept {
        std::size_t live = 0;
        for (std::size_t at = 0; at < len_; at++) {
            // Every slot from `live` up to `at` is already empty: its value
            // was either refused and destroyed, or moved down.
            if (keep(std::as_const(*slot(at)))) {
                if (live != at) move_down(at, live);
                live++;
            } else {
                std::destroy_at(slot(at));
            }
        }
        len_ = live;
    }

    void clear() noexcept {
        std::destroy(slot(0), slot(len_));
        len_ = 0;
    }

  private:
    T *slot(std::size_t at) noexcept { return reinterpret_cast<T *>(bytes_) + at; }
    const T *slot(std::size_t at) const noexcept { return reinterpret_cast<const T *>(bytes_) + at; }

    // Moves the value in the live slot `from` into the empty slot `to`, and
    // leaves `from` empty.
    void move_down(std::size_t from, std::size_t to) noexcept {
        std::construct_at(slot(to), std::move(*slot(from)));
        std::destroy_at(slot(from));
    }

    void take(List &o) noexcept {
        for (std::size_t i = 0; i < o.len_; i++) std::construct_at(slot(i), std::move(*o.slot(i)));
        len_ = o.len_;
        o.clear();
    }

    // Raw storage: `push` constructs a value in a slot, and whatever takes the
    // value out destroys it. There is room for one value even when `N` is 0,
    // because C++ has no empty array.
    alignas(T) std::byte bytes_[std::max<std::size_t>(N, 1) * sizeof(T)];
    std::size_t len_ = 0;
};

} // namespace xpute
