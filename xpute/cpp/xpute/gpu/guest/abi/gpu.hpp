// cpp/xpute/gpu/guest/abi/gpu.hpp
//
// GENERATED from gpu/wit (xpute-link-gen wit.rs)

#pragma once

#include <cstdint>
#include <optional>
#include <utility>
#include <variant>

#include "xpute/guest/link.hpp"
#include "xpute/guest/wait.hpp"
#include "xpute/kit/status/error.hpp"
#include "xpute/link/cabi.hpp"
#include "xpute/link/link.hpp"
#include "xpute/link/wait.hpp"

namespace xpute::abi::gpu::gpu {

namespace _rt {
using ::xpute::Result;
using ::xpute::guest::call;
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
using ::xpute::link::span;
} // namespace _rt

// The interface's handle, named by `xpute:gpu/gpu` (frame.json5 `NAMED`).
inline constexpr _rt::Handle HANDLE = 0x79d455;

// `run`
inline constexpr _rt::Method RUN = 0;
// `get-device`
inline constexpr _rt::Method GET_DEVICE = 1;
// `[method]device.lost`
inline constexpr _rt::Method DEVICE_LOST = 0;
// `[method]device.ready`
inline constexpr _rt::Method DEVICE_READY = 1;
// `[method]device.settled`
inline constexpr _rt::Method DEVICE_SETTLED = 2;

enum class ErrorCode : std::uint32_t {
    // The host has no device to give.
    no_device = 0,
};

// The member `val` is, or none for a number no member has.
constexpr std::optional<ErrorCode> of_ErrorCode(std::uint32_t val) noexcept {
    switch (val) {
    case 0: return ErrorCode::no_device;
    }
    return std::nullopt;
}

template <_rt::Lend L> inline _rt::Task<std::pair<L, _rt::Result<void>>> run(L recs) noexcept {
    _rt::Lent recs_lent_ = _rt::span(recs);
    auto sent_ = _rt::wait(HANDLE, RUN, _rt::Packet{8, [&](_rt::Lower &w_) { recs_lent_.store(w_, 0); }}, std::move(recs));
    auto [result_, data_, lent_] = co_await std::move(sent_);
    co_return {std::move(lent_), _rt::answer(result_)};
}

inline _rt::Result<std::uint32_t> get_device() noexcept {
    auto [result_, data_] = _rt::call(HANDLE, GET_DEVICE, std::nullopt);
    return _rt::answer(result_, data_, [&](_rt::Lift &r_) { return r_.get<std::uint32_t>(0); });
}

template <_rt::Lend L> inline _rt::Task<std::pair<L, _rt::Result<void>>> device_lost(_rt::Handle device_, L block) noexcept {
    _rt::Lent block_lent_ = _rt::span(block);
    auto sent_ = _rt::wait(device_, DEVICE_LOST, _rt::Packet{8, [&](_rt::Lower &w_) { block_lent_.store(w_, 0); }}, std::move(block));
    auto [result_, data_, lent_] = co_await std::move(sent_);
    co_return {std::move(lent_), _rt::answer(result_)};
}

inline _rt::Task<_rt::Result<std::variant<std::monostate, ::xpute::abi::gpu::gpu::ErrorCode>>> device_ready(_rt::Handle device_) noexcept {
    auto sent_ = _rt::wait(device_, DEVICE_READY, std::nullopt, std::monostate());
    auto [result_, data_, lent_] = co_await std::move(sent_);
    co_return _rt::answer(result_, data_, [&](_rt::Lift &r_) { return (r_.case_(0, 2) == 0 ? std::variant<std::monostate, ::xpute::abi::gpu::gpu::ErrorCode>(std::in_place_index<0>, std::monostate()) : std::variant<std::monostate, ::xpute::abi::gpu::gpu::ErrorCode>(std::in_place_index<1>, static_cast<::xpute::abi::gpu::gpu::ErrorCode>(r_.case_(1, 1)))); });
}

inline _rt::Task<_rt::Result<void>> device_settled(_rt::Handle device_) noexcept {
    auto sent_ = _rt::wait(device_, DEVICE_SETTLED, std::nullopt, std::monostate());
    auto [result_, data_, lent_] = co_await std::move(sent_);
    co_return _rt::answer(result_);
}

} // namespace xpute::abi::gpu::gpu
