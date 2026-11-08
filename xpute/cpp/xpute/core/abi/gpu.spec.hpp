// cpp/xpute/core/abi/gpu.spec.hpp
//
// GENERATED from spec/abi/gpu.json
//
// The GPU device's protocol on major GPU, which tells the core the device's state and the
// surfaces the UI lends it. The core's own calls to the GPU are not commands: they are
// records in its GPU stream (gpu/api.json). GPU_READY tells the core that the device its
// machine asked for has answered. A surface is a canvas or a view that the UI hands over,
// and from then on only the core draws into it. The surface is named by the handle that
// GPU_SURFACE_OPEN answers with: the UI binds its canvas under that handle (@xpute/runtime
// `Link.bind`), and the stream's CONFIGURE and CURRENT records name the surface by it. What
// a surface shows is the program's to decide.

#pragma once

#include <array>
#include <cstdint>
#include <optional>
#include <span>
#include <string_view>

#include "xpute/kit/abi/cmd.hpp"
#include "xpute/kit/status/error.hpp"
#include "xpute/kit/wire/xtp/cursor.hpp"
#include "xpute/kit/wire/xtp/encode.hpp"
#include "number.hpp"

namespace xpute::abi::gpu {

// A command's major, the high byte of its number, by which the runtime routes the frame.
enum class Major : std::uint32_t {
    GPU = 0x02,
};

} // namespace xpute::abi::gpu

template <> struct xpute::Members<xpute::abi::gpu::Major> {
    static constexpr std::array<xpute::abi::gpu::Major, 1> ALL{xpute::abi::gpu::Major::GPU};
};

namespace xpute::abi::gpu {

enum class GpuMinor : std::uint32_t {
    READY = 0x01,
    SURFACE_OPEN = 0x02,
    SURFACE_RESIZE = 0x03,
    SURFACE_CLOSE = 0x04,
};

} // namespace xpute::abi::gpu

template <> struct xpute::Members<xpute::abi::gpu::GpuMinor> {
    static constexpr std::array<xpute::abi::gpu::GpuMinor, 4> ALL{xpute::abi::gpu::GpuMinor::READY, xpute::abi::gpu::GpuMinor::SURFACE_OPEN, xpute::abi::gpu::GpuMinor::SURFACE_RESIZE, xpute::abi::gpu::GpuMinor::SURFACE_CLOSE};
};

namespace xpute::abi::gpu {

// Every command, by the side that sends it.
enum class Command : std::uint32_t {

    // ---- gpu, export ----
    // The GPU the machine asked for has answered, for the first time or again after the device
    // was lost. The STATUS word of the stream's device block tells the core the device's state.
    GPU_READY = 0x0201,
    // Asks the core to open a surface. The core answers with the surface's handle, or with 0
    // when it opens none.
    GPU_SURFACE_OPEN = 0x0202,
    // The surface's layout box in device pixels, ignoring transforms, and the device pixel
    // ratio. The size of the buffer the surface is drawn into is the core's to choose.
    GPU_SURFACE_RESIZE = 0x0203,
    // The surface is going away, and the core lets go of what it drew and held on the device
    // for it.
    GPU_SURFACE_CLOSE = 0x0204,
};

} // namespace xpute::abi::gpu

template <> struct xpute::Members<xpute::abi::gpu::Command> {
    static constexpr std::array<xpute::abi::gpu::Command, 4> ALL{xpute::abi::gpu::Command::GPU_READY, xpute::abi::gpu::Command::GPU_SURFACE_OPEN, xpute::abi::gpu::Command::GPU_SURFACE_RESIZE, xpute::abi::gpu::Command::GPU_SURFACE_CLOSE};
};

namespace xpute::abi::gpu {

// Command::GPU_SURFACE_RESIZE's packet.
struct GpuSurfaceResize {
    std::uint32_t surface{};
    std::uint32_t width{};
    std::uint32_t height{};
    double device_pixel_ratio{};

    static Result<GpuSurfaceResize> read(std::span<const std::uint8_t> pkt) noexcept {
        Result<xtp::TreeReader> t = xtp::TreeReader::make(pkt);
        if (!t) return t.error();
        Result<xtp::BranchCursor> f = t->read_branch();
        if (!f) return f.error();
        GpuSurfaceResize out;
        {
            Result<xtp::Cursor> c = f->at(0);
            if (!c) return c.error();
            auto v = c->get_number();
            if (!v) return v.error();
            auto n = integer<std::uint32_t>(*v);
            if (!n) return n.error();
            out.surface = *n;
        }
        {
            Result<xtp::Cursor> c = f->at(1);
            if (!c) return c.error();
            auto v = c->get_number();
            if (!v) return v.error();
            auto n = integer<std::uint32_t>(*v);
            if (!n) return n.error();
            out.width = *n;
        }
        {
            Result<xtp::Cursor> c = f->at(2);
            if (!c) return c.error();
            auto v = c->get_number();
            if (!v) return v.error();
            auto n = integer<std::uint32_t>(*v);
            if (!n) return n.error();
            out.height = *n;
        }
        {
            Result<xtp::Cursor> c = f->at(3);
            if (!c) return c.error();
            auto v = c->get_number();
            if (!v) return v.error();
            auto n = finite(*v);
            if (!n) return n.error();
            out.device_pixel_ratio = *n;
        }
        return out;
    }
};

// Command::GPU_SURFACE_CLOSE's packet.
struct GpuSurfaceClose {
    std::uint32_t surface{};

    static Result<GpuSurfaceClose> read(std::span<const std::uint8_t> pkt) noexcept {
        Result<xtp::TreeReader> t = xtp::TreeReader::make(pkt);
        if (!t) return t.error();
        Result<xtp::BranchCursor> f = t->read_branch();
        if (!f) return f.error();
        GpuSurfaceClose out;
        {
            Result<xtp::Cursor> c = f->at(0);
            if (!c) return c.error();
            auto v = c->get_number();
            if (!v) return v.error();
            auto n = integer<std::uint32_t>(*v);
            if (!n) return n.error();
            out.surface = *n;
        }
        return out;
    }
};

} // namespace xpute::abi::gpu
