// cpp/xpute/core/codec/json.test.cpp

#include <string>
#include <string_view>
#include <vector>

#include "../golden.hpp"
#include "../test.hpp"
#include "encoding.hpp"
#include "json.hpp"

using namespace xpute;

static std::string hex_of(std::string_view s) {
    std::string out(hex_len(s.size()), '\0');
    hex_encode({reinterpret_cast<const std::uint8_t *>(s.data()), s.size()}, out);
    return out;
}

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
    XPUTE_CHECK(doc && doc->kind() == JsonKind::object);
    XPUTE_CHECK(doc->get("a")->as_str()->eq_str("last"));
    JsonFields fields = doc->fields();
    JsonValue first = fields.next()->second;
    std::vector<JsonKind> kinds;
    JsonItems items = first.items();
    while (auto item = items.next()) kinds.push_back(item->kind());
    XPUTE_CHECK((kinds == std::vector<JsonKind>{JsonKind::number, JsonKind::number, JsonKind::boolean, JsonKind::null, JsonKind::string}));
    XPUTE_CHECK_EQ(*first.at(1)->as_f64(), -25.0, "a number reads as Rust's parse reads it");
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

XPUTE_TEST(json_writes_and_reads_what_the_host_does) {
    Golden v = Golden::load("spec/xpute/golden/codec/json.tsv");
    auto list = [&](const std::string &base) {
        std::vector<std::string> out;
        if (!v.has(base + ".0")) return out;
        v.each(base, [&](const std::string &k) { out.push_back(v.s(k)); });
        return out;
    };
    v.each("stringify", [&](const std::string &k) {
        std::vector<std::string> keys = list(k + ".keys"), values = list(k + ".values");
        StrMap<> map;
        for (std::size_t i = 0; i < keys.size(); i++) {
            std::vector<std::uint8_t> kb(keys[i].size() / 2);
            (void)hex_decode(keys[i], kb);
            std::optional<std::string> val;
            if (values[i] != "undefined") {
                std::vector<std::uint8_t> vb(values[i].size() / 2);
                (void)hex_decode(values[i], vb);
                val = std::string(vb.begin(), vb.end());
            }
            map.emplace_back(std::string(kb.begin(), kb.end()), val);
        }
        XPUTE_CHECK_EQ(stringify_str_map(map), text_of(v, k + ".json"), "stringify wrote another text");
    });
    v.each("parse", [&](const std::string &k) {
        Result<StrMap<>> m = parse_str_map(text_of(v, k + ".text"));
        std::string got;
        if (!m.ok()) {
            got = "refused";
        } else if (m->empty()) {
            got = "empty";
        } else {
            for (const auto &[key, val] : *m) {
                if (!got.empty()) got += ",";
                got += hex_of(key) + "=" + hex_of(*val);
            }
        }
        XPUTE_CHECK_EQ(got, v.s(k + ".read"), "parse read another map");
    });
}

XPUTE_TEST(json_quoted_escapes_a_quote_a_backslash_and_every_control_character) {
    std::string out;
    quoted("a\"b\\c\nd\x01", [&](std::string_view piece) { out += piece; });
    XPUTE_CHECK_EQ(out, std::string(R"("a\"b\\c\nd\u0001")"), "quoted wrote another text");
}
