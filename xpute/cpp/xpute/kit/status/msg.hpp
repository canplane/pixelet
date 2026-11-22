// cpp/xpute/kit/status/msg.hpp

// A message (`kit/spec/msg.json5`): what a person reads of an error, a bug or
// a log record, bounded text, which its maker cuts.

#pragma once

#include <algorithm>
#include <cstddef>
#include <string_view>

#include "msg.spec.hpp"

namespace xpute {

// `msg` as far as the cap holds it, cut at a character boundary. It is a part
// of `msg` itself, so cutting one allocates nothing, a bug's static message
// among them.
constexpr std::string_view cut_msg(std::string_view msg) noexcept {
    std::size_t len = std::min<std::size_t>(msg.size(), MSG_CAP);
    while (len < msg.size() && len != 0 && (static_cast<unsigned char>(msg[len]) & 0xc0) == 0x80) len--;
    return msg.substr(0, len);
}

} // namespace xpute
