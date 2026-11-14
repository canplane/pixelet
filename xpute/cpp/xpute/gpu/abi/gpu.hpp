// cpp/xpute/gpu/abi/gpu.hpp
//
// GENERATED from gpu/wit (xpute-link-gen wit.rs)

#pragma once

#include <cstdint>
#include <optional>

namespace xpute::abi::gpu::gpu {

// The interface's handle, named by `xpute:gpu/gpu` (frame.json `NAMED`).
inline constexpr std::uint32_t HANDLE = 0x79d455;

// `run`
inline constexpr std::uint32_t RUN = 0;
// `[method]device.lost`
inline constexpr std::uint32_t DEVICE_LOST = 0;
// `[method]device.ready`
inline constexpr std::uint32_t DEVICE_READY = 1;
// `[method]device.settled`
inline constexpr std::uint32_t DEVICE_SETTLED = 2;

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

} // namespace xpute::abi::gpu::gpu
