// cpp/xpute/core/entry.cpp

#include "entry.hpp"

namespace xpute::entry {

namespace {
bool booted = false;
}

double run(double deadline_ms, Boot boot, Entry entry) noexcept {
    if (!booted) {
        booted = true;
        BootInfo b = boot();
        set_clock(b.now);
        host::set(b.host);
    }
    return entry(deadline_ms, boot);
}

} // namespace xpute::entry
