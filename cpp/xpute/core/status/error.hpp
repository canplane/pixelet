// cpp/xpute/core/status/error.hpp

// How the kit says a call failed, as xpute-core's status/error.rs does:
// an error is its errno, its class and where it was made, and nothing else.
// What it means to a reader is the errno's, where it was made is the place,
// and a sentence carried with it would say what those already say.
//
// The policy is the same: an invariant that did not hold is fatal (bug.hpp),
// and a fault from outside — a peer, the network, a packet that does not
// read — is returned and handled. Exceptions are off, so a failure is a
// value: a Result, which is the value or the error.
//
// The classes are Rust's tree — invariant and fault, and under fault the
// marshal and the network — held as one type with its class, so a handler
// that takes an error takes any of them.

#pragma once

#include <cstdint>
#include <optional>
#include <source_location>
#include <utility>

#include "errno.spec.hpp"

namespace xpute {

enum class ErrorClass : std::uint8_t {
    // An internal invariant violated: an impossible state, our bug.
    invariant,
    // An external fault: untrusted input, a peer, the network.
    fault,
    // A wire or payload that does not decode, at a marshal boundary.
    marshal,
    // A network, transport or protocol fault.
    network,
};

struct Error {
    Errno code;
    ErrorClass cls;
    std::source_location at;

    // A marshal or network error is a fault as well.
    constexpr bool is_fault() const noexcept { return cls != ErrorClass::invariant; }
};

// An error of `cls`, made where it is written.
constexpr Error make_error(ErrorClass cls, Errno code, std::source_location at = std::source_location::current()) noexcept {
    return Error{code, cls, at};
}

constexpr Error marshal_error(Errno code, std::source_location at = std::source_location::current()) noexcept {
    return Error{code, ErrorClass::marshal, at};
}

// A value, or the error that stopped it.
template <class T> class [[nodiscard]] Result {
  public:
    Result(T value) noexcept : value_(std::move(value)) {}
    Result(Error error) noexcept : error_(error) {}

    bool ok() const noexcept { return value_.has_value(); }
    explicit operator bool() const noexcept { return ok(); }

    // The value; asking for it where there is none is a bug.
    T &value() noexcept { return *value_; }
    const T &value() const noexcept { return *value_; }
    T *operator->() noexcept { return &*value_; }
    T &operator*() noexcept { return *value_; }

    const Error &error() const noexcept { return error_; }

  private:
    std::optional<T> value_;
    Error error_{Errno::ok, ErrorClass::invariant, {}};
};

// Nothing, or the error.
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
