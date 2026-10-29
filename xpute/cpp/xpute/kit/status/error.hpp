// cpp/xpute/kit/status/error.hpp

// An error is its errno, its class and where it was made. A broken invariant
// is fatal (bug.hpp); a fault from outside is returned as a Result.

#pragma once

#include <cstdint>
#include <optional>
#include <source_location>
#include <utility>

#include "errno.spec.hpp"

namespace xpute {

enum class ErrorClass : std::uint8_t {
    invariant,
    fault,
    marshal,
    network,
};

struct Error {
    Errno errnum;
    ErrorClass cls;
    std::source_location at;

    constexpr bool is_fault() const noexcept { return cls != ErrorClass::invariant; }
};

constexpr Error make_error(ErrorClass cls, Errno errnum, std::source_location at = std::source_location::current()) noexcept {
    return Error{errnum, cls, at};
}

constexpr Error marshal_error(Errno errnum, std::source_location at = std::source_location::current()) noexcept {
    return Error{errnum, ErrorClass::marshal, at};
}

template <class T> class [[nodiscard]] Result {
  public:
    Result(T val) noexcept : val_(std::move(val)) {}
    Result(Error err) noexcept : err_(err) {}

    bool ok() const noexcept { return val_.has_value(); }
    explicit operator bool() const noexcept { return ok(); }

    // Asking where there is none is a bug.
    T &value() noexcept { return *val_; }
    const T &value() const noexcept { return *val_; }
    T *operator->() noexcept { return &*val_; }
    T &operator*() noexcept { return *val_; }

    const Error &error() const noexcept { return err_; }

  private:
    std::optional<T> val_;
    Error err_{Errno::ok, ErrorClass::invariant, {}};
};

template <> class [[nodiscard]] Result<void> {
  public:
    Result() noexcept = default;
    Result(Error err) noexcept : err_(err), failed_(true) {}

    bool ok() const noexcept { return !failed_; }
    explicit operator bool() const noexcept { return ok(); }
    const Error &error() const noexcept { return err_; }

  private:
    Error err_{Errno::ok, ErrorClass::invariant, {}};
    bool failed_ = false;
};

} // namespace xpute
