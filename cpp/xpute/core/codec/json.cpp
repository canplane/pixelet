// cpp/xpute/core/codec/json.cpp

#include "json.hpp"

#include <charconv>

namespace xpute {

namespace json_detail {

bool is_array_index(std::string_view k) noexcept {
    if (k.empty() || k.size() > 10 || (k.size() > 1 && k[0] == '0')) return false;
    for (char c : k) {
        if (c < '0' || c > '9') return false;
    }
    return index_of(k) < 0xffff'ffffu;
}

std::uint64_t index_of(std::string_view k) noexcept {
    std::uint64_t n = 0;
    for (char c : k) n = n * 10 + static_cast<std::uint64_t>(c - '0');
    return n;
}

std::size_t skip_ws(std::string_view s, std::size_t at) noexcept {
    while (at < s.size() && (s[at] == ' ' || s[at] == '\t' || s[at] == '\n' || s[at] == '\r')) at++;
    return at;
}

void push_utf8(std::uint32_t c, char *out, std::size_t &n) noexcept {
    if (c < 0x80) {
        out[n++] = static_cast<char>(c);
    } else if (c < 0x800) {
        out[n++] = static_cast<char>(0xc0 | (c >> 6));
        out[n++] = static_cast<char>(0x80 | (c & 0x3f));
    } else if (c < 0x10000) {
        out[n++] = static_cast<char>(0xe0 | (c >> 12));
        out[n++] = static_cast<char>(0x80 | ((c >> 6) & 0x3f));
        out[n++] = static_cast<char>(0x80 | (c & 0x3f));
    } else {
        out[n++] = static_cast<char>(0xf0 | (c >> 18));
        out[n++] = static_cast<char>(0x80 | ((c >> 12) & 0x3f));
        out[n++] = static_cast<char>(0x80 | ((c >> 6) & 0x3f));
        out[n++] = static_cast<char>(0x80 | (c & 0x3f));
    }
}

static bool is_hex(char c) noexcept {
    return (c >= '0' && c <= '9') || (c >= 'a' && c <= 'f') || (c >= 'A' && c <= 'F');
}

std::optional<std::uint32_t> hex4(std::string_view s, std::size_t at) noexcept {
    if (at + 4 > s.size()) return std::nullopt;
    std::uint32_t v = 0;
    for (std::size_t i = at; i < at + 4; i++) {
        char c = s[i];
        if (!is_hex(c)) return std::nullopt;
        v = v * 16 + static_cast<std::uint32_t>(c <= '9' ? c - '0' : (c | 0x20) - 'a' + 10);
    }
    return v;
}

} // namespace json_detail

using json_detail::hex4;
using json_detail::skip_ws;

// How deep arrays and objects may nest before a text is refused: the check
// recurses, and a module's stack is fixed.
static constexpr std::uint32_t MAX_DEPTH = 128;

// Where the string opening at `at` ends, past its closing quote.
static std::optional<std::size_t> skip_string(std::string_view s, std::size_t at) noexcept {
    at++;
    for (;;) {
        if (at >= s.size()) return std::nullopt;
        auto b = static_cast<unsigned char>(s[at]);
        if (b == '"') return at + 1;
        if (b == '\\') {
            if (at + 1 >= s.size()) return std::nullopt;
            switch (s[at + 1]) {
            case '"': case '\\': case '/': case 'b': case 'f': case 'n': case 'r': case 't': at += 2; break;
            case 'u':
                if (!hex4(s, at + 2)) return std::nullopt;
                at += 6;
                break;
            default: return std::nullopt;
            }
            continue;
        }
        if (b < 0x20) return std::nullopt;
        // A byte of a UTF-8 sequence is never a quote or a backslash, so the
        // text is walked a byte at a time.
        at++;
    }
}

// Whether `t` is a number as Rust's `f64` parse reads one, over the
// characters a JSON number is scanned by: an optional sign, digits with an
// optional point and fraction or a point and digits, an optional exponent.
static bool rust_f64(std::string_view t) noexcept {
    std::size_t i = 0;
    auto digits = [&] {
        std::size_t from = i;
        while (i < t.size() && t[i] >= '0' && t[i] <= '9') i++;
        return i - from;
    };
    if (i < t.size() && (t[i] == '+' || t[i] == '-')) i++;
    std::size_t whole = digits(), frac = 0;
    if (i < t.size() && t[i] == '.') {
        i++;
        frac = digits();
    }
    if (whole + frac == 0) return false;
    if (i < t.size() && (t[i] == 'e' || t[i] == 'E')) {
        i++;
        if (i < t.size() && (t[i] == '+' || t[i] == '-')) i++;
        if (digits() == 0) return false;
    }
    return i == t.size();
}

// Where the value starting at `at` ends.
static std::optional<std::size_t> skip_value(std::string_view s, std::size_t at, std::uint32_t depth) noexcept {
    if (depth > MAX_DEPTH || at >= s.size()) return std::nullopt;
    auto literal = [&](std::string_view word) -> std::optional<std::size_t> {
        if (s.substr(at, word.size()) == word) return at + word.size();
        return std::nullopt;
    };
    switch (s[at]) {
    case 'n': return literal("null");
    case 't': return literal("true");
    case 'f': return literal("false");
    case '"': return skip_string(s, at);
    case '[': {
        std::size_t i = skip_ws(s, at + 1);
        if (i < s.size() && s[i] == ']') return i + 1;
        for (;;) {
            std::optional<std::size_t> end = skip_value(s, i, depth + 1);
            if (!end) return std::nullopt;
            i = skip_ws(s, *end);
            if (i >= s.size()) return std::nullopt;
            if (s[i] == ',') i = skip_ws(s, i + 1);
            else if (s[i] == ']') return i + 1;
            else return std::nullopt;
        }
    }
    case '{': {
        std::size_t i = skip_ws(s, at + 1);
        if (i < s.size() && s[i] == '}') return i + 1;
        for (;;) {
            if (i >= s.size() || s[i] != '"') return std::nullopt;
            std::optional<std::size_t> key = skip_string(s, i);
            if (!key) return std::nullopt;
            i = skip_ws(s, *key);
            if (i >= s.size() || s[i] != ':') return std::nullopt;
            std::optional<std::size_t> end = skip_value(s, skip_ws(s, i + 1), depth + 1);
            if (!end) return std::nullopt;
            i = skip_ws(s, *end);
            if (i >= s.size()) return std::nullopt;
            if (s[i] == ',') i = skip_ws(s, i + 1);
            else if (s[i] == '}') return i + 1;
            else return std::nullopt;
        }
    }
    default: {
        std::size_t end = at;
        while (end < s.size() && (s[end] == '-' || s[end] == '+' || s[end] == '.' || s[end] == 'e' || s[end] == 'E' || (s[end] >= '0' && s[end] <= '9'))) end++;
        if (!rust_f64(s.substr(at, end - at))) return std::nullopt;
        return end;
    }
    }
}

std::optional<JsonValue> read_json(std::string_view text) noexcept {
    std::size_t start = skip_ws(text, 0);
    std::optional<std::size_t> end = skip_value(text, start, 0);
    if (!end || skip_ws(text, *end) != text.size()) return std::nullopt;
    return JsonValue(text.substr(start, *end - start));
}

JsonKind JsonValue::kind() const noexcept {
    switch (s_[0]) {
    case 'n': return JsonKind::null;
    case 't': case 'f': return JsonKind::boolean;
    case '"': return JsonKind::string;
    case '[': return JsonKind::array;
    case '{': return JsonKind::object;
    default: return JsonKind::number;
    }
}

std::optional<bool> JsonValue::as_bool() const noexcept {
    if (s_ == "true") return true;
    if (s_ == "false") return false;
    return std::nullopt;
}

std::optional<double> JsonValue::as_f64() const noexcept {
    if (kind() != JsonKind::number) return std::nullopt;
    std::string_view t = s_;
    // from_chars takes no leading plus, which Rust's parse does.
    if (!t.empty() && t[0] == '+') t.remove_prefix(1);
    double v = 0;
    auto [end, ec] = std::from_chars(t.data(), t.data() + t.size(), v);
    if (ec != std::errc() || end != t.data() + t.size()) return std::nullopt;
    return v;
}

std::optional<JsonStr> JsonValue::as_str() const noexcept {
    if (kind() != JsonKind::string) return std::nullopt;
    return JsonStr(s_.substr(1, s_.size() - 2));
}

JsonItems JsonValue::items() const noexcept {
    return JsonItems(s_, kind() == JsonKind::array ? 1 : s_.size());
}

JsonFields JsonValue::fields() const noexcept {
    return JsonFields(s_, kind() == JsonKind::object ? 1 : s_.size());
}

std::optional<JsonValue> JsonValue::get(std::string_view key) const noexcept {
    std::optional<JsonValue> last;
    JsonFields f = fields();
    while (auto field = f.next()) {
        if (field->first.eq_str(key)) last = field->second;
    }
    return last;
}

std::optional<JsonValue> JsonValue::at(std::size_t index) const noexcept {
    JsonItems it = items();
    for (std::size_t i = 0;; i++) {
        std::optional<JsonValue> v = it.next();
        if (!v || i == index) return v;
    }
}

std::optional<JsonValue> JsonItems::next() noexcept {
    std::size_t start = skip_ws(s_, at_);
    if (start >= s_.size() || s_[start] == ']') {
        at_ = s_.size();
        return std::nullopt;
    }
    // The text was checked whole, so a value is where the grammar says.
    std::size_t end = *skip_value(s_, start, 0);
    at_ = skip_ws(s_, end) + 1;
    return JsonValue(s_.substr(start, end - start));
}

std::optional<std::pair<JsonStr, JsonValue>> JsonFields::next() noexcept {
    std::size_t key_at = skip_ws(s_, at_);
    if (key_at >= s_.size() || s_[key_at] != '"') {
        at_ = s_.size();
        return std::nullopt;
    }
    std::size_t key_end = *skip_string(s_, key_at);
    std::size_t value_at = skip_ws(s_, skip_ws(s_, key_end) + 1);
    std::size_t value_end = *skip_value(s_, value_at, 0);
    at_ = skip_ws(s_, value_end) + 1;
    return std::pair{JsonStr(s_.substr(key_at + 1, key_end - key_at - 2)), JsonValue(s_.substr(value_at, value_end - value_at))};
}

std::optional<std::string_view> JsonStr::as_plain() const noexcept {
    if (raw_.find('\\') != std::string_view::npos) return std::nullopt;
    return raw_;
}

// The next code point of UTF-8 `s` at `at`, which it advances past.
static char32_t utf8_next(std::string_view s, std::size_t &at) noexcept {
    auto b = static_cast<unsigned char>(s[at]);
    std::size_t len = b < 0x80 ? 1 : b < 0xe0 ? 2 : b < 0xf0 ? 3 : 4;
    char32_t c = len == 1 ? b : len == 2 ? (b & 0x1f) : len == 3 ? (b & 0x0f) : (b & 0x07);
    for (std::size_t i = 1; i < len && at + i < s.size(); i++) c = (c << 6) | (static_cast<unsigned char>(s[at + i]) & 0x3f);
    at += len;
    return c;
}

std::optional<char32_t> JsonChars::next() noexcept {
    if (rest_.empty()) return std::nullopt;
    std::size_t at = 0;
    char32_t c = utf8_next(rest_, at);
    if (c != U'\\') {
        rest_.remove_prefix(at);
        return c;
    }
    if (at >= rest_.size()) return std::nullopt;
    char32_t e = utf8_next(rest_, at);
    switch (e) {
    case U'b': c = U'\b'; break;
    case U'f': c = U'\f'; break;
    case U'n': c = U'\n'; break;
    case U'r': c = U'\r'; break;
    case U't': c = U'\t'; break;
    case U'u': {
        std::optional<std::uint32_t> hi = hex4(rest_, at);
        if (!hi) return std::nullopt;
        at += 4;
        std::uint32_t unit = *hi;
        if (*hi >= 0xd800 && *hi < 0xdc00 && rest_.substr(at, 2) == "\\u") {
            std::optional<std::uint32_t> lo = hex4(rest_, at + 2);
            if (lo && *lo >= 0xdc00 && *lo < 0xe000) {
                unit = 0x10000 + ((*hi - 0xd800) << 10) + (*lo - 0xdc00);
                at += 6;
            }
        }
        c = unit >= 0xd800 && unit < 0xe000 ? U'�' : static_cast<char32_t>(unit);
        break;
    }
    default: c = e;
    }
    rest_.remove_prefix(at);
    return c;
}

bool JsonStr::eq_str(std::string_view s) const noexcept {
    if (std::optional<std::string_view> plain = as_plain()) return *plain == s;
    JsonChars c = chars();
    std::size_t at = 0;
    for (;;) {
        std::optional<char32_t> a = c.next();
        if (!a) return at == s.size();
        if (at >= s.size() || utf8_next(s, at) != *a) return false;
    }
}

} // namespace xpute
