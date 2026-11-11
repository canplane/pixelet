// cpp/xpute/core/entry.hpp

// This is the one function a core runs inside, `run`, and what the runtime
// hands it there, as xpute-core entry.rs is: a module's exported `run` and a
// native library's turn each call it with the program's entry.

#pragma once

#include "xpute/core/clock.hpp"
#include "xpute/core/host.hpp"

namespace xpute {

// What every core is handed, in whichever form it is built: how it reaches
// its runtime (host.hpp) and the runtime's clock (clock.hpp).
struct BootInfo {
    Host host;
    Clock now;
};

using Boot = BootInfo (*)();

// An entry runs one turn that may last `deadline_ms`, and answers in how many
// ms the core wants the next: 0 for as soon as the scheduling policy allows,
// NO_WAKE (sched/wake.hpp) for none.
using Entry = double (*)(double deadline_ms, Boot boot);

namespace entry {

// A form's exported entry calls this on every turn. On the first turn it
// installs the host and the clock from `boot`. It then runs `entry`.
double run(double deadline_ms, Boot boot, Entry entry) noexcept;

} // namespace entry

} // namespace xpute
