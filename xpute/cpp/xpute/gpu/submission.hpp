// cpp/xpute/gpu/submission.hpp
//
// What a core hears on the GPU's major, as the program's turn hands it a
// command it took from the door.

#pragma once

#include <cstdint>
#include <optional>
#include <span>
#include <variant>

#include "xpute/kit/status/error.hpp"
#include "abi.spec.hpp"

namespace xpute::gpu {

// The GPU the machine asked for has answered (GPU_READY), for the first time or
// again after a loss.
struct Ready {};

// A surface is lent to the GPU by the UI (abi/gpu.json). It is opened, and the
// program answers with its handle, or 0 for none; its box is resized; it is
// let go.
struct SurfaceOpen {};
using Surface = std::variant<SurfaceOpen, abi::gpu::GpuSurfaceResize, abi::gpu::GpuSurfaceClose>;

using Submission = std::variant<Ready, Surface>;

// What `cmd` says of the GPU, whose commands are the minors of the program's
// `major`, and none for a command that is not the GPU's.
inline Result<std::optional<Submission>> decode(std::uint32_t major, std::uint32_t cmd, std::span<const std::uint8_t> pkt) noexcept {
    using C = abi::gpu::Command;
    if (cmd >> 8 != major) return std::optional<Submission>{};
    const auto is = [cmd](C c) { return (cmd & 0xff) == static_cast<std::uint32_t>(c); };
    if (is(C::GPU_READY)) return std::optional<Submission>{Submission{Ready{}}};
    if (is(C::GPU_SURFACE_OPEN)) return std::optional<Submission>{Submission{Surface{SurfaceOpen{}}}};
    if (is(C::GPU_SURFACE_RESIZE)) {
        Result<abi::gpu::GpuSurfaceResize> r = abi::gpu::GpuSurfaceResize::read(pkt);
        if (!r) return r.error();
        return std::optional<Submission>{Submission{Surface{*r}}};
    }
    if (is(C::GPU_SURFACE_CLOSE)) {
        Result<abi::gpu::GpuSurfaceClose> r = abi::gpu::GpuSurfaceClose::read(pkt);
        if (!r) return r.error();
        return std::optional<Submission>{Submission{Surface{*r}}};
    }
    return std::optional<Submission>{};
}

} // namespace xpute::gpu
