// cpp/xpute/guest/sched/task.cpp

#include "task.hpp"

namespace xpute {

namespace {
Executor *installed_executor = nullptr;
}

void set_executor(Executor &executor) noexcept {
    installed_executor = &executor;
}

Executor *executor() noexcept {
    return installed_executor;
}

bool spawn(Task<> task, Priority priority) noexcept {
    if (installed_executor == nullptr || !task) return false;
    Task<>::Handle h = task.release();
    h.promise().executor = installed_executor;
    if (installed_executor->spawn(h, priority)) return true;
    h.destroy();
    return false;
}

void poll() noexcept {
    if (installed_executor != nullptr) installed_executor->poll();
}

bool pending() noexcept {
    return installed_executor != nullptr && installed_executor->pending();
}

} // namespace xpute
