// cpp/xpute/core/status/error.hpp

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
    Errno code;
    ErrorClass cls;
    std::source_location at;

    constexpr bool is_fault() const noexcept { return cls != ErrorClass::invariant; }
};

constexpr Error make_error(ErrorClass cls, Errno code, std::source_location at = std::source_location::current()) noexcept {
    return Error{code, cls, at};
}

constexpr Error marshal_error(Errno code, std::source_location at = std::source_location::current()) noexcept {
    return Error{code, ErrorClass::marshal, at};
}

template <class T> class [[nodiscard]] Result {
  public:
    Result(T value) noexcept : value_(std::move(value)) {}
    Result(Error error) noexcept : error_(error) {}

    bool ok() const noexcept { return value_.has_value(); }
    explicit operator bool() const noexcept { return ok(); }

    // Asking where there is none is a bug.
    T &value() noexcept { return *value_; }
    const T &value() const noexcept { return *value_; }
    T *operator->() noexcept { return &*value_; }
    T &operator*() noexcept { return *value_; }

    const Error &error() const noexcept { return error_; }

  private:
    std::optional<T> value_;
    Error error_{Errno::ok, ErrorClass::invariant, {}};
};

template <> class [[nodiscard]] Result<void> {
  public:
    Result() noexcept = default;
    Result(Error error) noexcept : error_(error), failed_(true) {}

    bool ok() const noexcept { return !failed_; }
    explicit operator bool() const noexcept { return ok(); }
    const Error &error() const noexcept { return error_; }

  private:
    Error error_{Errno::ok, ErrorClass::invariant, {}};
    bool failed_ = false;
};

} // namespace xpute
