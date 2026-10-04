// cpp/xpute/runtime/ipc/door.test.cpp

#include "door.hpp"

#include <vector>

#include "../../core/test.hpp"
#include "../../core/wire/xtp/cursor.hpp"

using namespace xpute;

namespace {

constexpr std::uint32_t SQ = 8;
constexpr std::uint32_t CQ = 256;
constexpr std::uint32_t SLOT = 64;
constexpr std::uint32_t MOST_SIGNALS = 64;
constexpr std::uint32_t MOST_COMPLETIONS = MOST_SIGNALS + 1;

struct Fixture {
    std::vector<std::uint64_t> buf;
    Door door;
};

Fixture door() {
    std::uint32_t sq_desc = 8, cq_desc = sq_desc + Ring::bytes(SQ), sq_slot = cq_desc + Ring::bytes(CQ), cq_slot = sq_slot + Ring::slots_bytes(SQ, SLOT);
    std::vector<std::uint64_t> buf((cq_slot + Ring::slots_bytes(CQ, SLOT)) / 8 + 1);
    Memory mem(reinterpret_cast<std::uintptr_t>(buf.data()), buf.size() * 8);
    Door d(Ring::init(mem, sq_desc, SQ, sq_slot, SLOT), Ring::init(mem, cq_desc, CQ, cq_slot, SLOT), MOST_COMPLETIONS);
    return Fixture{std::move(buf), d};
}

} // namespace

XPUTE_TEST(door_a_turn_applies_while_a_command_s_signals_and_reply_fit_and_leaves_the_rest_for_the_next_in_order) {
    Fixture f = door();
    const Door &d = f.door;
    for (std::uint32_t k = 0; k < SQ; k++) XPUTE_CHECK(d.submission().push(k, 1, frame::ACKREQ, 0) == Errno::ok);
    std::uint32_t answered = 0, turns = 0;
    for (;;) {
        // Every command raises as much as one may: a turn that starts one never runs
        // out of room for it.
        d.drain([&](std::uint32_t, std::span<const std::uint8_t>) {
            for (std::uint32_t k = 0; k < MOST_SIGNALS; k++) d.post(0, 2, 0, 0);
            return Applied{.value = 7};
        });
        turns++;
        const Ring &cq = d.completion();
        while (std::optional<Ring::Message> m = cq.peek()) {
            if (m->flags & frame::RES) {
                XPUTE_CHECK_EQ(m->tag == answered && m->result == 7, true, "replies come in the order sent");
                answered++;
            }
            cq.advance();
        }
        if (!d.pending()) break;
    }
    std::uint32_t per_turn = CQ / MOST_COMPLETIONS;
    XPUTE_CHECK_EQ(answered, SQ, "every submission is answered once");
    XPUTE_CHECK_EQ(turns, (SQ + per_turn - 1) / per_turn, "a turn applies as many as the ring has room to answer");
}

XPUTE_TEST(door_a_command_is_not_started_with_room_for_its_signals_but_not_its_reply) {
    Fixture f = door();
    const Door &d = f.door;
    for (std::uint32_t k = 0; k < CQ - MOST_SIGNALS; k++) d.post(0, 2, 0, 0);
    XPUTE_CHECK(d.submission().push(0, 1, frame::ACKREQ, 0) == Errno::ok);
    d.drain([&](std::uint32_t, std::span<const std::uint8_t>) {
        for (std::uint32_t k = 0; k < MOST_SIGNALS; k++) d.post(0, 2, 0, 0);
        return Applied{.value = 7};
    });
    XPUTE_CHECK_EQ(d.pending(), true, "it waits for the next turn rather than overrunning this one");
}

XPUTE_TEST(door_a_signal_s_packet_is_written_in_place_and_a_packet_that_does_not_read_is_answered_with_its_errno) {
    Fixture f = door();
    const Door &d = f.door;
    d.raise(9, 1, [](xtp::PacketWriter &w) { w.u32(41); });
    std::optional<Ring::Message> m = d.completion().peek();
    XPUTE_CHECK(m && m->cmd == 9 && (m->flags & frame::RES) == 0);
    XPUTE_CHECK(*xtp::TreeReader::make(*d.completion().packet(*m))->read_branch()->at(0)->get_number() == 41.0);
    d.completion().advance();

    XPUTE_CHECK(d.submission().push(5, 1, 0, 0) == Errno::ok);
    std::uint32_t frame_at = 8 + 4 * (ring::HEADER_WORDS + 0 * frame::WORDS);
    Memory mem(reinterpret_cast<std::uintptr_t>(f.buf.data()), f.buf.size() * 8);
    mem.set_word(frame_at + 4 * frame::PACKET, 4);
    bool applied = false;
    d.drain([&](std::uint32_t, std::span<const std::uint8_t>) { return applied = true, Applied{}; });
    XPUTE_CHECK(!applied);
    m = d.completion().peek();
    XPUTE_CHECK(m && m->tag == 5 && m->result == static_cast<std::int32_t>(Errno::ebadmsg));
}
