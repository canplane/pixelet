// cpp/xpute/gpu/guest/abi/surface.hpp
//
// GENERATED from gpu/wit (xpute-link-gen wit.rs)

#pragma once

#include <cstdint>
#include <memory>
#include <optional>
#include <span>
#include <utility>

#include "xpute/kit/status/error.hpp"
#include "xpute/link/cabi.hpp"
#include "xpute/link/link.hpp"

namespace xpute::abi::gpu::surface {

namespace _rt {
using ::xpute::Errno;
using ::xpute::Result;
using ::xpute::link::Handle;
using ::xpute::link::Lift;
using ::xpute::link::Method;
using ::xpute::link::Object;
using ::xpute::link::Reply;
using ::xpute::link::Tag;
} // namespace _rt

// The interface's handle, named by `xpute:gpu/surface` (frame.json5 `NAMED`).
inline constexpr _rt::Handle HANDLE = 0x6e5265;

// `[method]surface.resize`
inline constexpr _rt::Method SURFACE_RESIZE = 0;

class GuestSurface {
  public:
    virtual ~GuestSurface() = default;
    virtual _rt::Result<void> resize(std::uint32_t width, std::uint32_t height, double dpr) = 0;
};

class SurfaceObject_ final : public _rt::Object {
  public:
    explicit SurfaceObject_(std::unique_ptr<GuestSurface> implementation) noexcept : implementation_(std::move(implementation)) {}

    _rt::Reply call(_rt::Method m_, [[maybe_unused]] _rt::Tag tag_, [[maybe_unused]] std::optional<std::span<const std::uint8_t>> args_) override {
        switch (m_) {
        case SURFACE_RESIZE: {
            if (!args_) return _rt::Reply{static_cast<std::int32_t>(_rt::Errno::ebadmsg)};
            _rt::Lift r_(*args_);
            std::uint32_t width = r_.get<std::uint32_t>(0);
            std::uint32_t height = r_.get<std::uint32_t>(4);
            double dpr = r_.get<double>(8);
            if (!r_.finish()) return _rt::Reply{static_cast<std::int32_t>(_rt::Errno::ebadmsg)};
            _rt::Result<void> x_ = implementation_->resize(width, height, dpr);
            if (!x_) return _rt::Reply{static_cast<std::int32_t>(x_.error().errnum)};
            return _rt::Reply{0};
        }
        default:
            return _rt::Reply{static_cast<std::int32_t>(_rt::Errno::enosys)};
        }
    }

  private:
    std::unique_ptr<GuestSurface> implementation_;
};

// The object over `implementation`, for an opener or a function to answer.
inline std::unique_ptr<_rt::Object> surface(std::unique_ptr<GuestSurface> implementation) {
    return std::make_unique<SurfaceObject_>(std::move(implementation));
}

} // namespace xpute::abi::gpu::surface
