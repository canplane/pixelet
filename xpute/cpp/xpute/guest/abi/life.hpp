// cpp/xpute/guest/abi/life.hpp
//
// GENERATED from wit/life (xpute-link-gen wit.rs)

#pragma once

#include <cstdint>
#include <memory>
#include <optional>
#include <span>
#include <utility>

#include "xpute/guest/link.hpp"
#include "xpute/kit/status/error.hpp"
#include "xpute/link/link.hpp"

namespace xpute::abi::life {

namespace _rt {
using ::xpute::Errno;
using ::xpute::Result;
using ::xpute::guest::mount_named;
using ::xpute::link::Handle;
using ::xpute::link::Method;
using ::xpute::link::Object;
using ::xpute::link::Reply;
using ::xpute::link::Tag;
} // namespace _rt

// The interface's handle, named by `xpute:life/life` (frame.json `NAMED`).
inline constexpr _rt::Handle HANDLE = 0x46edb9;

// `hidden`
inline constexpr _rt::Method HIDDEN = 0;
// `shown`
inline constexpr _rt::Method SHOWN = 1;

class Guest {
  public:
    virtual ~Guest() = default;
    virtual _rt::Result<void> hidden() = 0;
    virtual _rt::Result<void> shown() = 0;
};

class Object_ final : public _rt::Object {
  public:
    explicit Object_(std::unique_ptr<Guest> implementation) noexcept : implementation_(std::move(implementation)) {}

    _rt::Reply call(_rt::Method m_, [[maybe_unused]] _rt::Tag tag_, [[maybe_unused]] std::optional<std::span<const std::uint8_t>> args_) override {
        switch (m_) {
        case HIDDEN: {
            _rt::Result<void> x_ = implementation_->hidden();
            if (!x_) return _rt::Reply{static_cast<std::int32_t>(x_.error().errnum)};
            return _rt::Reply{0};
        }
        case SHOWN: {
            _rt::Result<void> x_ = implementation_->shown();
            if (!x_) return _rt::Reply{static_cast<std::int32_t>(x_.error().errnum)};
            return _rt::Reply{0};
        }
        default:
            return _rt::Reply{static_cast<std::int32_t>(_rt::Errno::enosys)};
        }
    }

  private:
    std::unique_ptr<Guest> implementation_;
};

// Serves `implementation` under the interface's handle.
inline void mount(std::unique_ptr<Guest> implementation) {
    _rt::mount_named(HANDLE, std::make_unique<Object_>(std::move(implementation)));
}

} // namespace xpute::abi::life
