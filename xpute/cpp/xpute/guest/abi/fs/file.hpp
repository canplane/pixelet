// cpp/xpute/guest/abi/fs/file.hpp
//
// GENERATED from wit/fs (xpute-link-gen wit.rs)

#pragma once

#include <cstdint>
#include <optional>
#include <utility>
#include <variant>

#include "types.hpp"
#include "xpute/guest/wait.hpp"
#include "xpute/kit/status/error.hpp"
#include "xpute/link/cabi.hpp"
#include "xpute/link/link.hpp"
#include "xpute/link/wait.hpp"

namespace xpute::abi::fs::file {

namespace _rt {
using ::xpute::Result;
using ::xpute::guest::wait;
using ::xpute::link::Handle;
using ::xpute::link::Lend;
using ::xpute::link::Lent;
using ::xpute::link::Lift;
using ::xpute::link::Lower;
using ::xpute::link::Method;
using ::xpute::link::Packet;
using ::xpute::link::Task;
using ::xpute::link::answer;
using ::xpute::link::lent;
} // namespace _rt

// The interface's handle, named by `xpute:fs/file` (frame.json5 `NAMED`).
inline constexpr _rt::Handle HANDLE = 0x5b6743;

// `[method]file.size`
inline constexpr _rt::Method FILE_SIZE = 0;
// `[method]file.read`
inline constexpr _rt::Method FILE_READ = 1;
// `[method]file.write`
inline constexpr _rt::Method FILE_WRITE = 2;
// `[method]file.set-aside`
inline constexpr _rt::Method FILE_SET_ASIDE = 3;
// `[method]file.close`
inline constexpr _rt::Method FILE_CLOSE = 4;

inline _rt::Task<_rt::Result<std::variant<std::uint32_t, ::xpute::abi::fs::types::ErrorCode>>> file_size(_rt::Handle file_) noexcept {
    auto sent_ = _rt::wait(file_, FILE_SIZE, std::nullopt, std::monostate());
    auto [result_, data_, lent_] = co_await std::move(sent_);
    co_return _rt::answer(result_, data_, [&](_rt::Lift &r_) { return (r_.case_(0, 2) == 0 ? std::variant<std::uint32_t, ::xpute::abi::fs::types::ErrorCode>(std::in_place_index<0>, r_.get<std::uint32_t>(4)) : std::variant<std::uint32_t, ::xpute::abi::fs::types::ErrorCode>(std::in_place_index<1>, static_cast<::xpute::abi::fs::types::ErrorCode>(r_.case_(4, 7)))); });
}

template <_rt::Lend L> inline _rt::Task<std::pair<L, _rt::Result<std::variant<std::uint32_t, ::xpute::abi::fs::types::ErrorCode>>>> file_read(_rt::Handle file_, L buf) noexcept {
    _rt::Lent buf_lent_ = _rt::lent(buf);
    auto sent_ = _rt::wait(file_, FILE_READ, _rt::Packet{16, [&](_rt::Lower &w_) { buf_lent_.store(w_, 0); }}, std::move(buf));
    auto [result_, data_, lent_] = co_await std::move(sent_);
    co_return {std::move(lent_), _rt::answer(result_, data_, [&](_rt::Lift &r_) { return (r_.case_(0, 2) == 0 ? std::variant<std::uint32_t, ::xpute::abi::fs::types::ErrorCode>(std::in_place_index<0>, r_.get<std::uint32_t>(4)) : std::variant<std::uint32_t, ::xpute::abi::fs::types::ErrorCode>(std::in_place_index<1>, static_cast<::xpute::abi::fs::types::ErrorCode>(r_.case_(4, 7)))); })};
}

template <_rt::Lend L> inline _rt::Task<std::pair<L, _rt::Result<void>>> file_write(_rt::Handle file_, L buf) noexcept {
    _rt::Lent buf_lent_ = _rt::lent(buf);
    auto sent_ = _rt::wait(file_, FILE_WRITE, _rt::Packet{16, [&](_rt::Lower &w_) { buf_lent_.store(w_, 0); }}, std::move(buf));
    auto [result_, data_, lent_] = co_await std::move(sent_);
    co_return {std::move(lent_), _rt::answer(result_)};
}

inline _rt::Task<_rt::Result<std::variant<std::monostate, ::xpute::abi::fs::types::ErrorCode>>> file_set_aside(_rt::Handle file_) noexcept {
    auto sent_ = _rt::wait(file_, FILE_SET_ASIDE, std::nullopt, std::monostate());
    auto [result_, data_, lent_] = co_await std::move(sent_);
    co_return _rt::answer(result_, data_, [&](_rt::Lift &r_) { return (r_.case_(0, 2) == 0 ? std::variant<std::monostate, ::xpute::abi::fs::types::ErrorCode>(std::in_place_index<0>, std::monostate()) : std::variant<std::monostate, ::xpute::abi::fs::types::ErrorCode>(std::in_place_index<1>, static_cast<::xpute::abi::fs::types::ErrorCode>(r_.case_(1, 7)))); });
}

inline _rt::Task<_rt::Result<std::variant<std::monostate, ::xpute::abi::fs::types::ErrorCode>>> file_close(_rt::Handle file_) noexcept {
    auto sent_ = _rt::wait(file_, FILE_CLOSE, std::nullopt, std::monostate());
    auto [result_, data_, lent_] = co_await std::move(sent_);
    co_return _rt::answer(result_, data_, [&](_rt::Lift &r_) { return (r_.case_(0, 2) == 0 ? std::variant<std::monostate, ::xpute::abi::fs::types::ErrorCode>(std::in_place_index<0>, std::monostate()) : std::variant<std::monostate, ::xpute::abi::fs::types::ErrorCode>(std::in_place_index<1>, static_cast<::xpute::abi::fs::types::ErrorCode>(r_.case_(1, 7)))); });
}

} // namespace xpute::abi::fs::file
