// cpp/xpute/kit/codec/json.test.cpp

#include <bit>
#include <cstdint>
#include <optional>
#include <string>
#include <string_view>
#include <vector>

#include "xpute/kit/golden.hpp"
#include "xpute/kit/test.hpp"
#include "encoding.hpp"
#include "json.hpp"

using namespace xpute;

static std::string text_of(const Golden &v, const std::string &key) {
    if (!v.has(key)) return "";
    std::vector<std::uint8_t> b = v.bytes(key);
    return std::string(b.begin(), b.end());
}

static std::string decoded(JsonStr s) {
    std::string out;
    s.decode_into(out);
    return out;
}

XPUTE_TEST(json_read_in_place_walks_what_the_text_holds_and_refuses_what_is_not_json) {
    std::optional<JsonValue> doc = read_json(R"( {"a": [1, -2.5e1, true, null, "x\"y"], "b": {"k": "\uD83D\uDE00 \u00e9\n"}, "a": "last"} )");
    XPUTE_CHECK(doc && doc->type_of() == JsonType::object);
    XPUTE_CHECK(doc->get("a")->as_str()->eq_str("last"));
    JsonFields fields = doc->fields();
    JsonValue first = fields.next()->second;
    std::vector<JsonType> kinds;
    JsonItems items = first.items();
    while (auto item = items.next()) kinds.push_back(item->type_of());
    XPUTE_CHECK((kinds == std::vector<JsonType>{JsonType::number, JsonType::number, JsonType::boolean, JsonType::null, JsonType::string}));
    XPUTE_CHECK_EQ(*first.at(1)->as_f64(), -25.0, "a number reads as the host reads it");
    XPUTE_CHECK_EQ(*first.at(2)->as_bool(), true, "true");
    XPUTE_CHECK(first.at(3)->is_null());
    JsonStr q = *first.at(4)->as_str();
    XPUTE_CHECK(!q.as_plain());
    XPUTE_CHECK_EQ(decoded(q), std::string("x\"y"), "an escaped quote");
    XPUTE_CHECK(doc->get("b")->get("k")->as_str()->eq_str("\xf0\x9f\x98\x80 \xc3\xa9\n"));
    XPUTE_CHECK(!doc->get("missing"));
    XPUTE_CHECK(!read_json("[]")->items().next());
    XPUTE_CHECK_EQ(decoded(*read_json(R"("\ud800")")->as_str()), std::string("\xef\xbf\xbd"), "a lone surrogate");
    for (std::string_view bad : {"", "[1,]", "{\"a\" 1}", "\"open", "\"a\x01\"", "\"\\q\"", "1 2", "{\"a\":1,}", "tru"}) {
        XPUTE_CHECK(!read_json(bad));
    }
    XPUTE_CHECK(!read_json(std::string(200, '[') + std::string(200, ']')));
}

XPUTE_TEST(json_is_what_the_host_s_json_parse_takes) {
    Golden v = Golden::load("spec/golden/codec/json.tsv");
    v.each("grammar", [&](const std::string &k) {
        std::string got = read_json(text_of(v, k + ".text")) ? "json" : "refused";
        XPUTE_CHECK_EQ(got, v.s(k + ".read"), "a text judged JSON otherwise than the host judges it");
    });
}

XPUTE_TEST(json_reads_a_value_as_deep_as_the_record_and_refuses_one_deeper) {
    Golden v = Golden::load("spec/golden/codec/json.tsv");
    std::uint64_t most = v.u64("depth.most");
    auto nested = [](std::uint64_t n) { return std::string(n, '[') + "0" + std::string(n, ']'); };
    XPUTE_CHECK(read_json(nested(most)).has_value());
    XPUTE_CHECK(!read_json(nested(most + 1)).has_value());
}

XPUTE_TEST(json_a_number_past_the_doubles_reads_as_the_host_reads_it) {
    Golden v = Golden::load("spec/golden/codec/json.tsv");
    v.each("number", [&](const std::string &k) {
        std::optional<double> n = read_json(v.s(k + ".text"))->as_f64();
        XPUTE_CHECK(n.has_value());
        auto bits = std::bit_cast<std::uint64_t>(n.value_or(0));
        std::string got(16, '0');
        for (int i = 15; i >= 0; i--, bits >>= 4) got[i] = "0123456789abcdef"[bits & 15];
        XPUTE_CHECK_EQ(got, v.s(k + ".bits"), k.c_str());
    });
}

XPUTE_TEST(json_quoted_escapes_a_quote_a_backslash_and_every_control_character) {
    std::string out;
    quoted("a\"b\\c\nd\x01", [&](std::string_view piece) { out += piece; });
    XPUTE_CHECK_EQ(out, std::string(R"("a\"b\\c\nd\u0001")"), "quoted wrote another text");
}
