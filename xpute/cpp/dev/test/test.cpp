// cpp/dev/test/test.cpp

#include "test.hpp"

namespace xpute::test {

Case *&cases() noexcept {
    static Case *head = nullptr;
    return head;
}

} // namespace xpute::test

int main() {
    using xpute::test::Case;
    // Each test registers at the front of the list, so the list is reversed
    // to run the tests in the order they are written.
    Case *reversed = nullptr;
    for (Case *c = xpute::test::cases(); c != nullptr;) {
        Case *next = c->next;
        c->next = reversed;
        reversed = c;
        c = next;
    }
    int n = 0;
    for (Case *c = reversed; c != nullptr; c = c->next, n++) {
        std::printf("test %s ...\n", c->name);
        c->fn();
    }
    std::printf("ok: %d passed\n", n);
    return 0;
}
