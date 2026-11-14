// cpp/xpute/guest/abi/fs/types.hpp
//
// GENERATED from wit/fs (xpute-link-gen wit.rs)

#pragma once

#include <cstdint>
#include <optional>

namespace xpute::abi::fs::types {

enum class ErrorCode : std::uint32_t {
    // No file is there.
    no_entry = 0,
    // Its backend failed to read or write it.
    io = 1,
    // Its backend keeps nothing written there.
    read_only = 2,
    // Its backend refused access to it.
    access = 3,
    // Its backend holds it in a form it cannot read as bytes.
    bad_message = 4,
    // The network did not reach its server.
    remote_unreachable = 5,
    // Its server holds no such object.
    no_object = 6,
};

// The member `val` is, or none for a number no member has.
constexpr std::optional<ErrorCode> of_ErrorCode(std::uint32_t val) noexcept {
    switch (val) {
    case 0: return ErrorCode::no_entry;
    case 1: return ErrorCode::io;
    case 2: return ErrorCode::read_only;
    case 3: return ErrorCode::access;
    case 4: return ErrorCode::bad_message;
    case 5: return ErrorCode::remote_unreachable;
    case 6: return ErrorCode::no_object;
    }
    return std::nullopt;
}

} // namespace xpute::abi::fs::types
