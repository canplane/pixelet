// cpp/xpute/guest/stop.hpp

// The guest's report of its own failures to the host, by the stop the host
// implements (`wit/stop/stop.wit`), as `xpute-guest`'s `stop.rs` is a Rust
// guest's.

#pragma once

namespace xpute::guest::stop {

// Tells the host a bug in the guest's own code, with its message where it has
// one, by its stop, before the guest stops; `XPUTE_LINK` runs it as the
// guest's first export starts, so that it covers the guest's whole run. A bug outside an entry, in `init` before the
// host can hear a stop, or while the guest is out on `link_out`, is not told
// (`in_guest`): the trap tells it.
void hook() noexcept;

} // namespace xpute::guest::stop
