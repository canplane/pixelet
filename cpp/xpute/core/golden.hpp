// cpp/xpute/core/golden.hpp

// The reader of the golden records (spec/xpute/golden/): lines of
// `path<TAB>value`, the path the keys and indices down to the value joined by
// dots. A split per line and no parser, so reading is not itself a thing to
// verify. The same files the Rust and the TypeScript read, and the same rule
// (ARCHITECTURE.xpute.md, "The golden records"): across a protocol the bytes
// must match, inside a language each follows its own.
//
// For tests only, which are a host's programs and not a guest: this one takes
// its memory from the system, as a test may.

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
    // The record at `path`, from the repository's root.
    static Golden load(const char *path);

    bool has(std::string_view key) const;
    const std::string &s(std::string_view key) const;
    std::uint64_t u64(std::string_view key) const;
    std::int64_t i64(std::string_view key) const;
    std::uint32_t u32(std::string_view key) const;
    std::int32_t i32(std::string_view key) const;
    // A double written as its bits in hex.
    double f64(std::string_view key) const;
    bool boolean(std::string_view key) const;
    // Bytes written as hex.
    std::vector<std::uint8_t> bytes(std::string_view key) const;

    // Calls `f` with `base.0`, `base.1`, … while an entry has that prefix.
    void each(std::string_view base, const std::function<void(const std::string &)> &f) const;
    std::size_t len(std::string_view base) const;

  private:
    std::unordered_map<std::string, std::string> map_;
};

} // namespace xpute
