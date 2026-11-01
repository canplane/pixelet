// cpp/example/cmd.spec.hpp
//
// GENERATED from spec/example/cmd.json
//
// The example's own commands (rust/example, ts/example/sum.test.ts): a program's
// table, from which its guest's readers and its host's writers are generated.

#pragma once

#include <array>
#include <cstdint>
#include <optional>
#include <span>
#include <string_view>

#include "xpute/kit/abi/cmd.hpp"
#include "xpute/kit/status/error.hpp"
#include "xpute/kit/wire/xtp/cursor.hpp"
#include "xpute/kit/wire/xtp/encode.hpp"
#include "xpute/guest/abi/number.hpp"
#include "xpute/guest/program.hpp"

namespace example {

using ::xpute::Result;
using ::xpute::abi::finite;
using ::xpute::abi::integer;
namespace xtp = ::xpute::xtp;

// The example's hostcalls of its own, each answered by the function its host's `open` is given.
enum class Hostcall : std::uint32_t {
    // The number the host holds, answered now.
    ASK = 1,
};

} // namespace example

template <> struct xpute::Members<example::Hostcall> {
    static constexpr std::array<example::Hostcall, 1> ALL{example::Hostcall::ASK};
};

namespace example {

// The roots the example's files are read below; what each means is its host's `Fs`.
enum class Root : std::uint32_t {
    // The files COUNT reads.
    FILES = 0,
};

} // namespace example

template <> struct xpute::Members<example::Root> {
    static constexpr std::array<example::Root, 1> ALL{example::Root::FILES};
};

namespace example {

// The example's commands.
enum class Command : std::uint32_t {
    // Two numbers, answered with their sum and the number the host holds.
    SUM = 0x1001,
    // Raised as SUM is answered: the two numbers' sum, before the host's is added.
    SUMMED = 0x1002,
    // A file's path, read whole by a task; answered at once, its count raised as COUNTED once read.
    COUNT = 0x1003,
    // Raised once COUNT's file is read: its bytes, or -errno.
    COUNTED = 0x1004,
};

} // namespace example

template <> struct xpute::Members<example::Command> {
    static constexpr std::array<example::Command, 4> ALL{example::Command::SUM, example::Command::SUMMED, example::Command::COUNT, example::Command::COUNTED};
};

namespace example {

// Command::SUM's packet.
struct Sum {
    std::uint32_t a{};
    std::uint32_t b{};

    static Result<Sum> read(std::span<const std::uint8_t> pkt) noexcept {
        Result<xtp::TreeReader> t = xtp::TreeReader::make(pkt);
        if (!t) return t.error();
        Result<xtp::BranchCursor> f = t->read_branch();
        if (!f) return f.error();
        Sum out;
        {
            Result<xtp::Cursor> c = f->at(0);
            if (!c) return c.error();
            auto v = c->get_number();
            if (!v) return v.error();
            auto n = integer<std::uint32_t>(*v);
            if (!n) return n.error();
            out.a = *n;
        }
        {
            Result<xtp::Cursor> c = f->at(1);
            if (!c) return c.error();
            auto v = c->get_number();
            if (!v) return v.error();
            auto n = integer<std::uint32_t>(*v);
            if (!n) return n.error();
            out.b = *n;
        }
        return out;
    }

    static Result<Sum> from_packet(::xpute::Packet pkt) noexcept {
        std::optional<std::span<const std::uint8_t>> b = pkt.bytes();
        if (!b) return ::xpute::marshal_error(::xpute::Errno::ebadmsg);
        return read(*b);
    }
};

// Command::SUMMED's event.
struct SummedEvent {
    std::uint32_t total{};

    // Its number, and the children it is written as.
    static constexpr std::uint32_t NUMBER = 4098;
    static constexpr std::uint32_t FIELDS = 1;

    void write(xtp::PacketWriter &w) const noexcept {
        w.u32(total);
    }
};

// Command::COUNT's packet.
struct Count {
    std::string_view path{};

    static Result<Count> read(std::span<const std::uint8_t> pkt) noexcept {
        Result<xtp::TreeReader> t = xtp::TreeReader::make(pkt);
        if (!t) return t.error();
        Result<xtp::BranchCursor> f = t->read_branch();
        if (!f) return f.error();
        Count out;
        {
            Result<xtp::Cursor> c = f->at(0);
            if (!c) return c.error();
            auto v = c->get_str_ref();
            if (!v) return v.error();
            out.path = *v;
        }
        return out;
    }

    static Result<Count> from_packet(::xpute::Packet pkt) noexcept {
        std::optional<std::span<const std::uint8_t>> b = pkt.bytes();
        if (!b) return ::xpute::marshal_error(::xpute::Errno::ebadmsg);
        return read(*b);
    }
};

// Command::COUNTED's event.
struct CountedEvent {
    std::int32_t res{};

    // Its number, and the children it is written as.
    static constexpr std::uint32_t NUMBER = 4100;
    static constexpr std::uint32_t FIELDS = 1;

    void write(xtp::PacketWriter &w) const noexcept {
        w.i32(res);
    }
};

} // namespace example
