// cpp/xpute/core/host.cpp

#include "host.hpp"

#include <optional>

#include "xpute/kit/status/bug.hpp"

namespace xpute::host {

namespace {
std::optional<Host> installed;

const Host &host() noexcept {
    ensure(installed.has_value(), Errno::enosys);
    return *installed;
}
} // namespace

void set(const Host &host) noexcept {
    installed = host;
}

std::int32_t receive(std::uint32_t at, std::uint32_t cap) noexcept {
    return host().receive(at, cap);
}

void send(std::uint32_t at, std::uint32_t named_at, std::uint32_t named_len) noexcept {
    host().send(at, named_at, named_len);
}

std::int32_t call(std::uint32_t at) noexcept {
    return host().call(at);
}

} // namespace xpute::host
