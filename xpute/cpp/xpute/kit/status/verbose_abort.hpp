// cpp/xpute/kit/status/verbose_abort.hpp

// A failed check of libc++'s is a broken invariant, so it is a bug on every
// target (bug.cpp `xpute_verbose_abort`). libc++ reaches it through
// `_LIBCPP_VERBOSE_ABORT`, a macro a program may define before the first of
// libc++'s headers, so this header includes none: the tree's files are
// compiled with it included first (Makefile), and the amalgamation's header
// opens with it.

#pragma once

extern "C" [[noreturn]] void xpute_verbose_abort() noexcept;

#define _LIBCPP_VERBOSE_ABORT(...) ::xpute_verbose_abort()
