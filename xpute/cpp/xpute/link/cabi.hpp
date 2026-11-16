// cpp/xpute/link/cabi.hpp

// A packet's values in the Component Model's Canonical ABI layout, as
// xpute-link's `cabi.rs` lays them: each at its type's alignment,
// little-endian, a record's fields in order, a variant its discriminant and
// then its payload. A string's or a list's bytes lie past the packet's fixed
// part, and their pointer is an offset from the packet's start. A packet past
// the bytes it is given spoils, and `Lower::finish` answers EMSGSIZE. A packet
// that does not hold what its type says spoils its reading, whose reads answer
// zero from then on, and `Lift::finish` answers EBADMSG.

#pragma once

#include <bit>
#include <concepts>
#include <cstddef>
#include <cstdint>
#include <cstring>
#include <optional>
#include <span>
#include <string_view>
#include <utility>
#include <vector>

#include "xpute/kit/codec/encoding.hpp"
#include "xpute/kit/status/error.hpp"

namespace xpute::link {

static_assert(std::endian::native == std::endian::little, "a packet's numbers are copied as the machine holds them");

// A number a list holds, whose bytes, every one set, are its value
// little-endian: a list of them crosses as its bytes, in one copy.
template <class T>
concept Number = (std::integral<T> && !std::same_as<T, bool>) || std::floating_point<T>;

// A value one put or get carries: a number, or a bool as one byte.
template <class T>
concept Scalar = Number<T> || std::same_as<T, bool>;

// Writes a packet into the bytes it is given: its fixed part first
// (`start`), then the bytes of its strings and lists past it. A writer
// nothing was written by answers 0, no packet.
class Lower {
  public:
    // A writer over `buf`, nothing written there yet.
    explicit Lower(std::span<std::uint8_t> buf) noexcept : buf_(buf), end_(0), overflow_(false) {}

    // A packet of a fixed part `fixed_sz` long, zeroed, in `buf`.
    Lower(std::span<std::uint8_t> buf, std::size_t fixed_sz) noexcept : Lower(buf) { start(fixed_sz); }

    // Starts the packet with a fixed part `fixed_sz` long, zeroed.
    void start(std::size_t fixed_sz) noexcept {
        overflow_ = fixed_sz > buf_.size();
        if (!overflow_) std::memset(buf_.data(), 0, fixed_sz);
        end_ = fixed_sz;
    }

    template <Scalar T> void put(std::size_t at, T v) noexcept {
        if (overflow_ || at + sizeof v > buf_.size()) {
            overflow_ = true;
            return;
        }
        if constexpr (std::same_as<T, bool>) buf_[at] = v ? 1 : 0;
        else std::memcpy(buf_.data() + at, &v, sizeof v);
    }

    // Takes `len` elements of `sz` bytes at `align` past what is written,
    // zeroed, and puts their offset and count at `at`: a list's or a
    // string's pointer and length. It answers where they begin, or none
    // where they run past the bytes it is given.
    std::optional<std::size_t> list(std::size_t at, std::size_t len, std::size_t sz, std::size_t align) noexcept {
        std::size_t base = (end_ + align - 1) / align * align;
        std::size_t end = base + len * sz;
        if (overflow_ || end > buf_.size()) {
            overflow_ = true;
            return std::nullopt;
        }
        std::memset(buf_.data() + end_, 0, end - end_);
        end_ = end;
        put(at, static_cast<std::uint32_t>(base));
        put(at + 4, static_cast<std::uint32_t>(len));
        return base;
    }

    // Puts the numbers `v` past what is written, at their alignment, and
    // their offset and count at `at`.
    template <Number T> void arr(std::size_t at, std::span<const T> v) noexcept {
        auto base = list(at, v.size(), sizeof(T), sizeof(T));
        if (base && !v.empty()) std::memcpy(buf_.data() + *base, v.data(), v.size_bytes());
    }

    void str(std::size_t at, std::string_view s) noexcept {
        arr<std::uint8_t>(at, {reinterpret_cast<const std::uint8_t *>(s.data()), s.size()});
    }

    // The packet's length, or EMSGSIZE where it is past the bytes it is given.
    Result<std::size_t> finish() const noexcept {
        if (overflow_) return make_error(Errno::emsgsize);
        return end_;
    }

  private:
    std::span<std::uint8_t> buf_;
    std::size_t end_;
    bool overflow_;
};

// Reads a packet's values where its type says they lie. What lies past the
// packet, a misaligned list, a string not UTF-8 or a discriminant that names
// no case spoils the reading.
class Lift {
  public:
    explicit Lift(std::span<const std::uint8_t> pkt) noexcept : pkt_(pkt) {}

    // The packet's bytes, or none for no packet.
    std::optional<std::span<const std::uint8_t>> pkt() const noexcept {
        if (pkt_.empty()) return std::nullopt;
        return pkt_;
    }

    // A bool reads 0 as false and any other byte as true, as the ABI lifts
    // it.
    template <Scalar T> T get(std::size_t at) noexcept {
        T v{};
        if (malformed_ || at + sizeof v > pkt_.size()) {
            malformed_ = true;
            return v;
        }
        if constexpr (std::same_as<T, bool>) v = pkt_[at] != 0;
        else std::memcpy(&v, pkt_.data() + at, sizeof v);
        return v;
    }

    // A discriminant of `n` cases at `at`, its width by `n`.
    std::uint32_t case_(std::size_t at, std::size_t n) noexcept {
        std::uint32_t d = n <= 256 ? get<std::uint8_t>(at) : n <= 65536 ? get<std::uint16_t>(at) : get<std::uint32_t>(at);
        if (d < n) return d;
        malformed_ = true;
        return 0;
    }

    // The elements of `sz` bytes at `align` whose offset and count lie at
    // `at`: where they begin, and how many.
    std::pair<std::size_t, std::size_t> list(std::size_t at, std::size_t sz, std::size_t align) noexcept {
        std::size_t base = get<std::uint32_t>(at);
        std::uint64_t len = get<std::uint32_t>(at + 4);
        if (malformed_ || base % align != 0 || base > pkt_.size() || len * sz > pkt_.size() - base) {
            malformed_ = true;
            return {0, 0};
        }
        return {base, static_cast<std::size_t>(len)};
    }

    // A list of numbers at `at`, copied out.
    template <Number T> std::vector<T> arr(std::size_t at) noexcept {
        auto [base, len] = list(at, sizeof(T), sizeof(T));
        std::vector<T> v(len);
        if (len != 0) std::memcpy(v.data(), pkt_.data() + base, len * sizeof(T));
        return v;
    }

    std::string_view str(std::size_t at) noexcept {
        auto [base, len] = list(at, 1, 1);
        std::string_view s(reinterpret_cast<const char *>(pkt_.data() + base), len);
        if (is_utf8(s)) return s;
        malformed_ = true;
        return {};
    }

    // Nothing, or EBADMSG where the packet did not hold what was read.
    Result<void> finish() const noexcept {
        if (malformed_) return make_error(Errno::ebadmsg);
        return {};
    }

  private:
    std::span<const std::uint8_t> pkt_;
    bool malformed_ = false;
};

} // namespace xpute::link
