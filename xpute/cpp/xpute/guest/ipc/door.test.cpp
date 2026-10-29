// cpp/xpute/guest/ipc/door.test.cpp

#include "door.hpp"

#include <vector>

#include "xpute/kit/test.hpp"
#include "xpute/kit/wire/xtp/cursor.hpp"

using namespace xpute;

namespace {

constexpr std::uint32_t SQ = 8;
constexpr std::uint32_t CQ = 256;
constexpr std::uint32_t SLOT = 64;
constexpr std::uint32_t MOST_SIGNALS = 64;

struct Fixture {
    std::vector<std::uint64_t> buf;
    Door door;
};

Fixture door() {
    std::uint32_t sq_desc = 8, cq_desc = sq_desc + ring_bytes(SQ), sq_slot = cq_desc + ring_bytes(CQ), cq_slot = sq_slot + slots_bytes(SQ, SLOT);
    std::vector<std::uint64_t> buf((cq_slot + slots_bytes(CQ, SLOT)) / 8 + 1);
    Memory mem(reinterpret_cast<std::uintptr_t>(buf.data()), buf.size() * 8);
    Door d(Ring::init(mem, sq_desc, SQ, sq_slot, SLOT), Ring::init(mem, cq_desc, CQ, cq_slot, SLOT), MOST_SIGNALS);
    return Fixture{std::move(buf), d};
}

} // namespace

XPUTE_TEST(door_command_needs_reply_room) {
    Fixture f = door();
    const Door &d = f.door;
    for (std::uint32_t k = 0; k < CQ - MOST_SIGNALS; k++) d.post(0, 2, 0, 0);
    XPUTE_CHECK(d.sq().push(0, 1, frame::ACKREQ, 0) == Errno::ok);
    d.drain([&](std::uint32_t, std::span<const std::uint8_t>) {
        for (std::uint32_t k = 0; k < MOST_SIGNALS; k++) d.post(0, 2, 0, 0);
        return Applied{.val = 7};
    });
    XPUTE_CHECK_EQ(d.pending(), true, "it waits for the next turn rather than overrunning this one");
}

XPUTE_TEST(door_signal_pkt_errno) {
    Fixture f = door();
    const Door &d = f.door;
    d.raise(9, 1, [](xtp::PacketWriter &w) { w.u32(41); });
    std::optional<Frame> got = d.cq().peek();
    XPUTE_CHECK(got && got->cmd == 9 && (got->flags & frame::RES) == 0);
    XPUTE_CHECK(*xtp::TreeReader::make(*d.cq().pkt(*got))->read_branch()->at(0)->get_number() == 41.0);
    d.cq().advance();

    XPUTE_CHECK(d.sq().push(5, 1, 0, 0) == Errno::ok);
    // The submission ring lies at 8.
    std::uint32_t entry = 8 + 4 * d.sq().entry_at(0);
    Memory mem(reinterpret_cast<std::uintptr_t>(f.buf.data()), f.buf.size() * 8);
    mem.set_word(entry + 4 * frame::PKT, 4);
    bool applied = false;
    d.drain([&](std::uint32_t, std::span<const std::uint8_t>) { return applied = true, Applied{}; });
    XPUTE_CHECK(!applied);
    got = d.cq().peek();
    XPUTE_CHECK(got && got->tag == 5 && got->result == static_cast<std::int32_t>(Errno::ebadmsg));
}

XPUTE_TEST(door_ring_peaks) {
    Fixture f = door();
    const Door &d = f.door;
    // Three commands start before the reserve runs out, so five of eight wait.
    for (std::uint32_t k = 0; k < CQ - (MOST_SIGNALS + 1) - 2; k++) d.post(0, 2, 0, 0);
    for (std::uint32_t k = 0; k < SQ; k++) XPUTE_CHECK(d.sq().push(0, 1, frame::ACKREQ, 0) == Errno::ok);
    XPUTE_CHECK(d.drain([](std::uint32_t, std::span<const std::uint8_t>) { return Applied{}; }) == 3);
    XPUTE_CHECK((d.take_stat() == DoorStat{.cq_peak = CQ - MOST_SIGNALS, .sq_peak = SQ, .left_peak = SQ - 3}));
    XPUTE_CHECK_EQ(d.take_stat() == DoorStat{}, true, "taking it starts the next from zero");

    while (d.cq().peek()) d.cq().advance();
    XPUTE_CHECK(d.drain([](std::uint32_t, std::span<const std::uint8_t>) { return Applied{}; }) == SQ - 3);
    XPUTE_CHECK((d.take_stat() == DoorStat{.cq_peak = SQ - 3, .sq_peak = SQ - 3, .left_peak = 0}));
}
