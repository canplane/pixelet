// cpp/dev/test/test.hpp

// This is a minimal test harness that needs no exceptions. A failed check
// prints its file and line and aborts the run.

#pragma once

#include <cstdio>
#include <cstdlib>

namespace xpute::test {

using Fn = void (*)();

struct Case {
    const char *name;
    Fn fn;
    Case *next;
};

Case *&cases() noexcept;

struct Register {
    Case node;
    Register(const char *name, Fn fn) noexcept : node{name, fn, cases()} { cases() = &node; }
};

[[noreturn]] inline void fail(const char *file, int line, const char *what) noexcept {
    std::fprintf(stderr, "%s:%d: %s\n", file, line, what);
    std::abort();
}

} // namespace xpute::test

#define XPUTE_TEST(name)                                                                                                                             \
    static void name();                                                                                                                                \
    static ::xpute::test::Register name##_registered(#name, name);                                                                                     \
    static void name()

#define XPUTE_CHECK(cond)                                                                                                                            \
    do {                                                                                                                                               \
        if (!(cond)) ::xpute::test::fail(__FILE__, __LINE__, #cond);                                                                                   \
    } while (0)

#define XPUTE_CHECK_EQ(a, b, what)                                                                                                                   \
    do {                                                                                                                                               \
        if (!((a) == (b))) ::xpute::test::fail(__FILE__, __LINE__, (what));                                                                           \
    } while (0)
