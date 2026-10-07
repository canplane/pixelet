// cpp/guest/sched/task.cpp

#include "task.hpp"

namespace xpute {

namespace {
Executor *installed = nullptr;
}

void set_executor(Executor &executor) noexcept {
    installed = &executor;
}

Executor *executor() noexcept {
    return installed;
}

bool spawn(Task<> task, Priority priority) noexcept {
    if (installed == nullptr || !task) return false;
    Task<>::Handle h = task.release();
    h.promise().executor = installed;
    if (installed->spawn(h, priority)) return true;
    h.destroy();
    return false;
}

void poll() noexcept {
    if (installed != nullptr) installed->poll();
}

bool pending() noexcept {
    return installed != nullptr && installed->pending();
}

} // namespace xpute
