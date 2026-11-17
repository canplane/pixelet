// cpp/xpute/link/abi/types.hpp
//
// GENERATED from link/wit (xpute-link-gen wit.rs)

#pragma once

#include <cstddef>
#include <cstdint>
#include <string>
#include <variant>

#include "xpute/link/cabi.hpp"

namespace xpute::abi::link::types {

namespace _rt {
using ::xpute::link::Lift;
using ::xpute::link::Lower;
} // namespace _rt

struct Token {
    struct Int {
        std::int32_t val;
    };
    struct Str {
        std::string val;
    };

    std::variant<Int, Str> variant;

    static constexpr std::size_t SZ = 12;
    static constexpr std::size_t ALIGN = 4;

    void store(_rt::Lower &w, std::size_t at) const noexcept {
        switch (this->variant.index()) {
        case 0: w.put<std::uint8_t>(at, 0); w.put<std::int32_t>(at + 4, std::get<0>(this->variant).val); break;
        case 1: w.put<std::uint8_t>(at, 1); w.str(at + 4, std::get<1>(this->variant).val); break;
        }
    }

    static Token load(_rt::Lift &r, std::size_t at) noexcept {
        switch (r.case_(at, 2)) {
        case 0: return Token{Int{r.get<std::int32_t>(at + 4)}};
        case 1: return Token{Str{std::string(r.str(at + 4))}};
        }
        return {};
    }
};

struct Lent {
    std::uint64_t buf;
    std::uint32_t nbyte;

    static constexpr std::size_t SZ = 16;
    static constexpr std::size_t ALIGN = 8;

    void store(_rt::Lower &w, std::size_t at) const noexcept {
        w.put<std::uint64_t>(at, this->buf);
        w.put<std::uint32_t>(at + 8, this->nbyte);
    }

    static Lent load(_rt::Lift &r, std::size_t at) noexcept {
        return Lent{
            .buf = r.get<std::uint64_t>(at),
            .nbyte = r.get<std::uint32_t>(at + 8),
        };
    }
};

} // namespace xpute::abi::link::types
