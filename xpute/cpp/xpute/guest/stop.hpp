// cpp/xpute/guest/stop.hpp

// The guest's report of its own failures to the host, by the stop the host
// serves (`wit/stop/stop.wit`), as `xpute-guest`'s `stop.rs` is a Rust
// guest's.

#pragma once

namespace xpute::guest::stop {

// Tells the host a bug in the guest's own code, by its stop, before the guest
// stops; `XPUTE_LINK` runs it before the guest's boot. A bug outside an entry,
// or while the guest is out on `link_out`, is not told (`in_guest`).
void hook() noexcept;

} // namespace xpute::guest::stop
