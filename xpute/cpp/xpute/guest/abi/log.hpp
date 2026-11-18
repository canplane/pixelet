// cpp/xpute/guest/abi/log.hpp
//
// GENERATED from wit/log (xpute-link-gen wit.rs)

#pragma once

#include <cstdint>
#include <optional>
#include <string_view>

#include "xpute/guest/link.hpp"
#include "xpute/kit/status/error.hpp"
#include "xpute/link/cabi.hpp"
#include "xpute/link/link.hpp"

namespace xpute::abi::log {

namespace _rt {
using ::xpute::Result;
using ::xpute::guest::call;
using ::xpute::link::Handle;
using ::xpute::link::Lower;
using ::xpute::link::Method;
using ::xpute::link::Packet;
using ::xpute::link::answer;
} // namespace _rt

// The interface's handle, named by `xpute:log/log` (frame.json5 `NAMED`).
inline constexpr _rt::Handle HANDLE = 0x73da26;

// `log`
inline constexpr _rt::Method LOG = 0;

enum class Level : std::uint32_t {
    trace = 0,
    debug = 1,
    info = 2,
    warn = 3,
    error = 4,
    critical = 5,
};

// The member `val` is, or none for a number no member has.
constexpr std::optional<Level> of_Level(std::uint32_t val) noexcept {
    switch (val) {
    case 0: return Level::trace;
    case 1: return Level::debug;
    case 2: return Level::info;
    case 3: return Level::warn;
    case 4: return Level::error;
    case 5: return Level::critical;
    }
    return std::nullopt;
}

inline _rt::Result<void> log(::xpute::abi::log::Level level, std::string_view context, std::string_view msg) noexcept {
    auto [result_, data_] = _rt::call(HANDLE, LOG, _rt::Packet{20, [&](_rt::Lower &w_) { w_.put<std::uint8_t>(0, static_cast<std::uint8_t>(level)); w_.str(4, context); w_.str(12, msg); }});
    return _rt::answer(result_);
}

} // namespace xpute::abi::log
