// cpp/xpute/core/sched/task.hpp

// A task is a C++20 coroutine. Its frame comes from the core's heap; when the
// heap has no room, no task is made. What resumes a task, and in what order, is
// the program's own scheduler, which xpute does not hold: the scheduler sets in
// a task's promise how to queue it again (`queue`) as it starts the task, and
// each task it awaits takes the same. No correctness may rest on the order in
// which tasks run; work that must run in order within a turn runs in one task.
// A task checks the deadline at its own yield points (turn.hpp).

#pragma once

#include <coroutine>
#include <cstddef>
#include <cstdlib>
#include <new>
#include <optional>
#include <type_traits>
#include <utility>

namespace xpute {

// A task to wake, and how its scheduler queues it to run again; with none,
// waking it does nothing.
struct Waker {
    std::coroutine_handle<> task;
    void (*queue)(std::coroutine_handle<> task) noexcept = nullptr;

    void wake() const noexcept {
        if (queue != nullptr) queue(task);
    }
};

template <class T = void> class Task;

namespace task_detail {

// A spawned task has no continuation; its frame is destroyed when it ends.
struct PromiseBase {
    // How the program's scheduler queues this task again (`Waker`).
    void (*queue)(std::coroutine_handle<> task) noexcept = nullptr;
    std::coroutine_handle<> continuation;

    static void *operator new(std::size_t n) noexcept { return ::operator new(n, std::nothrow); }
    static void operator delete(void *p, std::size_t n) noexcept { ::operator delete(p, n); }

    std::suspend_always initial_suspend() const noexcept { return {}; }

    struct Final {
        bool await_ready() const noexcept { return false; }
        template <class P> std::coroutine_handle<> await_suspend(std::coroutine_handle<P> h) const noexcept {
            std::coroutine_handle<> next = h.promise().continuation;
            if (next) return next;
            h.destroy();
            return std::noop_coroutine();
        }
        void await_resume() const noexcept {}
    };
    Final final_suspend() const noexcept { return {}; }

    // The core is built with exceptions off, so nothing can reach this.
    void unhandled_exception() const noexcept { std::abort(); }
};

template <class T> struct Promise : PromiseBase {
    std::optional<T> val;
    Task<T> get_return_object() noexcept;
    static Task<T> get_return_object_on_allocation_failure() noexcept;
    template <class U> void return_value(U &&v) noexcept { val.emplace(std::forward<U>(v)); }
};

template <> struct Promise<void> : PromiseBase {
    Task<void> get_return_object() noexcept;
    static Task<void> get_return_object_on_allocation_failure() noexcept;
    void return_void() const noexcept {}
};

} // namespace task_detail

template <class T> class [[nodiscard]] Task {
  public:
    using promise_type = task_detail::Promise<T>;
    using Handle = std::coroutine_handle<promise_type>;

    Task() noexcept = default;
    explicit Task(Handle h) noexcept : h_(h) {}
    Task(Task &&o) noexcept : h_(std::exchange(o.h_, {})) {}
    Task &operator=(Task &&o) noexcept {
        if (this != &o) {
            if (h_) h_.destroy();
            h_ = std::exchange(o.h_, {});
        }
        return *this;
    }
    Task(const Task &) = delete;
    Task &operator=(const Task &) = delete;
    ~Task() {
        if (h_) h_.destroy();
    }

    explicit operator bool() const noexcept { return bool(h_); }

    Handle release() noexcept { return std::exchange(h_, {}); }

    struct Awaiter {
        Handle h;
        bool await_ready() const noexcept { return false; }
        template <class P> std::coroutine_handle<> await_suspend(std::coroutine_handle<P> parent) const noexcept {
            h.promise().continuation = parent;
            h.promise().queue = parent.promise().queue;
            return h;
        }
        T await_resume() const noexcept {
            if constexpr (!std::is_void_v<T>) return std::move(*h.promise().val);
        }
    };

    Awaiter operator co_await() noexcept { return Awaiter{h_}; }

  private:
    Handle h_;
};

namespace task_detail {

template <class T> Task<T> Promise<T>::get_return_object() noexcept {
    return Task<T>(std::coroutine_handle<Promise<T>>::from_promise(*this));
}
template <class T> Task<T> Promise<T>::get_return_object_on_allocation_failure() noexcept {
    return Task<T>();
}
inline Task<void> Promise<void>::get_return_object() noexcept {
    return Task<void>(std::coroutine_handle<Promise<void>>::from_promise(*this));
}
inline Task<void> Promise<void>::get_return_object_on_allocation_failure() noexcept {
    return Task<void>();
}

} // namespace task_detail

template <class P> Waker waker_of(std::coroutine_handle<P> h) noexcept {
    return Waker{h, h.promise().queue};
}

} // namespace xpute
