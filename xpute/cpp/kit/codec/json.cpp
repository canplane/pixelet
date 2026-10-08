// cpp/kit/codec/json.cpp

#include "json.hpp"

#include <charconv>

namespace xpute {

namespace json_detail {

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

// The check recurses, and a module's stack is fixed.
static constexpr std::uint32_t MAX_DEPTH = 128;

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
        // A UTF-8 continuation byte is never a quote or a backslash.
        at++;
    }
}

// By JSON's grammar: `+1`, `.5`, `1.` and `01` are refused, as `JSON.parse`
// refuses them.
static std::optional<std::size_t> skip_number(std::string_view s, std::size_t at) noexcept {
    auto digits = [&](std::size_t from) {
        std::size_t i = from;
        while (i < s.size() && s[i] >= '0' && s[i] <= '9') i++;
        return i - from;
    };
    if (at < s.size() && s[at] == '-') at++;
    std::size_t n = digits(at);
    if (n == 0 || (n > 1 && s[at] == '0')) return std::nullopt;
    at += n;
    if (at < s.size() && s[at] == '.') {
        n = digits(at + 1);
        if (n == 0) return std::nullopt;
        at += 1 + n;
    }
    if (at < s.size() && (s[at] == 'e' || s[at] == 'E')) {
        at++;
        if (at < s.size() && (s[at] == '+' || s[at] == '-')) at++;
        n = digits(at);
        if (n == 0) return std::nullopt;
        at += n;
    }
    return at;
}

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
    default: return skip_number(s, at);
    }
}

std::optional<JsonValue> read_json(std::string_view text) noexcept {
    std::size_t start = skip_ws(text, 0);
    std::optional<std::size_t> end = skip_value(text, start, 0);
    if (!end || skip_ws(text, *end) != text.size()) return std::nullopt;
    return JsonValue(text.substr(start, *end - start));
}

JsonType JsonValue::type_of() const noexcept {
    switch (s_[0]) {
    case 'n': return JsonType::null;
    case 't': case 'f': return JsonType::boolean;
    case '"': return JsonType::string;
    case '[': return JsonType::array;
    case '{': return JsonType::object;
    default: return JsonType::number;
    }
}

std::optional<bool> JsonValue::as_bool() const noexcept {
    if (s_ == "true") return true;
    if (s_ == "false") return false;
    return std::nullopt;
}

std::optional<double> JsonValue::as_f64() const noexcept {
    if (type_of() != JsonType::number) return std::nullopt;
    std::string_view t = s_;
    double v = 0;
    auto [end, ec] = std::from_chars(t.data(), t.data() + t.size(), v);
    if (ec != std::errc() || end != t.data() + t.size()) return std::nullopt;
    return v;
}

std::optional<JsonStr> JsonValue::as_str() const noexcept {
    if (type_of() != JsonType::string) return std::nullopt;
    return JsonStr(s_.substr(1, s_.size() - 2));
}

JsonItems JsonValue::items() const noexcept {
    return JsonItems(s_, type_of() == JsonType::array ? 1 : s_.size());
}

JsonFields JsonValue::fields() const noexcept {
    return JsonFields(s_, type_of() == JsonType::object ? 1 : s_.size());
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
    // The text was checked whole.
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
