// cpp/xpute/gpu/abi/vsync.hpp
//
// GENERATED from gpu/wit (xpute-link-gen wit.rs)

#pragma once

#include <optional>
#include <utility>
#include <variant>

#include "xpute/guest/wait.hpp"
#include "xpute/kit/status/error.hpp"
#include "xpute/link/link.hpp"
#include "xpute/link/wait.hpp"

namespace xpute::abi::gpu::vsync {

namespace _rt {
using ::xpute::Result;
using ::xpute::guest::wait;
using ::xpute::link::Handle;
using ::xpute::link::Method;
using ::xpute::link::Task;
using ::xpute::link::answer;
} // namespace _rt

// The interface's handle, named by `xpute:gpu/vsync` (frame.json `NAMED`).
inline constexpr _rt::Handle HANDLE = 0x42eda6;

// `next`
inline constexpr _rt::Method NEXT = 0;

inline _rt::Task<_rt::Result<void>> next() noexcept {
    auto sent_ = _rt::wait(HANDLE, NEXT, std::nullopt, std::monostate());
    auto [result_, data_, lent_] = co_await std::move(sent_);
    co_return _rt::answer(result_);
}

} // namespace xpute::abi::gpu::vsync
