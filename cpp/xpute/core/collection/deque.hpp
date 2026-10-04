// cpp/xpute/core/collection/deque.hpp

// A ring over a buffer the caller owns; it allocates nothing and never grows.

#pragma once

#include <cstdint>
#include <optional>
#include <span>
#include <utility>

#include "../status/bug.hpp"

namespace xpute {

template <class T> struct Deque {
    std::span<T> buf;
    std::uint32_t head = 0;
    std::uint32_t len = 0;

    std::uint32_t cap() const noexcept { return static_cast<std::uint32_t>(buf.size()); }
    bool is_empty() const noexcept { return len == 0; }
    bool is_full() const noexcept { return len == cap(); }

    const T *front() const noexcept { return len == 0 ? nullptr : &buf[head]; }
    const T *back() const noexcept { return len == 0 ? nullptr : &buf[(head + len - 1) % cap()]; }

    // A full deque is the caller's bug.
    void push_back(T v) noexcept {
        ensure(!is_full(), Errno::enospc, len);
        buf[(head + len) % cap()] = std::move(v);
        len++;
    }

    void push_front(T v) noexcept {
        ensure(!is_full(), Errno::enospc, len);
        head = (head + cap() - 1) % cap();
        buf[head] = std::move(v);
        len++;
    }

    std::optional<T> pop_front() noexcept {
        if (len == 0) return std::nullopt;
        T v = std::exchange(buf[head], T{});
        head = (head + 1) % cap();
        len--;
        return v;
    }

    std::optional<T> pop_back() noexcept {
        if (len == 0) return std::nullopt;
        len--;
        return std::exchange(buf[(head + len) % cap()], T{});
    }

    void clear() noexcept {
        for (std::size_t i = 0; i < len; i++) buf[(head + i) % cap()] = T{};
        head = 0;
        len = 0;
    }
};

} // namespace xpute
