// cpp/xpute/kit/codec/json.hpp

// A JSON reader that allocates nothing. `read_json` checks a text against
// JSON's grammar once; the values it hands out are then read in place, and no
// tree is built. `quoted` writes a string as a JSON string.

#pragma once

#include <cstddef>
#include <cstdint>
#include <optional>
#include <string_view>


namespace xpute {

namespace json_detail {

std::size_t skip_ws(std::string_view s, std::size_t at) noexcept;
void push_utf8(std::uint32_t c, char *out, std::size_t &n) noexcept;

} // namespace json_detail

enum class JsonType : std::uint8_t { null, boolean, number, string, array, object };

// The characters of a `JsonStr`, its escapes decoded. A `\u` escape of a lone
// surrogate reads as U+FFFD.
class JsonCharIter {
  public:
    explicit JsonCharIter(std::string_view rest) noexcept : rest_(rest) {}
    std::optional<char32_t> next() noexcept;

  private:
    std::string_view rest_;
};

// A JSON string as it stands in the text, without its quotes. Its escapes are
// decoded only as its characters are read.
class JsonStr {
  public:
    explicit JsonStr(std::string_view raw) noexcept : raw_(raw) {}
    // This answers the string as it stands, or none when it holds an escape
    // and must be decoded.
    std::optional<std::string_view> as_plain() const noexcept;
    JsonCharIter chars() const noexcept { return JsonCharIter(raw_); }
    bool eq_str(std::string_view s) const noexcept;
    template <class S> void decode_into(S &out) const {
        JsonCharIter c = chars();
        while (std::optional<char32_t> ch = c.next()) {
            char buf[4];
            std::size_t n = 0;
            json_detail::push_utf8(*ch, buf, n);
            out.append(buf, n);
        }
    }
    std::string_view raw() const noexcept { return raw_; }

  private:
    std::string_view raw_;
};

class JsonItemIter;
class JsonFieldIter;

class JsonValue {
  public:
    JsonType type_of() const noexcept;
    bool is_null() const noexcept { return type_of() == JsonType::null; }
    std::optional<bool> as_bool() const noexcept;
    std::optional<double> as_f64() const noexcept;
    std::optional<JsonStr> as_str() const noexcept;
    JsonItemIter items() const noexcept;
    // Every field in the text's order; a key that appears twice is yielded
    // twice.
    JsonFieldIter fields() const noexcept;
    // The last field with that key, which is the one the runtime's
    // `JSON.parse` keeps.
    std::optional<JsonValue> get(std::string_view key) const noexcept;
    std::optional<JsonValue> at(std::size_t idx) const noexcept;
    std::string_view text() const noexcept { return s_; }

  private:
    friend std::optional<JsonValue> read_json(std::string_view) noexcept;
    friend class JsonItemIter;
    friend class JsonFieldIter;
    explicit JsonValue(std::string_view s) noexcept : s_(s) {}
    std::string_view s_;
};

class JsonItemIter {
  public:
    std::optional<JsonValue> next() noexcept;

  private:
    friend class JsonValue;
    JsonItemIter(std::string_view s, std::size_t at) noexcept : s_(s), at_(at) {}
    std::string_view s_;
    std::size_t at_;
};

class JsonFieldIter {
  public:
    std::optional<std::pair<JsonStr, JsonValue>> next() noexcept;

  private:
    friend class JsonValue;
    JsonFieldIter(std::string_view s, std::size_t at) noexcept : s_(s), at_(at) {}
    std::string_view s_;
    std::size_t at_;
};

// The one value the text holds, or none when the text is not exactly one JSON
// value with only whitespace around it.
std::optional<JsonValue> read_json(std::string_view text) noexcept;

// Writes `s` as a JSON string, as `JSON.stringify` writes one: a control
// character other than \n, \r and \t becomes `\u00XX`.
template <class Put> void quoted(std::string_view s, Put &&put) {
    static constexpr char HEX[] = "0123456789abcdef";
    put(std::string_view("\""));
    std::size_t plain = 0;
    for (std::size_t i = 0; i < s.size(); i++) {
        auto c = static_cast<unsigned char>(s[i]);
        std::string_view esc;
        char u[6];
        switch (c) {
        case '"': esc = "\\\""; break;
        case '\\': esc = "\\\\"; break;
        case '\n': esc = "\\n"; break;
        case '\r': esc = "\\r"; break;
        case '\t': esc = "\\t"; break;
        default:
            if (c >= 0x20) continue;
            u[0] = '\\', u[1] = 'u', u[2] = '0', u[3] = '0', u[4] = HEX[c >> 4], u[5] = HEX[c & 15];
            esc = std::string_view(u, 6);
        }
        put(s.substr(plain, i - plain));
        put(esc);
        plain = i + 1;
    }
    put(s.substr(plain));
    put(std::string_view("\""));
}

} // namespace xpute
