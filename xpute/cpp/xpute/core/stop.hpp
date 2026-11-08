// cpp/xpute/core/stop.hpp

// How a core stops. A broken invariant (`bug`) is posted to the runtime as
// the turn's only frame, by the reporter `stop::install` is given (the
// core's `Door::abort`). Where the core's process dies with it, the stop is
// also told to the form's hook. C++ has no panic of its own: what libc++
// aborts on is a bug (kit status/bug.cpp).

#pragma once

#include <cstdint>
#include <source_location>

#include "xpute/kit/status/bug.hpp"
#include "xpute/kit/status/errno.spec.hpp"

namespace xpute::stop {

// A stop holds numbers only; the hook does any formatting.
struct Stop {
    Errno errnum;
    const std::source_location &at;
    std::uint64_t a;
    std::uint64_t b;
};

// The hook through which a form whose process dies with the core says why
// the core stopped, since no runtime is left to read the record.
using Hook = void (*)(const Stop &);

// Installs the bug reporter, once at boot. `report` publishes the record;
// `hook` is the form's, and is none where the runtime outlives the core (a
// page).
void install(Report report, Hook hook = nullptr) noexcept;

} // namespace xpute::stop
