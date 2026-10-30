// cpp/xpute/guest/ipc/sys.cpp

#include "sys.hpp"

#include <array>
#include <cstring>
#include <utility>

#include "xpute/kit/status/bug.hpp"
#include "xpute/guest/hostcall.hpp"

namespace xpute {

static std::span<const std::uint8_t> bytes_of(std::string_view s) noexcept {
    return {reinterpret_cast<const std::uint8_t *>(s.data()), s.size()};
}

void Sys::openat(Fd fd, std::uint32_t root, std::uint32_t flags, std::string_view path) noexcept {
    const std::array<std::uint32_t, OPENAT_ARGS> args{fd, root, flags};
    stream_.record(sys::op::OPENAT, args, {}, bytes_of(path));
}

void Sys::read(Fd fd, std::span<std::uint8_t> dst) noexcept {
    const std::array<std::uint32_t, READ_ARGS> args{fd, mem_.off_of(dst), static_cast<std::uint32_t>(dst.size())};
    stream_.record(sys::op::READ, args);
}

void Sys::write(Fd fd, std::vector<std::uint8_t> bytes) noexcept {
    const std::array<std::uint32_t, WRITE_ARGS> args{fd, mem_.off_of(std::span<const std::uint8_t>(bytes)), static_cast<std::uint32_t>(bytes.size())};
    stream_.record(sys::op::WRITE, args);
    held_.push_back(std::move(bytes));
}

void Sys::close(Fd fd) noexcept {
    const std::array<std::uint32_t, CLOSE_ARGS> args{fd};
    stream_.record(sys::op::CLOSE, args);
}

void Sys::call(std::uint32_t op, std::span<const std::uint32_t> args, std::span<const std::uint8_t> bytes) noexcept {
    ensure((op & sys::op::PROGRAM) != 0, Errno::einval, op);
    stream_.record(op, args, {}, bytes);
}

Result<std::uint32_t> Sys::hostcall(std::uint32_t op, std::span<const std::uint32_t> args, std::span<const std::uint8_t> bytes) noexcept {
    ensure((op & sys::op::PROGRAM) != 0, Errno::einval, op);
    std::optional<std::uint32_t> at = stream_.past(op, args, bytes);
    if (!at) return make_error(ErrorClass::fault, Errno::enospc);
    std::int32_t res = xpute::hostcall(*at);
    if (res < 0) return make_error(ErrorClass::fault, errno_of(res).value_or(Errno::eproto));
    return static_cast<std::uint32_t>(res);
}

bool Sys::try_call(std::uint32_t op, std::span<const std::uint32_t> args, std::span<const std::uint8_t> bytes, std::uint32_t reserve) noexcept {
    if (!stream_.fits(record_bytes(args.size(), bytes.size()), reserve)) return false;
    call(op, args, bytes);
    return true;
}

void Sys::abort(Errno errnum, const std::source_location &at, std::uint64_t a, std::uint64_t b) noexcept {
    stream_.publish();
    std::array<std::uint32_t, ABORT_ARGS> args{static_cast<std::uint32_t>(errnum), at.line(), at.column()};
    for (auto [i, v] : {std::pair{sys::abort::A, a}, std::pair{sys::abort::B, b}}) {
        args[i] = static_cast<std::uint32_t>(v);
        args[i + 1] = static_cast<std::uint32_t>(v >> 32);
    }
    stream_.record(sys::op::ABORT, args, {}, bytes_of(at.file_name()));
    stream_.publish();
}

} // namespace xpute
