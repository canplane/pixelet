// cpp/xpute/kit/status/verbose_abort.hpp

// A failed check in libc++'s headers is a broken invariant, so it is a bug on
// every target, reported with the check's own message (bug.cpp
// `xpute_verbose_abort`). libc++'s headers reach it
// through `_LIBCPP_VERBOSE_ABORT`, a macro a program may define before the
// first of them, so this header includes none: the tree's files are compiled
// with it included first (Makefile), and the amalgamation's header opens with
// it. libc++'s compiled library calls its own handler instead, which only a
// module replaces (bug.cpp).

#pragma once

extern "C" [[noreturn]] void xpute_verbose_abort(const char *fmt, ...) noexcept __attribute__((format(printf, 1, 2)));

#define _LIBCPP_VERBOSE_ABORT(...) ::xpute_verbose_abort(__VA_ARGS__)
