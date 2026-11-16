// cpp/xpute/link/resource.hpp

// A handle names an object on the side that implements it, which issued it
// (`link.hpp`), as xpute-link's `resource.rs` issues it. As the component
// model's table does, a handle's slot is used again once its object is
// dropped, so that a side issues handles for as long as it runs, the slots
// dropped first used first, so that each comes back as late as it can; the
// slot is the handle's low `SLOT_BITS`, from 1, the root being 0. Above it a
// generation, which a slot's drop advances, keeps a frame late for a dropped
// object from reaching the one in its slot now, until the slot has been used
// `GENERATION_MASK` + 1 times. More live objects than the slots hold is a
// bug.

#pragma once

#include <cstdint>
#include <optional>
#include <utility>
#include <vector>

#include "xpute/kit/status/bug.hpp"
#include "frame.spec.hpp"

namespace xpute::link {

using Handle = std::uint32_t;

// The objects a side implements, by handle: slot `k` at `k - 1`, and the queue
// of the slots dropped, through their `next`, from `head_` to `tail_`.
template <class T> class Table {
  public:
    // Issues a handle, for `put` to fill; `remove` gives it back unfilled.
    Handle issue() noexcept {
        std::uint32_t k;
        if (head_ != 0) {
            k = head_;
            head_ = std::exchange(slots_[k - 1].next, ISSUED);
            if (head_ == 0) tail_ = 0;
        } else {
            slots_.push_back(Slot{0, std::nullopt, ISSUED});
            k = static_cast<std::uint32_t>(slots_.size());
            ensure(k <= SLOT_MASK, Errno::emfile, k);
        }
        return k | slots_[k - 1].generation << frame::SLOT_BITS;
    }

    // Puts `v` under `h`: a handle this table issued, or one the other end
    // issued, as a UI's end holds what the runtime opens on it under the
    // guest's end's handles. The slot `h` names is then issued at `h`'s
    // generation, taken out of the queue if it was free, it and the slots
    // before it made free where there were none; one issued at another
    // generation is a bug.
    void put(Handle h, T v) noexcept {
        std::uint32_t k = h & SLOT_MASK;
        ensure(k != 0, Errno::ebadf, h);
        while (slots_.size() < k) {
            slots_.push_back(Slot{0, std::nullopt, ISSUED});
            remove(static_cast<std::uint32_t>(slots_.size()));
        }
        Slot &s = slots_[k - 1];
        if (s.next != ISSUED) {
            std::uint32_t prev = 0;
            for (std::uint32_t c = head_; c != k; c = slots_[c - 1].next) prev = c;
            if (prev == 0) head_ = s.next;
            else slots_[prev - 1].next = s.next;
            if (tail_ == k) tail_ = prev;
            s.next = ISSUED;
            s.generation = h >> frame::SLOT_BITS;
        }
        ensure(s.generation == h >> frame::SLOT_BITS, Errno::ebadf, h);
        s.val = std::move(v);
    }

    // Puts `v` at a handle issued for it, and answers the handle.
    Handle add(T v) noexcept {
        Handle h = issue();
        put(h, std::move(v));
        return h;
    }

    T *get(Handle h) noexcept {
        Slot *s = slot(h);
        return s != nullptr && s->val ? &*s->val : nullptr;
    }

    // Drops `h`: its object, if it holds one, and its slot, last in the
    // queue, for a later handle of the next generation. A slot that is free
    // already is no handle's.
    std::optional<T> remove(Handle h) noexcept {
        Slot *s = slot(h);
        if (s == nullptr || s->next != ISSUED) return std::nullopt;
        std::optional<T> v = std::exchange(s->val, std::nullopt);
        s->generation = (s->generation + 1) & GENERATION_MASK;
        s->next = 0;
        std::uint32_t k = h & SLOT_MASK;
        if (tail_ == 0) head_ = k;
        else slots_[tail_ - 1].next = k;
        tail_ = k;
        return v;
    }

  private:
    static constexpr std::uint32_t SLOT_MASK = (1u << frame::SLOT_BITS) - 1;
    // The largest generation an issued handle holds above its slot, below
    // `NAMED`.
    static constexpr std::uint32_t GENERATION_MASK = (frame::NAMED - 1) >> frame::SLOT_BITS;
    // What an issued slot's `next` holds, which no free slot's does.
    static constexpr std::uint32_t ISSUED = UINT32_MAX;

    // A slot: its generation, its object while it holds one, and the next
    // free slot while it is free, 0 for none; `ISSUED` while it is issued.
    struct Slot {
        std::uint32_t generation;
        std::optional<T> val;
        std::uint32_t next;
    };

    // The slot `h` names, where `h` is of its generation.
    Slot *slot(Handle h) noexcept {
        std::uint32_t k = h & SLOT_MASK;
        if (k == 0 || k > slots_.size()) return nullptr;
        Slot &s = slots_[k - 1];
        return s.generation == h >> frame::SLOT_BITS ? &s : nullptr;
    }

    std::vector<Slot> slots_;
    std::uint32_t head_ = 0;
    std::uint32_t tail_ = 0;
};

} // namespace xpute::link
