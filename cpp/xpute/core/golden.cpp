// cpp/xpute/core/golden.cpp

#include "golden.hpp"

#include <type_traits>

#include <bit>
#include <cstdio>
#include <cstdlib>

#include "status/bug.hpp"

namespace xpute {

Golden Golden::load(const char *path) {
    std::FILE *f = std::fopen(path, "rb");
    ensure(f != nullptr, Errno::enoent);
    std::string text;
    char buf[4096];
    for (std::size_t n; (n = std::fread(buf, 1, sizeof buf, f)) > 0;) text.append(buf, n);
    std::fclose(f);
    Golden g;
    std::size_t at = 0;
    while (at < text.size()) {
        std::size_t end = text.find('\n', at);
        if (end == std::string::npos) end = text.size();
        std::string_view line(text.data() + at, end - at);
        if (std::size_t tab = line.find('\t'); tab != std::string_view::npos) {
            g.map_.emplace(std::string(line.substr(0, tab)), std::string(line.substr(tab + 1)));
        }
        at = end + 1;
    }
    return g;
}

bool Golden::has(std::string_view key) const {
    return map_.contains(std::string(key));
}

const std::string &Golden::s(std::string_view key) const {
    auto it = map_.find(std::string(key));
    ensure(it != map_.end(), Errno::enoent);
    return it->second;
}

// A number, the whole of the value or a bug.
template <class T> static T parse(const std::string &text, T (*conv)(const char *, char **, int)) {
    // Digits, a sign only where the type has one: strtoull would take a
    // space and wrap a minus, where Rust's parse refuses both.
    bool sign = !text.empty() && text[0] == '-' && std::is_signed_v<T>;
    bool digits = text.size() > (sign ? 1u : 0u) && text.find_first_not_of("0123456789", sign ? 1 : 0) == std::string::npos;
    ensure(digits, Errno::einval);
    char *end = nullptr;
    T v = conv(text.c_str(), &end, 10);
    ensure(*end == '\0', Errno::einval);
    return v;
}

std::uint64_t Golden::u64(std::string_view key) const {
    return parse<unsigned long long>(s(key), std::strtoull);
}

std::int64_t Golden::i64(std::string_view key) const {
    return parse<long long>(s(key), std::strtoll);
}

std::uint32_t Golden::u32(std::string_view key) const {
    std::uint64_t v = u64(key);
    ensure(v <= 0xffffffffu, Errno::einval);
    return static_cast<std::uint32_t>(v);
}

std::int32_t Golden::i32(std::string_view key) const {
    std::int64_t v = i64(key);
    ensure(v >= INT32_MIN && v <= INT32_MAX, Errno::einval);
    return static_cast<std::int32_t>(v);
}

double Golden::f64(std::string_view key) const {
    const std::string &text = s(key);
    std::string_view hex = text;
    if (hex.starts_with("0x")) hex.remove_prefix(2);
    char *end = nullptr;
    std::string digits(hex);
    std::uint64_t bits = std::strtoull(digits.c_str(), &end, 16);
    ensure(!digits.empty() && *end == '\0', Errno::einval);
    return std::bit_cast<double>(bits);
}

bool Golden::boolean(std::string_view key) const {
    const std::string &text = s(key);
    if (text == "true") return true;
    if (text == "false") return false;
    bug(Errno::einval);
}

std::vector<std::uint8_t> Golden::bytes(std::string_view key) const {
    const std::string &text = s(key);
    ensure(text.size() % 2 == 0, Errno::einval);
    std::vector<std::uint8_t> out;
    for (std::size_t i = 0; i < text.size(); i += 2) {
        std::string pair = text.substr(i, 2);
        char *end = nullptr;
        unsigned long v = std::strtoul(pair.c_str(), &end, 16);
        ensure(*end == '\0', Errno::einval);
        out.push_back(static_cast<std::uint8_t>(v));
    }
    return out;
}

void Golden::each(std::string_view base, const std::function<void(const std::string &)> &f) const {
    std::size_t k = 0;
    for (;; k++) {
        std::string prefix = base.empty() ? std::to_string(k) : std::string(base) + "." + std::to_string(k);
        std::string dotted = prefix + ".";
        bool any = false;
        for (const auto &[key, value] : map_) {
            if (key == prefix || key.starts_with(dotted)) {
                any = true;
                break;
            }
        }
        if (!any) break;
        f(prefix);
    }
    ensure(k > 0, Errno::enoent);
}

std::size_t Golden::len(std::string_view base) const {
    std::size_t n = 0;
    each(base, [&n](const std::string &) { n++; });
    return n;
}

} // namespace xpute
