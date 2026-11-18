// cpp/xpute/kit/status/error.hpp

// An error is its errno, the place in the source where it was made, and what
// a person reads of it beyond the errno, where its maker says more; what
// happened is the errno's kind, and across the link it is its errno alone. A
// fault from outside the program is returned as a `Result`; a broken
// invariant is fatal and goes through `bug.hpp`.

#pragma once

#include <cstring>
#include <memory>
#include <optional>
#include <source_location>
#include <string_view>
#include <utility>

#include "errno.spec.hpp"

namespace xpute {

struct Error {
    Errno errnum;
    std::source_location at;
    // What a person reads beyond the errno, NUL-terminated, made once and
    // never grown, and shared by an error's copies, as an error is passed on
    // by value; null where the maker says nothing more.
    std::shared_ptr<const char[]> msg;

    constexpr Kind kind() const noexcept { return kind_Errno(errnum); }
};

inline Error make_error(Errno errnum, std::string_view msg = {},
                        std::source_location at = std::source_location::current()) noexcept {
    std::shared_ptr<char[]> m;
    if (!msg.empty()) {
        m = std::make_shared<char[]>(msg.size() + 1);
        std::memcpy(m.get(), msg.data(), msg.size());
    }
    return Error{errnum, at, std::move(m)};
}

template <class T> class [[nodiscard]] Result {
  public:
    Result(T val) noexcept : val_(std::move(val)) {}
    Result(Error err) noexcept : err_(std::move(err)) {}

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
    Error err_{Errno::ok, {}, {}};
};

template <> class [[nodiscard]] Result<void> {
  public:
    Result() noexcept = default;
    Result(Error err) noexcept : err_(std::move(err)), failed_(true) {}

    bool ok() const noexcept { return !failed_; }
    explicit operator bool() const noexcept { return ok(); }
    const Error &error() const noexcept { return err_; }

  private:
    Error err_{Errno::ok, {}, {}};
    bool failed_ = false;
};

} // namespace xpute
