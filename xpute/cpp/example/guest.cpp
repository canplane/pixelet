// cpp/example/guest.cpp

// The smallest guest in C++, the same as rust/example and driven by the same
// host (ts/example/sum.test.ts). It answers `SUM` with the sum of its two
// numbers and one the host holds, which it asks for by a hostcall answered
// now, and raises `SUMMED` as it does; and `COUNT` with a task that reads a
// file whole and raises `COUNTED` with its size a few turns later. Its commands
// are its table's (spec/example/cmd.json), generated for both ends. A module
// only (cpp/Makefile `wasm`).

#include <cstdint>

#include "xpute/guest/program.hpp"
#include "cmd.spec.hpp"

using namespace xpute;
using namespace example;

namespace {

Result<std::int32_t> sum(Turn &cx, Sum p) {
    std::uint32_t total = p.a + p.b;
    cx.raise(SummedEvent{.total = total});
    Result<std::uint32_t> held = cx.call_now(static_cast<std::uint32_t>(example::Hostcall::ASK));
    if (!held) return held.error();
    return static_cast<std::int32_t>(total + *held);
}

Task<> count_bytes(Read file) {
    Result<Bytes> bytes = co_await file;
    std::int32_t res = bytes ? static_cast<std::int32_t>(bytes->size()) : static_cast<std::int32_t>(bytes.error().errnum);
    raise(CountedEvent{.res = res});
}

// The read holds its own copy of the path, which the packet's lasts only as
// long as the handler.
Result<std::int32_t> count(Turn &cx, Count p) {
    Result<void> spawned = cx.spawn(count_bytes(read(static_cast<std::uint32_t>(Root::FILES), p.path)));
    if (!spawned) return spawned.error();
    return 0;
}

} // namespace

XPUTE_PROGRAM(on<Command::SUM, sum>, on<Command::COUNT, count>)
