// cpp/xpute/core/codec/json.hpp

// JSON read where it lies, as Rust's codec/json.rs reads it: the documents
// a guest is handed whole, checked once by JSON's own grammar and walked when
// asked, with nothing allocated. And a string written as JSON writes one, a
// piece at a time.

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

// ============ Reading in place ============
//
// `JSON.parse` for a module with no heap: the text is checked once, and what
// is read of it is read where it lies — a value is the span of its bytes, an
// array or an object is walked when asked, and a string is decoded character
// by character as it is compared or written out. Nothing is built.

enum class JsonKind : std::uint8_t { null, boolean, number, string, array, object };

// A JSON string's characters, escapes decoded; a lone surrogate reads as U+FFFD.
class JsonChars {
  public:
    explicit JsonChars(std::string_view rest) noexcept : rest_(rest) {}
    // The next character as a code point, or none past the last.
    std::optional<char32_t> next() noexcept;

  private:
    std::string_view rest_;
};

// A JSON string where it lies: its characters between the quotes, escapes as
// written, decoded as they are read.
class JsonStr {
  public:
    explicit JsonStr(std::string_view raw) noexcept : raw_(raw) {}
    // The string itself, where it holds no escape: most do.
    std::optional<std::string_view> as_plain() const noexcept;
    JsonChars chars() const noexcept { return JsonChars(raw_); }
    bool eq_str(std::string_view s) const noexcept;
    // The string decoded, as UTF-8, into `out` (a string or anything with
    // `append(const char *, size_t)`).
    template <class S> void decode_into(S &out) const {
        JsonChars c = chars();
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

class JsonItems;
class JsonFields;

// One JSON value where it lies in the text it was read from.
class JsonValue {
  public:
    JsonKind kind() const noexcept;
    bool is_null() const noexcept { return kind() == JsonKind::null; }
    std::optional<bool> as_bool() const noexcept;
    std::optional<double> as_f64() const noexcept;
    std::optional<JsonStr> as_str() const noexcept;
    // An array's items in order; nothing for any other value.
    JsonItems items() const noexcept;
    // An object's fields in order, a key given twice given twice; nothing for
    // any other value.
    JsonFields fields() const noexcept;
    // `value[key]` on an object: the last field of that name, as the host's
    // parse keeps.
    std::optional<JsonValue> get(std::string_view key) const noexcept;
    // `value[index]` on an array.
    std::optional<JsonValue> at(std::size_t index) const noexcept;
    // The value's own bytes, checked, from its first to its last.
    std::string_view text() const noexcept { return s_; }

  private:
    friend std::optional<JsonValue> read_json(std::string_view) noexcept;
    friend class JsonItems;
    friend class JsonFields;
    explicit JsonValue(std::string_view s) noexcept : s_(s) {}
    std::string_view s_;
};

class JsonItems {
  public:
    std::optional<JsonValue> next() noexcept;

  private:
    friend class JsonValue;
    JsonItems(std::string_view s, std::size_t at) noexcept : s_(s), at_(at) {}
    std::string_view s_;
    std::size_t at_;
};

class JsonFields {
  public:
    std::optional<std::pair<JsonStr, JsonValue>> next() noexcept;

  private:
    friend class JsonValue;
    JsonFields(std::string_view s, std::size_t at) noexcept : s_(s), at_(at) {}
    std::string_view s_;
    std::size_t at_;
};

// `JSON.parse(text)` in place: the one value the text holds, or none where it
// holds anything else.
std::optional<JsonValue> read_json(std::string_view text) noexcept;

// ============ Writing in place ============

// `s` written as a JSON string: quoted, with a quote, a backslash and every
// control character escaped — `JSON.stringify(String(v))` with the control
// characters other than \n, \r and \t as `\u00XX` — handed to `put` a piece
// at a time, as Rust's `Quoted` writes into a formatter.
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
