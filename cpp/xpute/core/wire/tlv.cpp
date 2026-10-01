// cpp/xpute/core/wire/tlv.cpp

#include "tlv.hpp"

namespace xpute::tlv {

Result<std::optional<Value>> Reader::next() noexcept {
    if (end_) return std::optional<Value>();
    if (off_ >= buf_.size()) {
        end_ = End::unclosed;
        return std::optional<Value>();
    }
    auto tag = static_cast<Tag>(buf_[off_++]);
    auto scalar = [this]<class T>() -> Result<std::optional<Value>> {
        if (!has(sizeof(T))) return marshal_error(Errno::ebadmsg);
        return std::optional<Value>(le<T>());
    };
    auto run = [this]() -> Result<std::span<const std::uint8_t>> {
        if (!has(4)) return marshal_error(Errno::ebadmsg);
        std::uint32_t len = le<std::uint32_t>();
        if (!has(len)) return marshal_error(Errno::ebadmsg);
        std::span<const std::uint8_t> bytes = buf_.subspan(off_, len);
        off_ += len;
        return bytes;
    };
    switch (tag) {
    case Tag::end:
        end_ = End::closed;
        return std::optional<Value>();
    case Tag::boolean: {
        if (!has(1)) return marshal_error(Errno::ebadmsg);
        return std::optional<Value>(buf_[off_++] != 0);
    }
    case Tag::u8:
        return scalar.operator()<std::uint8_t>();
    case Tag::i8:
        return scalar.operator()<std::int8_t>();
    case Tag::u16:
        return scalar.operator()<std::uint16_t>();
    case Tag::i16:
        return scalar.operator()<std::int16_t>();
    case Tag::u32:
        return scalar.operator()<std::uint32_t>();
    case Tag::i32:
        return scalar.operator()<std::int32_t>();
    case Tag::u64:
        return scalar.operator()<std::uint64_t>();
    case Tag::i64:
        return scalar.operator()<std::int64_t>();
    case Tag::f32:
        return scalar.operator()<float>();
    case Tag::f64:
        return scalar.operator()<double>();
    case Tag::str: {
        Result<std::span<const std::uint8_t>> r = run();
        if (!r) return r.error();
        return std::optional<Value>(Str{std::string_view(reinterpret_cast<const char *>(r->data()), r->size())});
    }
    case Tag::bytes: {
        Result<std::span<const std::uint8_t>> r = run();
        if (!r) return r.error();
        return std::optional<Value>(Bytes{*r});
    }
    }
    return marshal_error(Errno::ebadmsg);
}

} // namespace xpute::tlv
