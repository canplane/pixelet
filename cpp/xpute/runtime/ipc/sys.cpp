// cpp/xpute/runtime/ipc/sys.cpp

#include "sys.hpp"

#include <array>
#include <cstring>

#include "../../core/status/bug.hpp"

namespace xpute {

static std::span<const std::uint8_t> bytes_of(std::string_view s) noexcept {
    return {reinterpret_cast<const std::uint8_t *>(s.data()), s.size()};
}

void Sys::openat(Fd fd, std::uint32_t root, std::uint32_t flags, std::string_view path) noexcept {
    const std::array<std::uint32_t, 3> args{fd, root, flags};
    stream_.record(sys::op::OPENAT, args, {}, bytes_of(path));
}

void Sys::read(Fd fd, std::span<std::uint8_t> dst) noexcept {
    const std::array<std::uint32_t, 3> args{fd, mem_.off_of(dst), static_cast<std::uint32_t>(dst.size())};
    stream_.record(sys::op::READ, args);
}

void Sys::write(Fd fd, std::vector<std::uint8_t> bytes) noexcept {
    const std::array<std::uint32_t, 3> args{fd, mem_.off_of(std::span<const std::uint8_t>(bytes)), static_cast<std::uint32_t>(bytes.size())};
    stream_.record(sys::op::WRITE, args);
    held_.push_back(std::move(bytes));
}

void Sys::close(Fd fd) noexcept {
    const std::array<std::uint32_t, 1> args{fd};
    stream_.record(sys::op::CLOSE, args);
}

void Sys::call(std::uint32_t op, std::span<const std::uint32_t> args, std::span<const std::uint8_t> bytes) noexcept {
    ensure((op & sys::op::PROGRAM) != 0, Errno::einval, op);
    stream_.record(op, args, {}, bytes);
}

void Sys::abort(Errno code, const std::source_location &at, std::uint64_t a, std::uint64_t b) noexcept {
    stream_.publish();
    const std::array<std::uint32_t, 7> args{static_cast<std::uint32_t>(code), at.line(), at.column(), static_cast<std::uint32_t>(a),
                                            static_cast<std::uint32_t>(a >> 32), static_cast<std::uint32_t>(b), static_cast<std::uint32_t>(b >> 32)};
    stream_.record(sys::op::ABORT, args, {}, bytes_of(at.file_name()));
    stream_.publish();
}

} // namespace xpute
