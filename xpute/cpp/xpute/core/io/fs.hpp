// cpp/xpute/core/io/fs.hpp

// This header holds the core's end of the file device (abi/fs.json): a file's
// path, a descriptor, and what its completions carry. The file calls are
// `Door`'s (io/door.hpp), and `FileTable` keeps the owner of each open
// (io/file.hpp).

#pragma once

#include <cstdint>
#include <optional>
#include <span>
#include <string_view>
#include <variant>

#include "xpute/kit/status/bug.hpp"
#include "xpute/kit/status/error.hpp"
#include "xpute/kit/wire/xtp/cursor.hpp"
#include "xpute/kit/wire/xtp/encode.hpp"
#include "xpute/core/abi/number.hpp"
#include "fs.spec.hpp"

namespace xpute {

using Fd = std::uint32_t;

// A token of a file's path: an i32 or a string, each of which compares
// exactly. xpute gives no token a meaning.
using Token = std::variant<std::int32_t, std::string_view>;

// A file's path as a packet carries it, a branch of its tokens, read in place.
// Each token was checked when the path was read.
class Path {
  public:
    Path() noexcept = default;

    // Reads the path at `c`.
    static Result<Path> read(xtp::Cursor c) noexcept {
        Result<xtp::BranchCursor> b = c.as_branch();
        if (!b) return b.error();
        for (Result<xtp::Cursor> child : *b) {
            if (!child) return child.error();
            Result<Token> t = token_of(*child);
            if (!t) return t.error();
        }
        Path p;
        p.branch_ = *b;
        return p;
    }

    // Writes `tokens` as a path: a branch of an i32 or a string each.
    static void write(xtp::PacketWriter &w, std::span<const Token> tokens) noexcept {
        w.branch(static_cast<std::uint32_t>(tokens.size()), [&](xtp::PacketWriter &w) {
            for (const Token &t : tokens) {
                if (const std::int32_t *v = std::get_if<std::int32_t>(&t)) {
                    w.i32(*v);
                } else {
                    w.str(std::get<std::string_view>(t));
                }
            }
        });
    }

    std::uint32_t len() const noexcept { return branch_ ? branch_->len() : 0; }

    Token at(std::uint32_t i) const noexcept {
        xtp::Cursor c = or_bug(branch_->at(i), Errno::enotrecoverable);
        return or_bug(token_of(c), Errno::enotrecoverable);
    }

  private:
    // Reads a branch's child: a string, or a number whose value is a whole
    // i32, as a whole float's is. Any other child, a fraction, a bool, a
    // branch or an integer past an i32, is EBADMSG.
    static Result<Token> token_of(const xtp::Cursor &c) noexcept {
        if (Result<std::string_view> s = c.get_str_ref()) return Token{*s};
        Result<double> n = c.get_number();
        if (!n) return marshal_error(Errno::ebadmsg);
        Result<std::int32_t> v = abi::integer<std::int32_t>(*n);
        if (!v) return marshal_error(Errno::ebadmsg);
        return Token{*v};
    }

    std::optional<xtp::BranchCursor> branch_;
};

} // namespace xpute
