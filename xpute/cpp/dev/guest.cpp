// cpp/dev/guest.cpp

// The smallest guest, which `make wasm` links over the amalgamation as a
// module, as a program's guest is linked (Makefile `MODULE_FLAGS`). It
// implements nothing, and its entries start and end with nothing to do.

#include "xpute.hpp"

namespace {

void boot() {}

void enter() {}

bool end() {
    return false;
}

} // namespace

XPUTE_LINK(4096, boot, enter, end)
