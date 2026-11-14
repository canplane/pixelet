// cpp/xpute/guest/wait.hpp

// A sent method as an awaitable, on the guest's side (xpute/link wait.hpp),
// for the generated bindings' calls, as xpute-guest's wait.rs is.

#pragma once

#include <utility>

#include "xpute/link/link.hpp"
#include "xpute/link/wait.hpp"
#include "link.hpp"

namespace xpute::guest {

template <class L> link::Wait<L> wait(link::Handle h, link::Method m, const link::Args &args, L lent) {
    return link::Wait<L>(send, cancel, h, m, args, std::move(lent));
}

} // namespace xpute::guest
