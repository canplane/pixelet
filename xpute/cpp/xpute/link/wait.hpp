// cpp/xpute/link/wait.hpp

// A sent method as an awaitable, over a side's `send`, with no executor of
// its own, as xpute-link's `wait.rs` is a future. The method is sent when the
// wait is made. Its answer keeps the result, the return data and what the
// args lend, and resumes the coroutine that awaits it; the wait hands all
// three back. Destroying the wait before its answer cancels the call
// (`frame.json5` `CANCEL`), as the component model's dropped subtask is
// canceled; what the args lend stays alive until the answer, which still
// comes and owns it (`link/wit/types.wit` `lent`). The wait owns its slot
// until it is destroyed with the answer still to come; the answer owns it
// from then on, and frees it as it comes.
//
// A `Task` is the coroutine that awaits: what C++ needs to write an async
// function, which Rust's `async` gives. It runs as it is made, up to its
// first wait, so a call it makes is sent then, and it holds its frame: a task
// destroyed before it ends destroys the waits it holds, which cancels their
// calls. Awaiting a task resumes the awaiting coroutine once it ends.

#pragma once

#include <coroutine>
#include <cstdint>
#include <optional>
#include <tuple>
#include <type_traits>
#include <utility>
#include <vector>

#include "xpute/kit/status/bug.hpp"
#include "link.hpp"

namespace xpute::link {

// What a sent method comes to: its result, its return data and what its args
// lent.
template <class L> using Outcome = std::tuple<std::int32_t, std::optional<std::vector<std::uint8_t>>, L>;

// A side's send, which answers the request's tag (`guest/link.hpp` `send`).
using Send = Tag (*)(Handle, Method, const Args &, Answer);

template <class L> class [[nodiscard]] Wait {
  public:
    // Sends `m` of `h` by `by`, a side's send, with `args`, now; `lent` is
    // what the args lend by (at, len), kept until the answer. `cancel` is the
    // side's, which a destruction before the answer calls.
    Wait(Send by, void (*cancel)(Tag), Handle h, Method m, const Args &args, L lent)
        : slot_(new Slot(std::move(lent))), cancel_(cancel) {
        tag_ = by(h, m, args, [slot = slot_](std::int32_t result, std::optional<std::vector<std::uint8_t>> data) {
            if (slot->orphan) {
                delete slot;
                return;
            }
            slot->answer.emplace(result, std::move(data));
            if (std::coroutine_handle<> w = std::exchange(slot->continuation, {})) w.resume();
        });
    }

    Wait(Wait &&o) noexcept : slot_(std::exchange(o.slot_, nullptr)), tag_(o.tag_), cancel_(o.cancel_) {}
    Wait &operator=(Wait &&) = delete;
    Wait(const Wait &) = delete;

    ~Wait() {
        if (slot_ == nullptr) return;
        // Once the answer has come, a destruction cancels nothing.
        if (slot_->answer) {
            delete slot_;
            return;
        }
        slot_->orphan = true;
        cancel_(tag_);
    }

    bool await_ready() const noexcept { return slot_->answer.has_value(); }
    void await_suspend(std::coroutine_handle<> h) noexcept { slot_->continuation = h; }
    Outcome<L> await_resume() {
        auto [result, data] = std::move(*slot_->answer);
        return Outcome<L>(result, std::move(data), std::move(slot_->lent));
    }

  private:
    // Where the answer leaves its result and return data, beside what the
    // args lent, and the coroutine that awaits it.
    struct Slot {
        explicit Slot(L lent) : lent(std::move(lent)) {}
        L lent;
        std::optional<std::pair<std::int32_t, std::optional<std::vector<std::uint8_t>>>> answer;
        std::coroutine_handle<> continuation;
        // Whether the wait is gone, and the answer owns the slot.
        bool orphan = false;
    };

    Slot *slot_;
    Tag tag_ = 0;
    void (*cancel_)(Tag);
};

template <class T> class Task;

namespace task_detail {

struct PromiseBase {
    // The coroutine that awaits the task, resumed as it ends.
    std::coroutine_handle<> continuation;

    std::suspend_never initial_suspend() const noexcept { return {}; }

    struct Final {
        bool await_ready() const noexcept { return false; }
        template <class P> std::coroutine_handle<> await_suspend(std::coroutine_handle<P> h) const noexcept {
            if (std::coroutine_handle<> a = h.promise().continuation) return a;
            return std::noop_coroutine();
        }
        void await_resume() const noexcept {}
    };
    Final final_suspend() const noexcept { return {}; }

    // Nothing is thrown where exceptions are off.
    void unhandled_exception() const noexcept { bug(Errno::enotrecoverable); }
};

template <class T> struct Promise : PromiseBase {
    std::optional<T> val;
    Task<T> get_return_object() noexcept;
    void return_value(T v) noexcept { val.emplace(std::move(v)); }
};

template <> struct Promise<void> : PromiseBase {
    Task<void> get_return_object() noexcept;
    void return_void() const noexcept {}
};

} // namespace task_detail

template <class T> class [[nodiscard]] Task {
  public:
    using promise_type = task_detail::Promise<T>;

    explicit Task(std::coroutine_handle<promise_type> h) noexcept : h_(h) {}
    Task(Task &&o) noexcept : h_(std::exchange(o.h_, {})) {}
    Task &operator=(Task &&o) noexcept {
        if (this != &o) {
            if (h_) h_.destroy();
            h_ = std::exchange(o.h_, {});
        }
        return *this;
    }
    ~Task() {
        if (h_) h_.destroy();
    }

    // Whether the task has ended.
    bool done() const noexcept { return h_.done(); }

    // Its value, once it has ended; asking before is a bug.
    T get() {
        ensure(h_.done(), Errno::ebusy);
        if constexpr (!std::is_void_v<T>) return std::move(*h_.promise().val);
    }

    bool await_ready() const noexcept { return h_.done(); }
    void await_suspend(std::coroutine_handle<> a) noexcept { h_.promise().continuation = a; }
    T await_resume() { return get(); }

  private:
    std::coroutine_handle<promise_type> h_;
};

namespace task_detail {

template <class T> Task<T> Promise<T>::get_return_object() noexcept {
    return Task<T>(std::coroutine_handle<Promise<T>>::from_promise(*this));
}

inline Task<void> Promise<void>::get_return_object() noexcept {
    return Task<void>(std::coroutine_handle<Promise<void>>::from_promise(*this));
}

} // namespace task_detail

} // namespace xpute::link
