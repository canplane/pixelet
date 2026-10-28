// cpp/xpute/kit/collection/collection.test.cpp

#include <array>
#include <cstdlib>
#include <memory>
#include <string>
#include <utility>
#include <vector>

#include "xpute/kit/golden.hpp"
#include "xpute/kit/test.hpp"
#include "arena.hpp"
#include "deque.hpp"
#include "heap.hpp"
#include "list.hpp"

using namespace xpute;

XPUTE_TEST(deque_matches_record) {
    Golden v = Golden::load("spec/golden/collection/deque.tsv");
    std::array<std::uint32_t, 8> buf{};
    Deque<std::uint32_t> d{buf};
    auto show = [](const std::uint32_t *x) { return x ? std::to_string(*x) : std::string("none"); };
    v.each("", [&](const std::string &k) {
        std::uint32_t step = static_cast<std::uint32_t>(std::stoul(k));
        std::uint32_t r = v.u32(k + ".r");
        std::string popped = "none";
        if (r == 0 && !d.is_full()) {
            d.push_back(step);
        } else if (r == 1 && !d.is_full()) {
            d.push_front(step);
        } else if (r == 2) {
            if (auto x = d.pop_front()) popped = std::to_string(*x);
        } else if (r == 3) {
            if (auto x = d.pop_back()) popped = std::to_string(*x);
        }
        XPUTE_CHECK_EQ(popped, v.s(k + ".popped"), (k + ".popped").c_str());
        XPUTE_CHECK_EQ(d.head, v.u32(k + ".head"), (k + ".head").c_str());
        XPUTE_CHECK_EQ(d.len, v.u32(k + ".len"), (k + ".len").c_str());
        XPUTE_CHECK_EQ(show(d.front()), v.s(k + ".front"), (k + ".front").c_str());
        XPUTE_CHECK_EQ(show(d.back()), v.s(k + ".back"), (k + ".back").c_str());
    });
}

using E = Element<double, std::int32_t>;

static std::string show(const E &e) {
    // The record writes a key as Rust writes an f64 that is an integer: no
    // decimal point.
    return std::to_string(static_cast<long long>(e.key)) + ":" + std::to_string(e.val);
}

XPUTE_TEST(heap_matches_record) {
    Golden v = Golden::load("spec/golden/collection/heap.tsv");
    std::vector<E> a, b;
    v.each("steps", [&](const std::string &k) {
        if (v.s(k + ".op") == "pop") {
            auto min = MinHeap<std::vector<E>>(a).pop();
            auto max = MaxHeap<std::vector<E>>(b).pop();
            XPUTE_CHECK_EQ(min ? show(*min) : "none", v.s(k + ".min"), (k + ".min").c_str());
            XPUTE_CHECK_EQ(max ? show(*max) : "none", v.s(k + ".max"), (k + ".max").c_str());
        } else {
            E e{static_cast<double>(v.i32(k + ".key")), v.i32(k + ".val")};
            MinHeap<std::vector<E>>(a).push(e);
            MaxHeap<std::vector<E>>(b).push(e);
        }
    });
    std::vector<E> arr;
    for (std::size_t j = 0; j < v.len("input"); j++) {
        const std::string &s = v.s("input." + std::to_string(j));
        std::size_t colon = s.find(':');
        arr.push_back(E{std::strtod(s.substr(0, colon).c_str(), nullptr), std::stoi(s.substr(colon + 1))});
    }
    heapsort<double, std::int32_t>(arr);
    for (std::size_t j = 0; j < arr.size(); j++) XPUTE_CHECK_EQ(show(arr[j]), v.s("sorted." + std::to_string(j)), ("sorted." + std::to_string(j)).c_str());
}

namespace {

struct Slot {
    // What a slot owns and keeps across a rewind, which is the whole point: a
    // vector's buffer is still allocated when the slot is handed out again.
    std::vector<std::uint32_t> kept;
    std::uint32_t mark = 0;
};

// A test is a host's program, so its arena may take the system's memory.
using SlotArena = Arena<Slot, std::allocator<Slot>>;

SlotArena arena(std::uint32_t init_cap, std::uint32_t max_cap) {
    return std::move(*SlotArena::make({init_cap, max_cap}));
}

} // namespace

XPUTE_TEST(arena_rewind_keeps_buffers) {
    SlotArena a = arena(4, 1 << 10);
    for (std::uint32_t i = 0; i < 4; i++) {
        Slot *slot = *a.alloc();
        slot->kept.assign({i, i + 1, i + 2});
        slot->mark = i;
    }
    std::array<std::size_t, 4> caps{};
    for (std::uint32_t i = 0; i < 4; i++) caps[i] = a.get(i).kept.capacity();
    a.truncate(0);
    XPUTE_CHECK(a.len() == 0);
    for (std::uint32_t i = 0; i < 4; i++) {
        Slot *slot = *a.alloc();
        XPUTE_CHECK_EQ(slot->mark, i, "a rewound slot was rebuilt rather than kept");
        XPUTE_CHECK_EQ(slot->kept, (std::vector<std::uint32_t>{i, i + 1, i + 2}), "a rewound slot lost what it owned");
    }
    for (std::uint32_t i = 0; i < 4; i++) XPUTE_CHECK_EQ(a.get(i).kept.capacity(), caps[i], "a rewind gave a slot's buffer back");
}

XPUTE_TEST(arena_growth_doubles) {
    SlotArena a = arena(2, 1 << 10);
    XPUTE_CHECK(a.cap() == 2);
    for (std::uint32_t i = 0; i < 5; i++) (*a.alloc())->mark = i;
    XPUTE_CHECK_EQ(a.cap(), 8u, "the cap did not double to hold five");
    for (std::uint32_t i = 0; i < 5; i++) XPUTE_CHECK_EQ(a.get(i).mark, i, "growth lost what a slot held");
    XPUTE_CHECK(a.reserve(100).ok());
    std::uint32_t cap = a.cap();
    XPUTE_CHECK(cap >= 105);
    for (int i = 0; i < 100; i++) XPUTE_CHECK(a.alloc().ok());
    XPUTE_CHECK_EQ(a.cap(), cap, "an alloc after reserve grew again");
}

XPUTE_TEST(arena_growth_past_max_refused) {
    SlotArena a = arena(2, 4);
    for (std::uint32_t i = 0; i < 4; i++) (*a.alloc())->mark = i + 10;
    XPUTE_CHECK(a.cap() == 4);
    Result<Slot *> refused = a.alloc();
    XPUTE_CHECK_EQ(refused.ok(), false, "the arena grew past max_cap");
    XPUTE_CHECK_EQ(refused.error().errnum, Errno::eoverflow, "refused for the wrong reason");
    XPUTE_CHECK_EQ(a.len(), 4u, "a refused alloc moved the cursor");
    XPUTE_CHECK(a.cap() == 4);
    for (std::uint32_t i = 0; i < 4; i++) XPUTE_CHECK_EQ(a.get(i).mark, i + 10, "a refusal wrote over a live slot");
    XPUTE_CHECK(!a.reserve(1).ok());
}

XPUTE_TEST(arena_scope_rewinds) {
    SlotArena a = arena(8, 1 << 10);
    (*a.alloc())->mark = 1;
    XPUTE_CHECK(a.len() == 1);
    {
        auto pass = a.scope();
        for (int i = 0; i < 5; i++) XPUTE_CHECK(pass->alloc().ok());
        XPUTE_CHECK(pass->len() == 6);
    }
    XPUTE_CHECK_EQ(a.len(), 1u, "the scope did not rewind to its mark");
    XPUTE_CHECK_EQ(a.get(0).mark, 1u, "the rewind reached past the mark");
}

namespace {

// Counts the values it ends, so a test sees each live one dropped once.
struct Counted {
    int n;
    int *ended;
    Counted(int n, int *ended) noexcept : n(n), ended(ended) {}
    Counted(Counted &&o) noexcept : n(o.n), ended(std::exchange(o.ended, nullptr)) {}
    Counted(const Counted &) = delete;
    ~Counted() {
        if (ended != nullptr) (*ended)++;
    }
};

template <std::size_t N> std::vector<int> values(const List<Counted, N> &l) {
    std::vector<int> out;
    for (const Counted &c : l) out.push_back(c.n);
    return out;
}

} // namespace

XPUTE_TEST(list_order_and_drop) {
    int ended = 0;
    {
        List<Counted, 8> l;
        XPUTE_CHECK(l.is_empty() && l.as_slice().empty());
        for (int i = 1; i <= 6; i++) l.push(Counted(i, &ended));
        XPUTE_CHECK_EQ(l.len(), 6u, "six pushed");
        XPUTE_CHECK_EQ(ended, 0, "a push ended a value");

        Counted gone = l.remove(1);
        XPUTE_CHECK_EQ(gone.n, 2, "remove answered another value");
        XPUTE_CHECK_EQ(values(l), (std::vector<int>{1, 3, 4, 5, 6}), "remove did not keep the order");
        XPUTE_CHECK_EQ(l[4].n, 6, "the last moved down one");
        l.remove(4);
        XPUTE_CHECK_EQ(ended, 1, "the last removed, unkept, ends once");

        l.retain([](const Counted &c) { return c.n != 3 && c.n != 5; });
        XPUTE_CHECK_EQ(values(l), (std::vector<int>{1, 4}), "retain did not keep the order of the rest");
        XPUTE_CHECK_EQ(ended, 3, "retain ends what it refuses, once each");

        l.as_mut_slice()[0].n = 7;
        List<Counted, 8> moved(std::move(l));
        XPUTE_CHECK(l.is_empty());
        XPUTE_CHECK_EQ(values(moved), (std::vector<int>{7, 4}), "a move lost a value");
        XPUTE_CHECK_EQ(ended, 3, "a move ended a value");

        moved.clear();
        XPUTE_CHECK(moved.is_empty());
        XPUTE_CHECK_EQ(ended, 5, "clear ends every live value");
        for (int i = 0; i < 8; i++) moved.push(Counted(i, &ended));
        XPUTE_CHECK_EQ(moved.len(), 8u, "a cleared list holds N again");
    }
    XPUTE_CHECK_EQ(ended, 14, "the list ends what it holds when it goes, and `gone` its own");
}
