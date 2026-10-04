// cpp/xpute/runtime/sched/task.hpp

// Coroutine tasks and the executor they run on, installed once like a global
// allocator. A task's frame comes from the guest's heap; one that finds no
// room makes no task. An executor may ignore priority, so nothing may rest on
// the order tasks are polled in.

#pragma once

#include <coroutine>
#include <cstddef>
#include <cstdint>
#include <cstdlib>
#include <new>
#include <optional>
#include <type_traits>
#include <utility>

namespace xpute {

// Lower is sooner.
using Priority = std::uint8_t;

class Executor {
  public:
    virtual bool spawn(std::coroutine_handle<> task, Priority priority) noexcept = 0;
    virtual void wake(std::coroutine_handle<> task) noexcept = 0;
    // Never from inside a task.
    virtual void poll() noexcept = 0;
    virtual bool pending() const noexcept = 0;

  protected:
    ~Executor() = default;
};

struct Waker {
    std::coroutine_handle<> task;
    Executor *executor = nullptr;

    void wake() const noexcept {
        if (executor != nullptr) executor->wake(task);
    }
};

template <class T = void> class Task;

namespace task_detail {

// A spawned task has no continuation; its frame is destroyed when it ends.
struct PromiseBase {
    Executor *executor = nullptr;
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

    // Exceptions are off; nothing can arrive here.
    void unhandled_exception() const noexcept { std::abort(); }
};

template <class T> struct Promise : PromiseBase {
    std::optional<T> value;
    Task<T> get_return_object() noexcept;
    static Task<T> get_return_object_on_allocation_failure() noexcept;
    template <class U> void return_value(U &&v) noexcept { value.emplace(std::forward<U>(v)); }
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
            h.promise().executor = parent.promise().executor;
            return h;
        }
        T await_resume() const noexcept {
            if constexpr (!std::is_void_v<T>) return std::move(*h.promise().value);
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
    return Waker{h, h.promise().executor};
}

void set_executor(Executor &executor) noexcept;

Executor *executor() noexcept;

bool spawn(Task<> task, Priority priority) noexcept;

void poll() noexcept;

bool pending() noexcept;

} // namespace xpute
