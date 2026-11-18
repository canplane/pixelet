// cpp/xpute/guest/log.hpp

// The guest's log records, told to its host by the log it implements
// (`wit/log/log.wit`), as `xpute-guest`'s `log.rs` is a Rust guest's: a
// level, a context and a message, each record worth telling and neither an
// error nor a bug.

#pragma once

#include <string_view>

#include "xpute/guest/abi/log.hpp"

namespace xpute::guest {

// Tells the host `msg`, at `level`, about `context`. A host that hears
// nothing more, the link trapped among them, has nothing to answer, so the
// answer is not read.
void log(abi::log::Level level, std::string_view context, std::string_view msg) noexcept;

} // namespace xpute::guest
