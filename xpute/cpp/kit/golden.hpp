// cpp/kit/golden.hpp

// The golden records: lines of `path<TAB>value`, split per line with no parser
// so reading is not itself a thing to verify. For tests only.

#pragma once

#include <cstdint>
#include <functional>
#include <string>
#include <string_view>
#include <unordered_map>
#include <vector>

namespace xpute {

class Golden {
  public:
    static Golden load(const char *path);

    bool has(std::string_view key) const;
    const std::string &s(std::string_view key) const;
    std::uint64_t u64(std::string_view key) const;
    std::int64_t i64(std::string_view key) const;
    std::uint32_t u32(std::string_view key) const;
    std::int32_t i32(std::string_view key) const;
    double f64(std::string_view key) const;
    bool boolean(std::string_view key) const;
    std::vector<std::uint8_t> bytes(std::string_view key) const;

    void each(std::string_view base, const std::function<void(const std::string &)> &f) const;
    std::size_t len(std::string_view base) const;
    bool is_empty() const noexcept { return map_.empty(); }

  private:
    std::unordered_map<std::string, std::string> map_;
};

} // namespace xpute
