// cpp/xpute/core/test.hpp

// What the C++ tests are written with: a test is a function registered by its
// name, and a check that fails says where and what, then stops the run — no
// framework, since there are no exceptions to unwind to one and nothing to
// gain from a runner the other two languages do not share. The tests are run
// by script/cpp.ts, which builds them with the platform's compiler.

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

// Every registered test, last registered first.
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

// Two values that must be equal; `what` names the case when they are not.
#define XPUTE_CHECK_EQ(a, b, what)                                                                                                                   \
    do {                                                                                                                                               \
        if (!((a) == (b))) ::xpute::test::fail(__FILE__, __LINE__, (what));                                                                           \
    } while (0)
