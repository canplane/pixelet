// cpp/xpute/core/collection/heap.hpp

// A binary heap over an array the caller owns. Keys must be totally ordered:
// a NaN key makes the sift walk off the order.

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

// `MIN` puts the smallest key first.
template <class Vec, bool MIN> class Heap {
  public:
    using E = typename Vec::value_type;

    explicit Heap(Vec &a) noexcept : a_(a) {}

    const E *top() const noexcept { return a_.empty() ? nullptr : &a_[0]; }

    // Both of the root's subtrees must already hold the heap property.
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

    static void build_of(std::span<E> a) noexcept {
        for (std::size_t i = a.size() >> 1; i-- > 0;) heapify(a, i, a.size());
    }

    void build() noexcept { build_of(a_); }

    // The new element's place is opened by moving parents down, so one write
    // lands it rather than a swap per level.
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

    std::optional<E> pop() noexcept {
        if (a_.empty()) return std::nullopt;
        E out = a_[0];
        a_[0] = a_.back();
        a_.pop_back();
        if (!a_.empty()) heapify(a_, 0, a_.size());
        return out;
    }

    // The heap must not be empty.
    void replace_root(E e) noexcept {
        a_[0] = e;
        heapify(a_, 0, a_.size());
    }

  private:
    static bool ahead(const auto &x, const auto &y) noexcept {
        if constexpr (MIN) return x < y;
        else return y < x;
    }

    Vec &a_;
};

template <class Vec> using MinHeap = Heap<Vec, true>;
template <class Vec> using MaxHeap = Heap<Vec, false>;

// Unstable, in place, O(n log n).
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
