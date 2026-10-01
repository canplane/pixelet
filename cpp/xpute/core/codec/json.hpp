// cpp/xpute/core/codec/json.hpp

// `JSON.stringify` and `JSON.parse` of a flat string map — the one JSON the
// records carry (a POI's tags, a placement's), written by either side and
// read by the other, so the text is the host's to the byte — and a reader
// that walks any JSON where it lies, as Rust's codec/json.rs does.
//
// What the host does that a naive port would not:
// - an object's own key order puts array-index keys ("0", "12") first in
//   ascending number, then the rest in insertion order, and both stringify
//   and parse follow it;
// - a key given twice keeps its first position and its last value;
// - an absent value is left out of the text;
// - a string escapes `"`, `\`, the five short controls, and every other code
//   unit below U+0020 as `\u00xx` in lowercase hex, and nothing else.
//
// The one thing it cannot be: a lone surrogate, which a JavaScript string
// holds and a UTF-8 one cannot; `\ud800` alone parses to U+FFFD.
//
// The map is built with the caller's allocator, as `arena` is, so a guest's
// map is the guest's heap's; the reader allocates nothing.

#pragma once

#include <algorithm>
#include <cstddef>
#include <cstdint>
#include <memory>
#include <optional>
#include <string>
#include <string_view>
#include <utility>
#include <vector>

#include "../status/error.hpp"

namespace xpute {

namespace json_detail {

// A canonical array index: "0", or digits with no leading zero, below 2^32 − 1.
bool is_array_index(std::string_view k) noexcept;

// The index's value, for one `is_array_index` accepted.
std::uint64_t index_of(std::string_view k) noexcept;

// `s` as `JSON.stringify` writes a string, into `out`.
template <class S> void quote(S &out, std::string_view s) {
    static constexpr char HEX[] = "0123456789abcdef";
    out.push_back('"');
    for (char ch : s) {
        auto c = static_cast<unsigned char>(ch);
        switch (c) {
        case '"': out.append("\\\""); break;
        case '\\': out.append("\\\\"); break;
        case '\b': out.append("\\b"); break;
        case '\f': out.append("\\f"); break;
        case '\n': out.append("\\n"); break;
        case '\r': out.append("\\r"); break;
        case '\t': out.append("\\t"); break;
        default:
            if (c < 0x20) {
                const char esc[] = {'\\', 'u', '0', '0', HEX[c >> 4], HEX[c & 15]};
                out.append(esc, sizeof esc);
            } else {
                out.push_back(ch);
            }
        }
    }
    out.push_back('"');
}

// One parsed string, decoded into `out` from the text at `at`, which opens on
// its quote; where it ends, or none for one that does not read.
template <class S> std::optional<std::size_t> parse_string(std::string_view s, std::size_t at, S &out);

} // namespace json_detail

template <class Alloc = std::allocator<char>> using JsonText = std::basic_string<char, std::char_traits<char>, Alloc>;

// `Record<string, string | undefined>`, in the order it was built.
template <class Alloc = std::allocator<char>>
using StrMap = std::vector<std::pair<JsonText<Alloc>, std::optional<JsonText<Alloc>>>,
                           typename std::allocator_traits<Alloc>::template rebind_alloc<std::pair<JsonText<Alloc>, std::optional<JsonText<Alloc>>>>>;

namespace json_detail {

// The entries as the object holds them: a key once, at its first position
// with its last value, index keys first.
template <class Alloc> StrMap<Alloc> ordered(const StrMap<Alloc> &map, const Alloc &alloc) {
    StrMap<Alloc> out(alloc);
    out.reserve(map.size());
    for (const auto &[k, v] : map) {
        auto have = std::find_if(out.begin(), out.end(), [&](const auto &e) { return e.first == k; });
        if (have != out.end()) have->second = v;
        else out.emplace_back(k, v);
    }
    auto rest = std::stable_partition(out.begin(), out.end(), [](const auto &e) { return is_array_index(e.first); });
    std::stable_sort(out.begin(), rest, [](const auto &a, const auto &b) { return index_of(a.first) < index_of(b.first); });
    return out;
}

} // namespace json_detail

// `JSON.stringify(map)`.
template <class Alloc = std::allocator<char>> JsonText<Alloc> stringify_str_map(const StrMap<Alloc> &map, const Alloc &alloc = Alloc()) {
    JsonText<Alloc> out(alloc);
    out.push_back('{');
    bool first = true;
    for (const auto &[k, v] : json_detail::ordered(map, alloc)) {
        if (!v) continue;
        if (!first) out.push_back(',');
        first = false;
        json_detail::quote(out, k);
        out.push_back(':');
        json_detail::quote(out, *v);
    }
    out.push_back('}');
    return out;
}

namespace json_detail {

std::size_t skip_ws(std::string_view s, std::size_t at) noexcept;
void push_utf8(std::uint32_t c, char *out, std::size_t &n) noexcept;
std::optional<std::uint32_t> hex4(std::string_view s, std::size_t at) noexcept;

template <class S> std::optional<std::size_t> parse_string(std::string_view s, std::size_t at, S &out) {
    at = skip_ws(s, at);
    if (at >= s.size() || s[at] != '"') return std::nullopt;
    at++;
    for (;;) {
        if (at >= s.size()) return std::nullopt;
        auto b = static_cast<unsigned char>(s[at]);
        if (b == '"') return at + 1;
        if (b == '\\') {
            if (++at >= s.size()) return std::nullopt;
            char e = s[at++];
            switch (e) {
            case '"': out.push_back('"'); break;
            case '\\': out.push_back('\\'); break;
            case '/': out.push_back('/'); break;
            case 'b': out.push_back('\b'); break;
            case 'f': out.push_back('\f'); break;
            case 'n': out.push_back('\n'); break;
            case 'r': out.push_back('\r'); break;
            case 't': out.push_back('\t'); break;
            case 'u': {
                std::optional<std::uint32_t> hi = hex4(s, at);
                if (!hi) return std::nullopt;
                at += 4;
                std::uint32_t unit = *hi;
                if (*hi >= 0xd800 && *hi < 0xdc00 && s.substr(at, 2) == "\\u") {
                    std::optional<std::uint32_t> lo = hex4(s, at + 2);
                    if (!lo) return std::nullopt;
                    if (*lo >= 0xdc00 && *lo < 0xe000) {
                        unit = 0x10000 + ((*hi - 0xd800) << 10) + (*lo - 0xdc00);
                        at += 6;
                    }
                }
                if (unit >= 0xd800 && unit < 0xe000) unit = 0xfffd;
                char buf[4];
                std::size_t n = 0;
                push_utf8(unit, buf, n);
                out.append(buf, n);
                break;
            }
            default: return std::nullopt;
            }
            continue;
        }
        if (b < 0x20) return std::nullopt;
        out.push_back(static_cast<char>(b));
        at++;
    }
}

} // namespace json_detail

// `JSON.parse(text)` for an object whose values are strings. Anything else —
// another value type, trailing text — is refused (EBADMSG), where the host
// would hand back whatever the text held.
template <class Alloc = std::allocator<char>> Result<StrMap<Alloc>> parse_str_map(std::string_view text, const Alloc &alloc = Alloc()) {
    using json_detail::skip_ws;
    auto bad = [] { return marshal_error(Errno::ebadmsg); };
    std::size_t at = skip_ws(text, 0);
    if (at >= text.size() || text[at] != '{') return bad();
    at = skip_ws(text, at + 1);
    StrMap<Alloc> raw(alloc);
    if (at < text.size() && text[at] == '}') {
        at++;
    } else {
        for (;;) {
            JsonText<Alloc> k(alloc), v(alloc);
            std::optional<std::size_t> end = json_detail::parse_string(text, at, k);
            if (!end) return bad();
            at = skip_ws(text, *end);
            if (at >= text.size() || text[at] != ':') return bad();
            end = json_detail::parse_string(text, at + 1, v);
            if (!end) return bad();
            raw.emplace_back(std::move(k), std::move(v));
            at = skip_ws(text, *end);
            if (at < text.size() && text[at] == ',') {
                at++;
            } else if (at < text.size() && text[at] == '}') {
                at++;
                break;
            } else {
                return bad();
            }
        }
    }
    if (skip_ws(text, at) != text.size()) return bad();
    return json_detail::ordered(raw, alloc);
}

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
