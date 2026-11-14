// cpp/xpute/kit/status/error.hpp

// An error is its errno and the place in the source where it was made, and
// what happened is the errno's kind. A fault from outside the program is
// returned as a `Result`; a broken invariant is fatal and goes through
// `bug.hpp`.

#pragma once

#include <optional>
#include <source_location>
#include <utility>

#include "errno.spec.hpp"

namespace xpute {

struct Error {
    Errno errnum;
    std::source_location at;

    constexpr Kind kind() const noexcept { return kind_Errno(errnum); }
};

constexpr Error make_error(Errno errnum, std::source_location at = std::source_location::current()) noexcept {
    return Error{errnum, at};
}

template <class T> class [[nodiscard]] Result {
  public:
    Result(T val) noexcept : val_(std::move(val)) {}
    Result(Error err) noexcept : err_(err) {}

    bool ok() const noexcept { return val_.has_value(); }
    explicit operator bool() const noexcept { return ok(); }

    // Asking a failed result for its value is the caller's bug, and is not
    // checked.
    T &value() noexcept { return *val_; }
    const T &value() const noexcept { return *val_; }
    T *operator->() noexcept { return &*val_; }
    T &operator*() noexcept { return *val_; }

    const Error &error() const noexcept { return err_; }

  private:
    std::optional<T> val_;
    Error err_{Errno::ok, {}};
};

template <> class [[nodiscard]] Result<void> {
  public:
    Result() noexcept = default;
    Result(Error err) noexcept : err_(err), failed_(true) {}

    bool ok() const noexcept { return !failed_; }
    explicit operator bool() const noexcept { return ok(); }
    const Error &error() const noexcept { return err_; }

  private:
    Error err_{Errno::ok, {}};
    bool failed_ = false;
};

} // namespace xpute
