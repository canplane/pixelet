// cpp/xpute/core/call.hpp

// The runtime's synchronous `call`. The core writes a call's frame into the
// core memory and passes the frame's offset; the runtime answers before
// `call` returns, with a value or a negative errno. Until a runtime installs
// its `call`, every call answers ENOSYS, which native tests run with.

#pragma once

#include <cstdint>

#include "xpute/kit/status/errno.spec.hpp"
#include "xpute/kit/status/error.hpp"

namespace xpute {

using Call = std::int32_t (*)(std::uint32_t at);

void set_call(Call call) noexcept;

std::int32_t call(std::uint32_t at) noexcept;

// Reads a runtime's answer as a count, or as the errno it refused with. A
// negative code that names no errno means the runtime broke the protocol,
// and reads as EPROTO.
inline Result<std::uint32_t> answer_of(std::int32_t res) noexcept {
    if (res < 0) return make_error(ErrorClass::fault, errno_of(res).value_or(Errno::eproto));
    return static_cast<std::uint32_t>(res);
}

} // namespace xpute
