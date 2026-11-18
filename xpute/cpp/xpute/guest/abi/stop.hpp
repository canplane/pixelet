// cpp/xpute/guest/abi/stop.hpp
//
// GENERATED from wit/stop (xpute-link-gen wit.rs)

#pragma once

#include <cstdint>
#include <optional>
#include <string_view>

#include "xpute/guest/link.hpp"
#include "xpute/kit/status/error.hpp"
#include "xpute/link/cabi.hpp"
#include "xpute/link/link.hpp"

namespace xpute::abi::stop {

namespace _rt {
using ::xpute::Result;
using ::xpute::guest::call;
using ::xpute::link::Handle;
using ::xpute::link::Lower;
using ::xpute::link::Method;
using ::xpute::link::Packet;
using ::xpute::link::answer;
} // namespace _rt

// The interface's handle, named by `xpute:stop/stop` (frame.json5 `NAMED`).
inline constexpr _rt::Handle HANDLE = 0x4eccaa;

// `stopped`
inline constexpr _rt::Method STOPPED = 0;

inline _rt::Result<void> stopped(std::int32_t errnum, std::string_view file, std::uint32_t line, std::uint64_t n1, std::uint64_t n2, std::optional<std::string_view> msg) noexcept {
    auto [result_, data_] = _rt::call(HANDLE, STOPPED, _rt::Packet{48, [&](_rt::Lower &w_) { w_.put<std::int32_t>(0, errnum); w_.str(4, file); w_.put<std::uint32_t>(12, line); w_.put<std::uint64_t>(16, n1); w_.put<std::uint64_t>(24, n2); if (msg) { w_.put<std::uint8_t>(32, 1); w_.str(36, (*msg)); } }});
    return _rt::answer(result_);
}

} // namespace xpute::abi::stop
