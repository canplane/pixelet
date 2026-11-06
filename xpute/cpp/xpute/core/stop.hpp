// cpp/xpute/core/stop.hpp

// How a core stops: a broken invariant (`bug`), posted to the runtime as the
// turn's one frame (what `stop::install` is given, the core's `Door::abort`)
// and told to the hook of a form whose process dies with the core. C++ has
// no panic of its own: what libc++ aborts on is a bug (kit status/bug.cpp).

#pragma once

#include <cstdint>
#include <source_location>

#include "xpute/kit/status/bug.hpp"
#include "xpute/kit/status/errno.spec.hpp"

namespace xpute::stop {

// Numbers only: formatting is the hook's.
struct Stop {
    Errno errnum;
    const std::source_location &at;
    std::uint64_t a;
    std::uint64_t b;
};

// Where a form whose process dies with the core says why: no runtime is left to
// run the record.
using Hook = void (*)(const Stop &);

// Installs the bug reporter, once at boot: `report` publishes the record,
// `hook` is the form's, none where the runtime outlives the core (a page).
void install(Report report, Hook hook = nullptr) noexcept;

} // namespace xpute::stop
