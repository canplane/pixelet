// cpp/guest/ipc/ring.test.cpp

#include "ring.hpp"

#include <string>
#include <vector>

#include "../../kit/test.hpp"
#include "../../kit/wire/xtp/cursor.hpp"
#include "../../kit/wire/xtp/encode.hpp"

using namespace xpute;

namespace {

// A ring of `capacity` slots of `slot` bytes, its descriptors at 8 and its
// payloads past them, in a buffer of its own.
struct Fixture {
    std::vector<std::uint64_t> buf;
    Ring ring;
};

Fixture fixture(std::uint32_t capacity, std::uint32_t slot) {
    std::uint32_t slot_base = 8 + Ring::bytes(capacity);
    std::vector<std::uint64_t> buf((slot_base + Ring::slots_bytes(capacity, slot)) / 8 + 1);
    Memory mem(reinterpret_cast<std::uintptr_t>(buf.data()), buf.size() * 8);
    Ring r = Ring::init(mem, 8, capacity, slot_base, slot);
    return Fixture{std::move(buf), r};
}

// A packet of a branch of a u32 and a u64, written by `w`'s caller.
std::optional<std::uint32_t> pair(std::span<std::uint8_t> out, std::uint32_t n) {
    xtp::PacketWriter w(out);
    w.branch(2, [n](xtp::PacketWriter &b) { b.u32(n).u64(std::uint64_t{n} << 40); });
    Result<std::uint32_t> len = w.finish();
    return len ? std::optional(*len) : std::nullopt;
}

std::vector<std::uint8_t> packet_of(std::uint32_t n) {
    std::vector<std::uint8_t> out(256);
    out.resize(*pair(out, n));
    return out;
}

} // namespace

XPUTE_TEST(ring_messages_come_out_in_order_across_the_wrap_and_a_full_ring_refuses) {
    Fixture f = fixture(4, 64);
    const Ring &r = f.ring;
    for (std::uint32_t round = 0; round < 3; round++) {
        for (std::uint32_t k = 0; k < 4; k++) XPUTE_CHECK(r.push(round * 4 + k, 7, frame::ACKREQ, -static_cast<std::int32_t>(round * 4 + k)) == Errno::ok);
        XPUTE_CHECK_EQ(r.push(99, 0, 0, 0), Errno::eagain, "a full ring refuses");
        for (std::uint32_t k = 0; k < 4; k++) {
            std::optional<Ring::Message> m = r.peek();
            XPUTE_CHECK(m && m->tag == round * 4 + k && m->cmd == 7 && m->flags == frame::ACKREQ && m->result == -static_cast<std::int32_t>(round * 4 + k));
            XPUTE_CHECK(r.packet(*m)->empty());
            r.advance();
        }
        XPUTE_CHECK(!r.peek());
    }
    XPUTE_CHECK_EQ(r.head() == 12 && r.tail() == 12, true, "positions only grow");
}

XPUTE_TEST(ring_a_packet_written_in_place_reads_as_the_one_pushed_and_one_past_a_slot_is_refused_whatever_the_ring_holds) {
    Fixture f = fixture(8, 64);
    const Ring &r = f.ring;
    XPUTE_CHECK(r.push_in_place(1, 2, 0, 0, [](std::span<std::uint8_t> out) { return pair(out, 5); }) == Errno::ok);
    std::span<const std::uint8_t> got = *r.packet(*r.peek());
    std::vector<std::uint8_t> want = packet_of(5);
    XPUTE_CHECK((std::vector<std::uint8_t>(got.begin(), got.end()) == want));

    // The same packet is refused the same way with a message in the ring and
    // with none: a slot is a slot.
    std::string long_text(200, 'x');
    auto too_long = [&](std::span<std::uint8_t> out) -> std::optional<std::uint32_t> {
        xtp::PacketWriter w(out);
        w.branch(1, [&](xtp::PacketWriter &b) { b.str(long_text); });
        Result<std::uint32_t> n = w.finish();
        return n ? std::optional(*n) : std::nullopt;
    };
    std::uint32_t tail = r.tail();
    XPUTE_CHECK(r.push_in_place(1, 2, 0, 0, too_long) == Errno::emsgsize);
    XPUTE_CHECK_EQ(r.tail(), tail, "and nothing written");
    r.advance();
    XPUTE_CHECK_EQ(r.push_in_place(1, 2, 0, 0, too_long), Errno::emsgsize, "an empty ring answers the same");

    XPUTE_CHECK(r.push_in_place(3, 4, 0, -1, [](std::span<std::uint8_t> out) { return pair(out, 6); }) == Errno::ok);
    std::optional<Ring::Message> m = r.peek();
    XPUTE_CHECK(m->tag == 3 && m->cmd == 4 && m->result == -1);
    xtp::BranchCursor root = *xtp::TreeReader::make(*r.packet(*m))->read_branch();
    XPUTE_CHECK(*root.at(0)->get_number() == 6.0);
}

XPUTE_TEST(ring_one_payload_a_slot_so_the_payloads_run_out_with_the_slots_and_never_before_them) {
    Fixture f = fixture(8, 64);
    const Ring &r = f.ring;
    XPUTE_CHECK(packet_of(1).size() == 56);
    for (std::uint32_t n = 1; n <= 8; n++) XPUTE_CHECK(r.push(n, 0x0101, 0, 0, packet_of(n)) == Errno::ok);
    XPUTE_CHECK_EQ(r.push(9, 0x0101, 0, 0, packet_of(9)), Errno::eagain, "the ring fills before the payloads do");
    XPUTE_CHECK(r.len() == 8);
    for (std::uint32_t n = 1; n <= 8; n++) {
        std::optional<Ring::Message> m = r.peek();
        xtp::BranchCursor root = *xtp::TreeReader::make(*r.packet(*m))->read_branch();
        XPUTE_CHECK(m->tag == n && *root.at(0)->get_number() == n && *root.at(1)->get_u64() == std::uint64_t{n} << 40);
        r.advance();
    }
    XPUTE_CHECK_EQ(r.push(9, 0x0101, 0, 0, packet_of(9)), Errno::ok, "a slot freed is a payload freed");
    std::vector<std::uint8_t> big(72);
    XPUTE_CHECK(r.push(10, 0x0101, 0, 0, big) == Errno::emsgsize);
    Ring::Message outside{.tag = 0, .cmd = 0, .flags = 0, .result = 0, .packet_at = 4};
    XPUTE_CHECK_EQ(r.packet(outside).ok(), false, "outside a slot");
}
