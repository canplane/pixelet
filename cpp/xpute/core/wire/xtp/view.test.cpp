// cpp/xpute/core/wire/xtp/view.test.cpp

#include <array>
#include <string>
#include <vector>

#include "../../codec/encoding.hpp"
#include "../../golden.hpp"
#include "../../test.hpp"
#include "view.hpp"

using namespace xpute;
using namespace xpute::xtp;

namespace {

std::string hex(std::span<const std::uint8_t> bytes) {
    std::string out(hex_len(bytes.size()), '\0');
    hex_encode(bytes, out);
    return out;
}

// The packet the tree `build` makes, encoded from a buffer of `init`, in hex,
// or the errno it was refused with.
template <class F> std::string encoded(F &&build, std::uint32_t init = 1024, std::optional<std::uint32_t> max = std::nullopt) {
    TreeView<> t;
    build(t);
    auto pkt = encode<std::allocator<std::byte>>(t, TreeEncoderOptions{.init_cap = init, .max_cap = max});
    if (!pkt) return "errno:" + std::to_string(static_cast<int>(pkt.error().code));
    return hex(*pkt);
}

} // namespace

XPUTE_TEST(xtp_the_tree_encodes_to_the_bytes_the_other_languages_write) {
    Golden v = Golden::load("spec/xpute/golden/wire/xtp/encode.tsv");
    auto check = [&v](const char *name, auto &&build) {
        std::string k;
        v.each("", [&](const std::string &j) {
            if (v.s(j + ".name") == name) k = j;
        });
        XPUTE_CHECK(!k.empty());
        XPUTE_CHECK_EQ(encoded(build), v.s(k + ".packet"), name);
        // From a buffer that has to grow, the same bytes.
        XPUTE_CHECK_EQ(encoded(build, 16), v.s(k + ".packet"), name);
    };
    check("nil", [](TreeView<> &t) { t.nil(); });
    check("i32", [](TreeView<> &t) { (void)t.set({std::int32_t{42}}); });
    check("f64", [](TreeView<> &t) { (void)t.set({1.5}); });
    check("bigint", [](TreeView<> &t) { t.i64(-5); });
    check("u64", [](TreeView<> &t) { t.u64(0xffff'ffff'ffff'fffeu); });
    check("bool", [](TreeView<> &t) { (void)t.set({true}); });
    check("str", [](TreeView<> &t) { (void)t.set({std::string_view("héllo")}); });
    check("str empty", [](TreeView<> &t) { t.str(""); });
    static const std::array<std::uint16_t, 3> u16s{1, 2, 3};
    check("typed u16", [](TreeView<> &t) { t.u16_array(std::span<const std::uint16_t>(u16s)); });
    static const std::array<float, 2> f32s{0.5f, -2.0f};
    check("typed f32", [](TreeView<> &t) { t.f32_array(std::span<const float>(f32s)); });
    static const std::array<std::uint8_t, 9> bits{1, 0, 1, 1, 0, 0, 0, 0, 1};
    check("bitset", [](TreeView<> &t) { t.bitset(std::span<const std::uint8_t>(bits)); });
    static const std::array<std::string_view, 3> strs{"a", "héllo", ""};
    check("strs", [](TreeView<> &t) { t.strs(std::span<const std::string_view>(strs)); });
    static const std::array<std::int32_t, 1> seven{7};
    check("branch mixed", [](TreeView<> &t) {
        using V = ViewValue<>;
        static const std::array<V, 2> pair{V{std::int32_t{1}}, V{std::int32_t{2}}};
        static const std::array<V, 7> list{
            V{std::int32_t{42}},
            V{std::string_view("hello")},
            V{true},
            V{V::Null{}},
            V{std::int64_t{1} << 40},
            V{std::span<const V>(pair)},
            V{V::Array{SequenceType::i32_array, std::as_bytes(std::span(seven)).empty() ? std::span<const std::uint8_t>() : std::span<const std::uint8_t>(reinterpret_cast<const std::uint8_t *>(seven.data()), sizeof seven)}},
        };
        (void)t.set({std::span<const V>(list)});
    });
    check("branch null child", [](TreeView<> &t) {
        t.branch([](BranchView<> &b) { b.u8(std::nullopt).str(std::nullopt).no_branch().u32(9); });
    });
    check("branch empty", [](TreeView<> &t) { t.branch([](BranchView<> &) {}); });
    check("typed null root", [](TreeView<> &t) { t.u32(std::nullopt); });
    static std::vector<std::uint8_t> inner;
    {
        TreeView<> t;
        t.branch([](BranchView<> &b) { b.i32(1).str("x"); });
        inner = *encode<std::allocator<std::byte>>(t);
    }
    check("graft", [](TreeView<> &t) { t.graft(inner); });
}

XPUTE_TEST(xtp_the_tree_is_refused_past_its_cap_and_a_cap_outside_a_packet_s_sizes) {
    auto big = [](TreeView<> &t) { t.str(std::string_view("0123456789abcdef0123456789abcdef")); };
    XPUTE_CHECK_EQ(encoded(big, 16, 48), "errno:" + std::to_string(static_cast<int>(Errno::eoverflow)), "past the cap");
    XPUTE_CHECK_EQ(encoded([](TreeView<> &t) { t.nil(); }, 16, 4), "errno:" + std::to_string(static_cast<int>(Errno::einval)), "a cap under a header");
}
