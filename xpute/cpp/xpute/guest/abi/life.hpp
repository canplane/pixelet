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
#include "xpute/link/cabi.hpp"
#include "xpute/link/link.hpp"

namespace xpute::abi::life {

namespace _rt {
using ::xpute::Errno;
using ::xpute::Result;
using ::xpute::guest::mount_named;
using ::xpute::link::Handle;
using ::xpute::link::Lift;
using ::xpute::link::Lower;
using ::xpute::link::Method;
using ::xpute::link::Object;
using ::xpute::link::Tag;
} // namespace _rt

// The interface's handle, named by `xpute:life/life` (frame.json5 `NAMED`).
inline constexpr _rt::Handle HANDLE = 0x46edb9;

// `hidden`
inline constexpr _rt::Method HIDDEN = 0;
// `shown`
inline constexpr _rt::Method SHOWN = 1;
// `attached`
inline constexpr _rt::Method ATTACHED = 2;

class Guest {
  public:
    virtual ~Guest() = default;
    virtual _rt::Result<void> hidden() = 0;
    virtual _rt::Result<void> shown() = 0;
    virtual _rt::Result<void> attached() = 0;
};

class Object_ final : public _rt::Object {
  public:
    explicit Object_(std::unique_ptr<Guest> implementation) noexcept : implementation_(std::move(implementation)) {}

    std::int32_t call(_rt::Method m_, [[maybe_unused]] _rt::Tag tag_, [[maybe_unused]] _rt::Lift args_, [[maybe_unused]] _rt::Lower &ret_) override {
        switch (m_) {
        case HIDDEN: {
            _rt::Result<void> x_ = implementation_->hidden();
            if (!x_) return static_cast<std::int32_t>(x_.error().errnum);
            return 0;
        }
        case SHOWN: {
            _rt::Result<void> x_ = implementation_->shown();
            if (!x_) return static_cast<std::int32_t>(x_.error().errnum);
            return 0;
        }
        case ATTACHED: {
            _rt::Result<void> x_ = implementation_->attached();
            if (!x_) return static_cast<std::int32_t>(x_.error().errnum);
            return 0;
        }
        default:
            return static_cast<std::int32_t>(_rt::Errno::enosys);
        }
    }

  private:
    std::unique_ptr<Guest> implementation_;
};

// Mounts `implementation` under the interface's handle.
inline void mount(std::unique_ptr<Guest> implementation) {
    _rt::mount_named(HANDLE, std::make_unique<Object_>(std::move(implementation)));
}

} // namespace xpute::abi::life
