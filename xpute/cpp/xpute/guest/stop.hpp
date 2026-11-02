// cpp/xpute/guest/stop.hpp

// How a guest stops: a broken invariant (`bug`), published to the host as the
// turn's one record (what `stop::install` is given, the guest's `Sys::abort`)
// and told to the hook of a form whose process dies with the guest. C++ has
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

// Where a form whose process dies with the guest says why: no host is left to
// run the record.
using Hook = void (*)(const Stop &);

// Installs the bug reporter, once at boot: `abort` publishes the record,
// `hook` is the form's, none where the host outlives the guest (a page).
void install(Report abort, Hook hook = nullptr) noexcept;

} // namespace xpute::stop
