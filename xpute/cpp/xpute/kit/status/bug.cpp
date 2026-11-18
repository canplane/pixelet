// cpp/xpute/kit/status/bug.cpp

#include "bug.hpp"
#include "verbose_abort.hpp"

#include <atomic>
#include <cstdarg>
#include <cstddef>
#include <cstdlib>

namespace xpute {

namespace {
std::atomic<Report> installed_report{nullptr};
}

void set_report(Report report) noexcept {
    Report none = nullptr;
    installed_report.compare_exchange_strong(none, report);
}

#ifndef __wasm__
sigjmp_buf *&bug_detail::catcher() noexcept {
    thread_local sigjmp_buf *innermost = nullptr;
    return innermost;
}
#endif

void bug(Errno errnum, std::uint64_t a, std::uint64_t b, const char *msg, std::source_location at) noexcept {
    if (Report report = installed_report.load()) report(errnum, at, a, b, msg);
#ifndef __wasm__
    if (sigjmp_buf *c = bug_detail::catcher()) siglongjmp(*c, 1);
#endif
    std::abort();
}

} // namespace xpute

namespace {
// The bytes of a libc++ check's message: its file, its line, the expression
// and what it says of it, a longer one cut.
constexpr std::size_t ABORT_MSG_CAP = 512;

// The bug of a failed check of libc++'s own, with the check's message as
// `fmt` and `args` write it. The message is written into one static buffer,
// so that it is storage nothing allocates, as a bug's message is, one a
// thread, as a native bug is caught on its own thread. It is
// written here and not by `vsnprintf`, which links WASI's file calls into a
// module: a string, a character and an integer of any length are written, and
// from a conversion that is none of them on, `fmt` goes as it is, since
// nothing says how wide its argument is.
[[noreturn]] void abort_bug(const char *fmt, std::va_list args) noexcept {
    static thread_local char msg[ABORT_MSG_CAP];
    std::size_t n = 0;
    auto put = [&](char c) {
        if (n + 1 < ABORT_MSG_CAP) msg[n++] = c;
    };
    auto number = [&](unsigned long long v, unsigned base) {
        char digits[24];
        std::size_t k = 0;
        do digits[k++] = "0123456789abcdef"[v % base];
        while ((v /= base) != 0);
        while (k != 0) put(digits[--k]);
    };
    const char *f = fmt;
    while (*f != '\0') {
        if (*f != '%') {
            put(*f++);
            continue;
        }
        const char *spec = f + 1;
        int length = 0; // 0 an int, 1 a long, 2 a long long, 3 a size_t
        for (; *spec == 'l' || *spec == 'z'; spec++) length = *spec == 'z' ? 3 : length + 1;
        auto is_signed = *spec == 'd' || *spec == 'i';
        if (*spec == '%') {
            put('%');
        } else if (*spec == 's' && length == 0) {
            const char *str = va_arg(args, const char *);
            for (; str != nullptr && *str != '\0'; str++) put(*str);
        } else if (*spec == 'c' && length == 0) {
            put(static_cast<char>(va_arg(args, int)));
        } else if ((is_signed || *spec == 'u' || *spec == 'x') && length <= 3) {
            unsigned long long v;
            if (length == 0) {
                v = is_signed ? static_cast<unsigned long long>(va_arg(args, int)) : va_arg(args, unsigned);
            } else if (length == 1) {
                v = is_signed ? static_cast<unsigned long long>(va_arg(args, long)) : va_arg(args, unsigned long);
            } else if (length == 2) {
                v = va_arg(args, unsigned long long);
            } else {
                v = va_arg(args, std::size_t);
            }
            if (is_signed && static_cast<long long>(v) < 0) {
                put('-');
                v = 0 - v;
            }
            number(v, *spec == 'x' ? 16 : 10);
        } else {
            for (; *f != '\0'; f++) put(*f);
            break;
        }
        f = spec + 1;
    }
    msg[n] = '\0';
    xpute::bug(xpute::Errno::enotrecoverable, 0, 0, msg);
}
} // namespace

void xpute_verbose_abort(const char *fmt, ...) noexcept {
    std::va_list args;
    va_start(args, fmt);
    abort_bug(fmt, args);
}

#ifdef __wasm__
// libc++'s compiled library calls its own handler, not the macro, and its
// definition prints to stderr. On WebAssembly that imports WASI's file calls,
// which would give the module a second way to reach the host beside the
// contract, so a module's handler is the bug as well.
void std::__libcpp_verbose_abort(char const *fmt, ...) noexcept {
    std::va_list args;
    va_start(args, fmt);
    abort_bug(fmt, args);
}

// libc++abi's `__cxa_pure_virtual`, which an abstract class's vtable names,
// prints to stderr in the same way, so it is replaced as well.
extern "C" void __cxa_pure_virtual() {
    xpute::bug(xpute::Errno::enotrecoverable);
}

// So is libc++abi's `__abort_message`, which its type information's vtables
// reach, and which a class over one of libc++'s, as the kit's `HeapMalloc`
// memory resource is, links in.
extern "C" void __abort_message(const char *fmt, ...) {
    std::va_list args;
    va_start(args, fmt);
    abort_bug(fmt, args);
}
#endif
