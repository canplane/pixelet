// cpp/xpute/core/collection/heap.hpp

// A binary heap over an array the caller owns, keyed by whatever orders. The
// array stays the caller's — any container with the vector's operations, so
// one whose memory comes from the guest's heap — and the key is anything with
// `<`, so a distance in a double keys it as it is.
//
// One implementation serves both directions: `MIN` picks which way a key
// outranks another and folds away at instantiation.
//
// Keys are totally ordered and the caller upholds it: a NaN key makes the two
// ways of asking — `ahead` and its negation — disagree, and the sift walks off
// the order.

#pragma once

#include <cstddef>
#include <optional>
#include <span>
#include <utility>

namespace xpute {

template <class K, class T> struct Element {
    K key;
    T val;
    bool operator==(const Element &) const = default;
};

// `Vec` holds the elements and is the caller's; `MIN` is the direction:
// smaller key first when true, larger when false.
template <class Vec, bool MIN> class Heap {
  public:
    using E = typename Vec::value_type;

    explicit Heap(Vec &a) noexcept : a_(a) {}

    // The root: the smallest key, or the largest.
    const E *top() const noexcept { return a_.empty() ? nullptr : &a_[0]; }

    // Restores the heap property in `a[0..size)`, given that both of the
    // root's subtrees already hold it.
    static void heapify(std::span<E> a, std::size_t root, std::size_t size) noexcept {
        std::size_t i = root;
        E e = a[i];
        std::size_t child = (i << 1) + 1;
        while (child < size) {
            std::size_t r = child + 1;
            if (r < size && ahead(a[r].key, a[child].key)) child = r;
            if (!ahead(a[child].key, e.key)) break;
            a[i] = a[child];
            i = child;
            child = (i << 1) + 1;
        }
        a[i] = e;
    }

    // Bottom-up heapify, O(n).
    static void build_of(std::span<E> a) noexcept {
        for (std::size_t i = a.size() >> 1; i-- > 0;) heapify(a, i, a.size());
    }

    void build() noexcept { build_of(a_); }

    // Hole sift-up: the new element's place is opened by moving parents
    // down, so one write lands it rather than a swap per level.
    void push(E e) {
        std::size_t i = a_.size();
        a_.push_back(e);
        while (i > 0) {
            std::size_t p = (i - 1) >> 1;
            if (!ahead(e.key, a_[p].key)) break;
            a_[i] = a_[p];
            i = p;
        }
        a_[i] = e;
    }

    // Removes the root and returns it.
    std::optional<E> pop() noexcept {
        if (a_.empty()) return std::nullopt;
        E out = a_[0];
        a_[0] = a_.back();
        a_.pop_back();
        if (!a_.empty()) heapify(a_, 0, a_.size());
        return out;
    }

    // Overwrites the root and sifts it down. The heap must not be empty.
    void replace_root(E e) noexcept {
        a_[0] = e;
        heapify(a_, 0, a_.size());
    }

  private:
    // Whether `x` outranks `y`. The only place the direction is read.
    static bool ahead(const auto &x, const auto &y) noexcept {
        if constexpr (MIN) return x < y;
        else return y < x;
    }

    Vec &a_;
};

template <class Vec> using MinHeap = Heap<Vec, true>;
template <class Vec> using MaxHeap = Heap<Vec, false>;

// Sorts ascending by key: unstable, in place, O(n log n). A max-heap over the
// whole array, then each root swapped to the end of the unsorted part.
template <class K, class T> void heapsort(std::span<Element<K, T>> a) noexcept {
    using H = Heap<std::span<Element<K, T>>, false>;
    std::size_t n = a.size();
    if (n <= 1) return;
    H::build_of(a);
    for (std::size_t end = n - 1; end > 0; end--) {
        std::swap(a[0], a[end]);
        H::heapify(a, 0, end);
    }
}

} // namespace xpute
