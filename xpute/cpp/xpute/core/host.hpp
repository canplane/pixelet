// cpp/xpute/core/host.hpp

// The core reaches its runtime through three functions, which the runtime
// hands it at boot. Each names a frame in the core's memory by its offset: its
// words, then its packet (io/frame.hpp). A page's runtime gives them as the
// module's imports (wasm.hpp), a native one as functions of its process.
//
// - `receive` writes the next frame the runtime sends the core into the
//   `cap` bytes at `at`, and answers 1, or 0 when none waits.
// - `send` takes the frame at `at`, and the `named_len` bytes at `named_at`
//   that the frame names, before it returns: both are the core's again at
//   once.
// - `call` takes the frame at `at` and answers it before it returns, with a
//   value or a negative errno. It never calls into the core.
//
// Reaching the runtime before it has handed them over is a bug.

#pragma once

#include <cstdint>

#include "xpute/kit/status/errno.spec.hpp"
#include "xpute/kit/status/error.hpp"

namespace xpute {

struct Host {
    std::int32_t (*receive)(std::uint32_t at, std::uint32_t cap);
    void (*send)(std::uint32_t at, std::uint32_t named_at, std::uint32_t named_len);
    std::int32_t (*call)(std::uint32_t at);
};

namespace host {

void set(const Host &host) noexcept;

std::int32_t receive(std::uint32_t at, std::uint32_t cap) noexcept;

void send(std::uint32_t at, std::uint32_t named_at, std::uint32_t named_len) noexcept;

std::int32_t call(std::uint32_t at) noexcept;

} // namespace host

// Reads a runtime's answer as a count, or as the errno it refused with. A
// negative code that names no errno means the runtime broke the protocol,
// and reads as EPROTO.
inline Result<std::uint32_t> answer_of(std::int32_t res) noexcept {
    if (res < 0) return make_error(ErrorClass::fault, of_Errno(res).value_or(Errno::eproto));
    return static_cast<std::uint32_t>(res);
}

} // namespace xpute
