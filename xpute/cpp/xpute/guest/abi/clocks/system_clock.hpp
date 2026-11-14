// cpp/xpute/guest/abi/clocks/system_clock.hpp
//
// GENERATED from wit/clocks (xpute-link-gen wit.rs)

#pragma once

#include <cstddef>
#include <cstdint>
#include <optional>

#include "xpute/guest/link.hpp"
#include "xpute/kit/status/error.hpp"
#include "xpute/link/cabi.hpp"
#include "xpute/link/link.hpp"

namespace xpute::abi::clocks::system_clock {

namespace _rt {
using ::xpute::Result;
using ::xpute::guest::call;
using ::xpute::link::Handle;
using ::xpute::link::Lift;
using ::xpute::link::Lower;
using ::xpute::link::Method;
using ::xpute::link::answer;
} // namespace _rt

// The interface's handle, named by `xpute:clocks/system-clock` (frame.json5 `NAMED`).
inline constexpr _rt::Handle HANDLE = 0x7dfa61;

// `now`
inline constexpr _rt::Method NOW = 0;

struct Instant {
    std::int64_t seconds;
    std::uint32_t nanoseconds;

    static constexpr std::size_t SZ = 16;
    static constexpr std::size_t ALIGN = 8;

    void store(_rt::Lower &w, std::size_t at) const noexcept {
        w.put<std::int64_t>(at, this->seconds);
        w.put<std::uint32_t>(at + 8, this->nanoseconds);
    }

    static Instant load(_rt::Lift &r, std::size_t at) noexcept {
        return Instant{
            .seconds = r.get<std::int64_t>(at),
            .nanoseconds = r.get<std::uint32_t>(at + 8),
        };
    }
};

inline _rt::Result<::xpute::abi::clocks::system_clock::Instant> now() noexcept {
    auto [result_, data_] = _rt::call(HANDLE, NOW, std::nullopt);
    return _rt::answer(result_, data_, [&](_rt::Lift &r_) { return ::xpute::abi::clocks::system_clock::Instant::load(r_, 0); });
}

} // namespace xpute::abi::clocks::system_clock
