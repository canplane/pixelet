// cpp/xpute/core/ipc/door.test.cpp

#include "door.hpp"

#include <vector>

#include "xpute/kit/test.hpp"
#include "xpute/kit/wire/xtp/cursor.hpp"

using namespace xpute;

namespace {

constexpr std::uint32_t SQ = 8;
constexpr std::uint32_t CQ = 256;
constexpr std::uint32_t SLOT = 64;
constexpr std::uint32_t MOST_FRAMES = 64;
// A command of the program's own, past xpute's majors.
constexpr std::uint32_t PROGRAM = frame::PROGRAM_MAJOR << 8 | 1;

// A program that answers its own commands with `apply`, and the rest with 0.
template <class Apply> auto program(Apply apply) {
    return [apply](const Submission &sub) mutable {
        if (const Program *p = std::get_if<Program>(&sub)) return apply(p->cmd, p->pkt);
        return Outcome{};
    };
}

struct Fixture {
    std::vector<std::uint64_t> buf;
    Door door;
};

Fixture door(std::uint32_t slot = SLOT) {
    std::uint32_t sq_desc = 8, cq_desc = sq_desc + ring_bytes(SQ), sq_slot = cq_desc + ring_bytes(CQ), cq_slot = sq_slot + slots_bytes(SQ, slot);
    std::vector<std::uint64_t> buf((cq_slot + slots_bytes(CQ, slot)) / 8 + 1);
    Memory mem(reinterpret_cast<std::uintptr_t>(buf.data()), buf.size() * 8);
    Door d(Ring::init(mem, sq_desc, SQ, sq_slot, slot), Ring::init(mem, cq_desc, CQ, cq_slot, slot), MOST_FRAMES);
    return Fixture{std::move(buf), std::move(d)};
}

} // namespace

XPUTE_TEST(door_command_needs_reply_room) {
    Fixture f = door();
    const Door &d = f.door;
    for (std::uint32_t k = 0; k < CQ - MOST_FRAMES; k++) d.post(0, 2, 0, 0);
    XPUTE_CHECK(d.sq().push(0, PROGRAM, frame::ACKREQ, 0) == Errno::ok);
    d.receive(program([&](std::uint32_t, std::span<const std::uint8_t>) {
        for (std::uint32_t k = 0; k < MOST_FRAMES; k++) d.post(0, 2, 0, 0);
        return Outcome{.val = 7};
    }));
    XPUTE_CHECK_EQ(d.pending(), true, "it waits for the next turn rather than overrunning this one");
}

XPUTE_TEST(door_event_pkt_errno) {
    Fixture f = door();
    const Door &d = f.door;
    d.send_with(7, 9, 0, 1, [](xtp::PacketWriter &w) { w.u32(41); });
    std::optional<Frame> got = d.cq().peek();
    XPUTE_CHECK(got && got->cmd == 9 && got->tag == 7 && (got->flags & frame::RES) == 0);
    XPUTE_CHECK(*xtp::TreeReader::make(*d.cq().pkt(*got))->read_branch()->at(0)->get_number() == 41.0);
    d.cq().advance();

    XPUTE_CHECK(d.sq().push(5, PROGRAM, 0, 0) == Errno::ok);
    // The submission ring lies at 8.
    std::uint32_t entry = 8 + 4 * d.sq().entry_at(0);
    Memory mem(reinterpret_cast<std::uintptr_t>(f.buf.data()), f.buf.size() * 8);
    mem.set_word(entry + 4 * frame::PKT, 4);
    bool applied = false;
    d.receive(program([&](std::uint32_t, std::span<const std::uint8_t>) { return applied = true, Outcome{}; }));
    XPUTE_CHECK(!applied);
    got = d.cq().peek();
    XPUTE_CHECK(got && got->tag == 5 && got->result == static_cast<std::int32_t>(Errno::ebadmsg));
}

XPUTE_TEST(door_ring_peaks) {
    Fixture f = door();
    const Door &d = f.door;
    // Three commands start before the reserve runs out, so five of eight wait.
    for (std::uint32_t k = 0; k < CQ - (MOST_FRAMES + 2) - 2; k++) d.post(0, 2, 0, 0);
    for (std::uint32_t k = 0; k < SQ; k++) XPUTE_CHECK(d.sq().push(0, PROGRAM, frame::ACKREQ, 0) == Errno::ok);
    XPUTE_CHECK(d.receive(program([](std::uint32_t, std::span<const std::uint8_t>) { return Outcome{}; })) == 3);
    XPUTE_CHECK((d.take_stat() == DoorStat{.cq_peak = CQ - MOST_FRAMES - 1, .sq_peak = SQ, .left_peak = SQ - 3}));
    XPUTE_CHECK_EQ(d.take_stat() == DoorStat{}, true, "taking it starts the next from zero");

    while (d.cq().peek()) d.cq().advance();
    XPUTE_CHECK(d.receive(program([](std::uint32_t, std::span<const std::uint8_t>) { return Outcome{}; })) == SQ - 3);
    XPUTE_CHECK((d.take_stat() == DoorStat{.cq_peak = SQ - 3, .sq_peak = SQ - 3, .left_peak = 0}));
}

XPUTE_TEST(door_program_command_handed_over) {
    Fixture f = door();
    const Door &d = f.door;
    std::uint32_t tag = 6;
    XPUTE_CHECK(d.sq().push(tag, PROGRAM, frame::ACKREQ, 0) == Errno::ok);
    std::uint32_t got = 0xffff;
    d.receive(program([&](std::uint32_t cmd, std::span<const std::uint8_t>) { return got = cmd, Outcome{.val = 1}; }));
    XPUTE_CHECK(got == PROGRAM);
    std::optional<Frame> reply = d.cq().peek();
    XPUTE_CHECK(reply && reply->tag == tag && reply->result == 1);
}

XPUTE_TEST(door_xpute_number_unknown_refused) {
    Fixture f = door();
    const Door &d = f.door;
    XPUTE_CHECK(d.sq().push(5, 0x0f01, 0, 0) == Errno::ok);
    bool applied = false;
    d.receive(program([&](std::uint32_t, std::span<const std::uint8_t>) { return applied = true, Outcome{}; }));
    XPUTE_CHECK(!applied);
    std::optional<Frame> got = d.cq().peek();
    XPUTE_CHECK(got && got->tag == 5 && got->result == static_cast<std::int32_t>(Errno::enosys));
}

XPUTE_TEST(door_open_and_close_ask_for_completion) {
    Fixture f = door(256);
    const Door &d = f.door;
    d.openat(5, 1, 0, "a");
    d.close(5);
    for (abi::fs::Command cmd : {abi::fs::Command::FS_OPENAT, abi::fs::Command::FS_CLOSE}) {
        std::optional<Frame> got = d.cq().peek();
        // Tagged with its descriptor.
        XPUTE_CHECK(got && got->tag == 5 && got->cmd == static_cast<std::uint32_t>(cmd) && (got->flags & frame::ACKREQ) != 0);
        d.cq().advance();
    }
}

XPUTE_TEST(door_completions_handed_over) {
    Fixture f = door();
    const Door &d = f.door;
    XPUTE_CHECK(d.sq().push(3, static_cast<std::uint32_t>(abi::fs::Command::FS_OPENAT), frame::RES, 9) == Errno::ok);
    XPUTE_CHECK(d.sq().push(3, static_cast<std::uint32_t>(abi::fs::Command::FS_CLOSE), frame::RES, -5) == Errno::ok);
    std::optional<FsOpened> opened;
    std::optional<FsClosed> closed;
    d.receive([&](const Submission &sub) {
        if (const FsOpened *o = std::get_if<FsOpened>(&sub)) opened = *o;
        if (const FsClosed *c = std::get_if<FsClosed>(&sub)) closed = *c;
        return Outcome{};
    });
    XPUTE_CHECK(opened && opened->fd == 3 && opened->res == 9);
    XPUTE_CHECK(closed && closed->fd == 3 && closed->res == -5);
    XPUTE_CHECK_EQ(d.cq().peek().has_value(), false, "a completion is answered by nothing");
}
