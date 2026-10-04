// cpp/xpute/core/codec/json.hpp

// JSON checked once and read where it lies, with nothing allocated.

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

enum class JsonKind : std::uint8_t { null, boolean, number, string, array, object };

// A lone surrogate reads as U+FFFD.
class JsonChars {
  public:
    explicit JsonChars(std::string_view rest) noexcept : rest_(rest) {}
    std::optional<char32_t> next() noexcept;

  private:
    std::string_view rest_;
};

// Escapes as written, decoded as they are read.
class JsonStr {
  public:
    explicit JsonStr(std::string_view raw) noexcept : raw_(raw) {}
    // None when it holds an escape.
    std::optional<std::string_view> as_plain() const noexcept;
    JsonChars chars() const noexcept { return JsonChars(raw_); }
    bool eq_str(std::string_view s) const noexcept;
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

class JsonValue {
  public:
    JsonKind kind() const noexcept;
    bool is_null() const noexcept { return kind() == JsonKind::null; }
    std::optional<bool> as_bool() const noexcept;
    std::optional<double> as_f64() const noexcept;
    std::optional<JsonStr> as_str() const noexcept;
    JsonItems items() const noexcept;
    // A key given twice is given twice.
    JsonFields fields() const noexcept;
    // The last field of that name, as the host's parse keeps.
    std::optional<JsonValue> get(std::string_view key) const noexcept;
    std::optional<JsonValue> at(std::size_t index) const noexcept;
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

// None where the text holds anything but one value.
std::optional<JsonValue> read_json(std::string_view text) noexcept;

// Control characters other than \n, \r and \t as `\u00XX`, as
// `JSON.stringify` writes them.
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
