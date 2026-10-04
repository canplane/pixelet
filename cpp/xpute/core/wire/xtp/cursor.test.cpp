// cpp/xpute/core/wire/xtp/cursor.test.cpp

#include <array>
#include <charconv>
#include <cmath>
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

std::vector<std::uint8_t> bytes(const std::string &hex_text) {
    std::vector<std::uint8_t> out(hex_text.size() / 2);
    if (!hex_decode(hex_text, out).ok()) test::fail(__FILE__, __LINE__, "not hex");
    return out;
}

// The shortest digits that read back, as JavaScript prints.
std::string js_number(double x) {
    if (std::isnan(x)) return "NaN";
    char buf[64];
    auto r = std::to_chars(buf, buf + sizeof buf, x);
    return std::string(buf, r.ptr);
}

std::string errno_of(const Error &e) {
    return "errno:" + std::to_string(static_cast<int>(e.code));
}

template <class T> std::string elements(const Array &a) {
    Elements<T> e = a.elements<T>();
    std::string out;
    for (std::uint32_t i = 0; i < e.size(); i++) {
        if (i) out += ' ';
        if constexpr (std::is_floating_point_v<T>) out += js_number(e[i]);
        else out += std::to_string(e[i]);
    }
    return out;
}

// As the TypeScript prints it, so the traces compare.
std::string show(const Value &v) {
    return std::visit(
        [](const auto &x) -> std::string {
            using T = std::decay_t<decltype(x)>;
            if constexpr (std::is_same_v<T, Null>) return "null";
            else if constexpr (std::is_same_v<T, BranchCursor>) return "branch";
            else if constexpr (std::is_same_v<T, bool>) return std::string("boolean:") + (x ? "true" : "false");
            else if constexpr (std::is_same_v<T, std::uint64_t> || std::is_same_v<T, std::int64_t>) return "bigint:" + std::to_string(x);
            else if constexpr (std::is_floating_point_v<T>) return "number:" + js_number(x);
            else if constexpr (std::is_same_v<T, std::string_view>) return "string:" + hex({reinterpret_cast<const std::uint8_t *>(x.data()), x.size()});
            else if constexpr (std::is_same_v<T, Strs>) {
                std::string out = "[";
                for (std::uint32_t i = 0; i < x.len; i++) {
                    std::string_view s = x[i];
                    out += (i ? ",string:" : "string:") + hex({reinterpret_cast<const std::uint8_t *>(s.data()), s.size()});
                }
                return out + "]";
            } else if constexpr (std::is_same_v<T, Bitset>) {
                std::string out = "Uint8Array(";
                for (std::uint32_t i = 0; i < x.len; i++) out += (i ? " " : "") + std::to_string(x[i] ? 1 : 0);
                return out + ")";
            } else if constexpr (std::is_same_v<T, Array>) {
                switch (x.type) {
                case SequenceType::u8_array:
                    return "Uint8Array(" + elements<std::uint8_t>(x) + ")";
                case SequenceType::i8_array:
                    return "Int8Array(" + elements<std::int8_t>(x) + ")";
                case SequenceType::u16_array:
                    return "Uint16Array(" + elements<std::uint16_t>(x) + ")";
                case SequenceType::i16_array:
                    return "Int16Array(" + elements<std::int16_t>(x) + ")";
                case SequenceType::u32_array:
                    return "Uint32Array(" + elements<std::uint32_t>(x) + ")";
                case SequenceType::i32_array:
                    return "Int32Array(" + elements<std::int32_t>(x) + ")";
                case SequenceType::u64_array:
                    return "BigUint64Array(" + elements<std::uint64_t>(x) + ")";
                case SequenceType::i64_array:
                    return "BigInt64Array(" + elements<std::int64_t>(x) + ")";
                case SequenceType::f32_array:
                    return "Float32Array(" + elements<float>(x) + ")";
                case SequenceType::f64_array:
                    return "Float64Array(" + elements<double>(x) + ")";
                default:
                    test::fail(__FILE__, __LINE__, "a bitset or a string reads as its own value");
                }
            } else return "number:" + std::to_string(x);
        },
        v);
}

Result<std::string> deep(const Cursor &c) {
    if (!c.is_branch()) {
        Result<Value> v = c.get();
        if (!v) return v.error();
        return show(*v);
    }
    Result<BranchCursor> b = c.as_branch();
    if (!b) return b.error();
    std::string out = "[";
    for (std::uint32_t i = 0; i < b->len(); i++) {
        Result<Cursor> child = b->at(i);
        if (!child) return child.error();
        Result<std::string> s = deep(*child);
        if (!s) return s;
        out += (i ? "," : "") + *s;
    }
    return out + "]";
}

std::string read_deep(std::span<const std::uint8_t> pkt) {
    Result<TreeReader> r = TreeReader::make(pkt);
    if (!r) return errno_of(r.error());
    Result<std::string> s = deep(r->read());
    return s ? "ok" : errno_of(s.error());
}

} // namespace

XPUTE_TEST(xtp_the_cursor_reads_what_the_other_languages_read) {
    Golden v = Golden::load("spec/xpute/golden/wire/xtp/cursor.tsv");
    v.each("packets", [&v](const std::string &k) {
        std::vector<std::uint8_t> pkt = bytes(v.s(k + ".packet"));
        Result<TreeReader> reader = TreeReader::make(pkt);
        XPUTE_CHECK(reader.ok());
        Cursor root = reader->read();
        XPUTE_CHECK_EQ(std::uint32_t{root.type()}, v.u32(k + ".type"), (k + ".type").c_str());
        XPUTE_CHECK_EQ(deep(root).value(), v.s(k + ".deep"), (k + ".deep").c_str());
        if (!root.is_branch()) return;
        BranchCursor b = root.as_branch().value();
        std::size_t n = v.has(k + ".shallow.0") ? v.len(k + ".shallow") : 0;
        XPUTE_CHECK_EQ(std::size_t{b.len()}, n, (k + ".shallow").c_str());
        for (std::uint32_t j = 0; j < b.len(); j++) {
            Cursor c = b.at(j).value();
            std::string got = std::to_string(c.type()) + ":" + (c.is_branch() ? "branch" : show(c.get().value()));
            XPUTE_CHECK_EQ(got, v.s(k + ".shallow." + std::to_string(j)), (k + ".shallow." + std::to_string(j)).c_str());
        }
    });

    const std::string &good = v.s("packets.0.packet");
    auto refused = [](const std::string &h) { return read_deep(bytes(h)); };
    XPUTE_CHECK_EQ(refused(good.substr(0, 20)), v.s("malformed.truncated_header"), "a header cut short");
    XPUTE_CHECK_EQ(refused("00" + good.substr(2)), v.s("malformed.bad_magic"), "a magic that is not");
    XPUTE_CHECK_EQ(refused(good.substr(0, 40)), v.s("malformed.truncated_payload"), "a payload cut short");
    v.each("huge", [&](const std::string &k) { XPUTE_CHECK_EQ(refused(v.s(k + ".packet")), v.s(k + ".read"), k.c_str()); });

    const std::uint32_t aligns[] = {0x7ffffff9u, 0xfffffff8u, 0xfffffff9u};
    for (std::size_t j = 0; j < 3; j++) {
        Result<std::uint32_t> a = align(aligns[j], 8);
        XPUTE_CHECK_EQ(a ? "ok:" + std::to_string(*a) : errno_of(a.error()), v.s("align." + std::to_string(j)), "align");
    }

    auto at_cap = [](std::string_view s, std::size_t cap) {
        std::vector<std::uint8_t> buf(cap);
        PacketWriter w(buf);
        w.str(s);
        Result<std::uint32_t> n = w.finish();
        return n ? hex(std::span(buf).first(*n)) : errno_of(n.error());
    };
    const std::size_t caps[] = {24, 32, 40, 48};
    for (std::size_t j = 0; j < 4; j++) XPUTE_CHECK_EQ(at_cap("abcdefgh", caps[j]), v.s("caps." + std::to_string(j)), "caps");
    for (std::size_t j = 0; j < 3; j++) XPUTE_CHECK_EQ(at_cap("한글한", caps[j]), v.s("caps_utf8." + std::to_string(j)), "caps_utf8");
}

XPUTE_TEST(xtp_a_packet_reads_back_what_was_written) {
    std::array<std::uint8_t, 256> buf{};
    PacketWriter w(buf);
    const std::array<std::int32_t, 2> ints{7, 8};
    w.branch(6, [&](PacketWriter &b) {
        b.i32(42).str("hello").boolean(true).nil();
        b.branch(2, [](PacketWriter &b) { b.i32(1).i64(-2); });
        b.i32_array(ints);
    });
    std::span<const std::uint8_t> pkt = std::span(buf).first(w.finish().value());
    BranchCursor root = TreeReader::make(pkt)->read_branch().value();
    XPUTE_CHECK_EQ(root.len(), 6u, "the root's children");
    XPUTE_CHECK_EQ(deep(TreeReader::make(pkt)->read()).value(), std::string("[number:42,string:68656c6c6f,boolean:true,null,[number:1,bigint:-2],Int32Array(7 8)]"), "deep");
    XPUTE_CHECK(std::holds_alternative<BranchCursor>(root.at(4)->get().value()));
    XPUTE_CHECK(root.at_branch(4).value() == std::get<BranchCursor>(root.at(4)->get().value()));
    XPUTE_CHECK_EQ(root.at(5)->get_array<std::int32_t>().value()[1], 8, "an element");
    XPUTE_CHECK(!root.at(5)->get_array<std::uint32_t>().ok());
    XPUTE_CHECK(!root.at_branch(4)->at(1)->get_number().ok());
    XPUTE_CHECK_EQ(root.at_branch(4)->at(1)->get_i64().value(), -2, "a 64-bit integer");
    XPUTE_CHECK_EQ(root.at(6).error().code, Errno::efault, "past the last child");

    std::array<std::uint8_t, 64> one{};
    PacketWriter n(one);
    n.branch(1, [](PacketWriter &b) { b.u32(std::nullopt); });
    BranchCursor nulls = TreeReader::make(std::span(one).first(n.finish().value()))->read_branch().value();
    Cursor absent = nulls.at(0).value();
    XPUTE_CHECK(absent.is_null() && absent.type() == type_of(ScalarType::u32));
    XPUTE_CHECK(!absent.opt());
    XPUTE_CHECK_EQ(absent.as_branch().error().code, Errno::efault, "an absent node is no branch");
}

XPUTE_TEST(xtp_a_packet_reads_wherever_it_lies) {
    std::array<std::uint8_t, 64> buf{};
    PacketWriter w{std::span(buf).subspan(1)};
    const std::array<double, 2> xs{0.5, -4.0};
    w.f64_array(xs);
    std::span<const std::uint8_t> pkt = std::span(buf).subspan(1, w.finish().value());
    Elements<double> e = TreeReader::make(pkt)->read().get_array<double>().value();
    XPUTE_CHECK(e.size() == 2 && e[0] == 0.5 && e[1] == -4.0);
}
