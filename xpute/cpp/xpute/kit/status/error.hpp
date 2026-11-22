// cpp/xpute/kit/status/error.hpp

// An error is its errno, the place in the source where it was made, its file
// and its line, and what a person reads of it beyond the errno, where its
// maker says more; what happened is the errno's kind. Across the link it goes
// whole, so the error its caller holds is the one that was made, with the
// place and the message of its maker's side. A fault from outside the
// program is returned as a `Result`; a broken invariant is fatal and goes
// through `bug.hpp`.

#pragma once

#include <cstdint>
#include <cstring>
#include <memory>
#include <optional>
#include <source_location>
#include <string_view>
#include <utility>

#include "errno.spec.hpp"
#include "msg.hpp"

namespace xpute {

struct Error {
    Errno errnum;
    // The file of the place it was made: this program's own source, or what
    // came across the link with an error another side made.
    std::string_view file;
    std::uint32_t line;
    // What a person reads beyond the errno, empty where the maker says
    // nothing more; a message past the cap is cut as it is made (`cut_msg`).
    std::string_view msg;
    // The bytes of `file` and `msg` that are no static string's, made once
    // and shared by an error's copies, as an error is passed on by value;
    // null where both are static.
    std::shared_ptr<const char[]> buf;

    constexpr Kind kind() const noexcept { return kind_Errno(errnum); }
};

// The error `errnum` with the message `msg`, which came across the link from
// the side that made it at `line` of `file`.
inline Error make_error(Errno errnum, std::string_view file, std::uint32_t line, std::string_view msg) noexcept {
    file = cut_msg(file);
    msg = cut_msg(msg);
    std::shared_ptr<char[]> buf = std::make_shared<char[]>(file.size() + msg.size());
    std::memcpy(buf.get(), file.data(), file.size());
    std::memcpy(buf.get() + file.size(), msg.data(), msg.size());
    return Error{errnum, {buf.get(), file.size()}, line, {buf.get() + file.size(), msg.size()}, std::move(buf)};
}

// The error `errnum` with the message `msg`, made here.
inline Error make_error(Errno errnum, std::string_view msg = {},
                        std::source_location at = std::source_location::current()) noexcept {
    std::string_view file = cut_msg(at.file_name());
    msg = cut_msg(msg);
    if (msg.empty()) return Error{errnum, file, at.line(), {}, {}};
    std::shared_ptr<char[]> buf = std::make_shared<char[]>(msg.size());
    std::memcpy(buf.get(), msg.data(), msg.size());
    return Error{errnum, file, at.line(), {buf.get(), msg.size()}, std::move(buf)};
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
    Error err_{Errno::ok, {}, 0, {}, {}};
};

template <> class [[nodiscard]] Result<void> {
  public:
    Result() noexcept = default;
    Result(Error err) noexcept : err_(std::move(err)), failed_(true) {}

    bool ok() const noexcept { return !failed_; }
    explicit operator bool() const noexcept { return ok(); }
    const Error &error() const noexcept { return err_; }

  private:
    Error err_{Errno::ok, {}, 0, {}, {}};
    bool failed_ = false;
};

} // namespace xpute
