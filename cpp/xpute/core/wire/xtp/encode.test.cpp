// cpp/xpute/core/wire/xtp/encode.test.cpp

#include <array>
#include <charconv>
#include <string>
#include <vector>

#include "../../codec/encoding.hpp"
#include "../../golden.hpp"
#include "../../test.hpp"
#include "cursor.hpp"
#include "encode.hpp"

using namespace xpute;
using namespace xpute::xtp;

namespace {

std::string hex(std::span<const std::uint8_t> bytes) {
    std::string out(hex_len(bytes.size()), '\0');
    hex_encode(bytes, out);
    return out;
}

// The packet `build` writes as its root, in hex, or the errno it was refused
// with.
template <class F> std::string encode(F &&build, std::size_t cap = 1024) {
    std::vector<std::uint8_t> buf(cap, 0xff);
    PacketWriter w(buf);
    build(w);
    Result<std::uint32_t> n = w.finish();
    if (!n) return "errno:" + std::to_string(static_cast<int>(n.error().code));
    return hex(std::span(buf).first(*n));
}

std::vector<std::uint8_t> bytes(const std::string &hex_text) {
    std::vector<std::uint8_t> out(hex_text.size() / 2);
    if (!hex_decode(hex_text, out).ok()) test::fail(__FILE__, __LINE__, "not hex");
    return out;
}

} // namespace

XPUTE_TEST(xtp_the_writer_writes_the_bytes_the_other_languages_write) {
    Golden v = Golden::load("spec/xpute/golden/wire/xtp/encode.tsv");
    auto check = [&v](const char *name, auto &&build) {
        std::string k;
        v.each("", [&](const std::string &j) {
            if (v.s(j + ".name") == name) k = j;
        });
        XPUTE_CHECK(!k.empty());
        XPUTE_CHECK_EQ(encode(build), v.s(k + ".packet"), name);
    };
    check("nil", [](PacketWriter &w) { w.nil(); });
    check("i32", [](PacketWriter &w) { w.i32(42); });
    check("f64", [](PacketWriter &w) { w.f64(1.5); });
    check("bigint", [](PacketWriter &w) { w.i64(-5); });
    check("u64", [](PacketWriter &w) { w.u64(0xffff'ffff'ffff'fffeu); });
    check("bool", [](PacketWriter &w) { w.boolean(true); });
    check("str", [](PacketWriter &w) { w.str("héllo"); });
    check("str empty", [](PacketWriter &w) { w.str(""); });
    const std::array<std::uint16_t, 3> u16s{1, 2, 3};
    check("typed u16", [&](PacketWriter &w) { w.u16_array(u16s); });
    const std::array<float, 2> f32s{0.5f, -2.0f};
    check("typed f32", [&](PacketWriter &w) { w.f32_array(f32s); });
    const std::array<std::uint8_t, 9> bits{1, 0, 1, 1, 0, 0, 0, 0, 1};
    check("bitset", [&](PacketWriter &w) { w.bitset(bits); });
    const std::array<std::string_view, 3> strs{"a", "héllo", ""};
    check("strs", [&](PacketWriter &w) { w.strs(strs); });
    const std::array<std::int32_t, 1> seven{7};
    check("branch mixed", [&](PacketWriter &w) {
        w.branch(7, [&](PacketWriter &b) {
            b.i32(42).str("hello").boolean(true).nil().i64(std::int64_t{1} << 40);
            b.branch(2, [](PacketWriter &b) { b.i32(1).i32(2); });
            b.i32_array(seven);
        });
    });
    check("branch null child", [](PacketWriter &w) {
        w.branch(4, [](PacketWriter &b) { b.u8(std::nullopt).str(std::nullopt).no_branch().u32(9); });
    });
    check("branch empty", [](PacketWriter &w) { w.branch(0, [](PacketWriter &) {}); });
    check("typed null root", [](PacketWriter &w) { w.u32(std::nullopt); });
    std::vector<std::uint8_t> inner = bytes(encode([](PacketWriter &w) { w.branch(2, [](PacketWriter &b) { b.i32(1).str("x"); }); }));
    check("graft", [&](PacketWriter &w) { w.graft(inner); });
}

XPUTE_TEST(xtp_a_packet_grafted_whole_is_that_packet) {
    std::string inner = encode([](PacketWriter &w) { w.branch(2, [](PacketWriter &b) { b.i32(1).str("x"); }); });
    std::vector<std::uint8_t> packet = bytes(inner);
    XPUTE_CHECK_EQ(encode([&packet](PacketWriter &w) { w.graft(packet); }), inner, "a graft of a whole packet");
}

XPUTE_TEST(xtp_a_string_written_in_place_is_the_string_written_from_bytes_and_reads_back_where_it_lies) {
    std::array<std::uint8_t, 128> buf;
    buf.fill(0xff);
    PacketWriter w(buf);
    w.branch(2, [](PacketWriter &b) {
        b.str_with([](std::span<char> room) -> std::optional<std::size_t> {
             std::string_view head = "héllo-";
             if (head.size() > room.size()) return std::nullopt;
             std::copy(head.begin(), head.end(), room.begin());
             auto r = std::to_chars(room.data() + head.size(), room.data() + room.size(), 42);
             if (r.ec != std::errc()) return std::nullopt;
             return static_cast<std::size_t>(r.ptr - room.data());
         }).u32(7);
    });
    Result<std::uint32_t> n = w.finish();
    XPUTE_CHECK(n.ok());
    std::span<const std::uint8_t> pkt = std::span(buf).first(*n);
    XPUTE_CHECK_EQ(hex(pkt), encode([](PacketWriter &w) { w.branch(2, [](PacketWriter &b) { b.str("héllo-42").u32(7); }); }), "a string written in place");
    Result<BranchCursor> branch = TreeReader::make(pkt)->read_branch();
    XPUTE_CHECK(branch.ok());
    XPUTE_CHECK(branch->at(0)->get_str_ref().value() == "héllo-42");
    XPUTE_CHECK(!branch->at(1)->get_str_ref().ok());

    std::array<std::uint8_t, 40> small{};
    PacketWriter tight(small);
    tight.branch(1, [](PacketWriter &b) {
        b.str_with([](std::span<char> room) -> std::optional<std::size_t> {
            std::string_view text = "far longer than the room that is left";
            if (text.size() > room.size()) return std::nullopt;
            return text.size();
        });
    });
    XPUTE_CHECK_EQ(tight.finish().error().code, Errno::eoverflow, "a string that does not fit spoils the packet");
}

XPUTE_TEST(xtp_what_does_not_fit_spoils_the_packet) {
    std::string long_str = encode([](PacketWriter &w) { w.branch(1, [](PacketWriter &b) { b.str("longer than what is left"); }); }, 40);
    XPUTE_CHECK_EQ(long_str, std::string("errno:-75"), "a child that does not fit");
    std::string long_branch = encode(
        [](PacketWriter &w) { w.branch(1, [](PacketWriter &b) { b.branch(3, [](PacketWriter &b) { b.u32(1).u32(2).str("past the room"); }); }); }, 48);
    XPUTE_CHECK_EQ(long_branch, std::string("errno:-75"), "a branch that does not fit");
    XPUTE_CHECK_EQ(encode([](PacketWriter &w) { w.nil(); }, 8), std::string("errno:-22"), "a buffer shorter than a header");
    std::vector<std::uint8_t> bad(16, 0);
    XPUTE_CHECK_EQ(encode([&bad](PacketWriter &w) { w.graft(bad); }), std::string("errno:-74"), "a graft of what is no packet");
}

// A branch ends on a word, so a byte after it lies where the tree encoder
// lays it rather than right after the branch's last child.
XPUTE_TEST(xtp_a_node_after_a_branch_starts_on_a_word) {
    std::vector<std::uint8_t> pkt = bytes(encode([](PacketWriter &w) {
        w.branch(2, [](PacketWriter &b) {
            b.branch(1, [](PacketWriter &b) { b.u8(1); });
            b.u8(2);
        });
    }));
    Result<BranchCursor> root = TreeReader::make(pkt)->read_branch();
    XPUTE_CHECK(root.ok());
    Result<Cursor> after = root->at(1);
    XPUTE_CHECK(after.ok() && after->get_number().value() == 2);
    // The inner branch at 16 + 24, its table one word, its byte at 56; the
    // next byte on the word after it.
    XPUTE_CHECK_EQ(std::get<0>(get_word(pkt, HDR_SZ + WORD_SZ * 2)) + root->base(), 64u, "the byte's place");
}
