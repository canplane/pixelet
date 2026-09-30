// cpp/xpute/runtime/mem/memory.hpp

// The memory both sides see, as the guest holds it: where it begins, and
// words read and written at offsets into it. Every address that crosses is an
// offset (ARCHITECTURE.xpute.md, the contract's rule 1), so this is the one
// place one becomes a pointer. A word is copied rather than dereferenced in
// place: an offset the other side names carries no alignment a compiler may
// assume.

#pragma once

#include <cstdint>
#include <cstring>

namespace xpute {

class Memory {
  public:
    explicit Memory(std::uint8_t *base) noexcept : base_(base) {}

    std::uint8_t *at(std::uint32_t off) const noexcept { return base_ + off; }

    std::uint32_t word(std::uint32_t off) const noexcept {
        std::uint32_t w;
        std::memcpy(&w, base_ + off, sizeof w);
        return w;
    }

    void set_word(std::uint32_t off, std::uint32_t w) const noexcept { std::memcpy(base_ + off, &w, sizeof w); }

  private:
    std::uint8_t *base_;
};

} // namespace xpute
