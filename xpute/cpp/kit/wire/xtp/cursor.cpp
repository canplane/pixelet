// cpp/kit/wire/xtp/cursor.cpp

#include "cursor.hpp"

namespace xpute::xtp {

template <class T> Result<T> Cursor::get_as() const noexcept {
    Result<Value> v = get();
    if (!v) return v.error();
    if (const T *x = std::get_if<T>(&*v)) return *x;
    return marshal_error(Errno::ebadmsg);
}

template <class T> Result<Value> Cursor::scalar() const noexcept {
    if (std::uint64_t{base_} + sizeof(T) > pkt_.size()) return marshal_error(Errno::ebadmsg);
    return Value(load<T>(pkt_.data() + base_));
}

Result<Cursor> Cursor::at(std::span<const std::uint8_t> pkt, std::uint32_t base, NodeType type) noexcept {
    Result<std::uint32_t> unit = align_sz(type);
    if (!unit) return unit.error();
    if (base % *unit != 0) return marshal_error(Errno::ebadmsg);
    if (type != type_of(SpecialType::branch)) return Cursor(pkt, base, type);
    if (std::uint64_t{base} + WORD_SZ > pkt.size()) return marshal_error(Errno::ebadmsg);
    Result<std::uint32_t> len = get_len(pkt, base);
    if (!len) return len.error();
    if (*len > (pkt.size() - base - WORD_SZ) / WORD_SZ) return marshal_error(Errno::ebadmsg);
    return Cursor(pkt, base, type, *len);
}

Result<BranchCursor> Cursor::as_branch() const noexcept {
    if (is_null()) return marshal_error(Errno::efault);
    if (!is_branch()) return marshal_error(Errno::ebadmsg);
    return BranchCursor(*this);
}

Result<Array> Cursor::array(std::uint32_t elem) const noexcept {
    std::uint64_t payload = std::uint64_t{base_} + WORD_SZ;
    if (payload > pkt_.size()) return marshal_error(Errno::ebadmsg);
    Result<std::uint32_t> len = get_len(pkt_, base_);
    if (!len) return len.error();
    std::uint64_t bytes = std::uint64_t{*len} * elem;
    if (bytes > pkt_.size() - payload) return marshal_error(Errno::ebadmsg);
    return Array{static_cast<SequenceType>(type_), pkt_.subspan(payload, bytes)};
}

Result<Bitset> Cursor::bitset() const noexcept {
    std::uint64_t payload = std::uint64_t{base_} + WORD_SZ;
    if (payload > pkt_.size()) return marshal_error(Errno::ebadmsg);
    Result<std::uint32_t> len = get_len(pkt_, base_);
    if (!len) return len.error();
    std::uint64_t bytes = (std::uint64_t{*len} + 7) >> 3;
    if (bytes > pkt_.size() - payload) return marshal_error(Errno::ebadmsg);
    return Bitset{pkt_.subspan(payload, bytes), *len};
}

// The wire carries a NUL after the bytes, which the length leaves out and a
// string that reads must have; the bytes are UTF-8 or refused.
Result<std::string_view> Cursor::str() const noexcept {
    std::uint64_t payload = std::uint64_t{base_} + WORD_SZ;
    if (payload > pkt_.size()) return marshal_error(Errno::ebadmsg);
    Result<std::uint32_t> nbyte = get_len(pkt_, base_);
    if (!nbyte) return nbyte.error();
    if (std::uint64_t{*nbyte} + 1 > pkt_.size() - payload) return marshal_error(Errno::ebadmsg);
    if (pkt_[payload + *nbyte] != 0) return marshal_error(Errno::ebadmsg);
    std::string_view s(reinterpret_cast<const char *>(pkt_.data() + payload), *nbyte);
    if (!is_utf8(s)) return marshal_error(Errno::ebadmsg);
    return s;
}

Result<std::string_view> Cursor::get_str_ref() const noexcept {
    if (is_null() || type_ != type_of(SequenceType::str)) return marshal_error(Errno::ebadmsg);
    return str();
}

Result<Strs> Cursor::strs() const noexcept {
    std::uint64_t payload = std::uint64_t{base_} + WORD_SZ;
    if (payload > pkt_.size()) return marshal_error(Errno::ebadmsg);
    Result<std::uint32_t> len = get_len(pkt_, base_);
    if (!len) return len.error();
    std::uint32_t n = *len;
    std::uint64_t blob = payload + 4 * (std::uint64_t{n} + 1);
    if (blob > pkt_.size()) return marshal_error(Errno::ebadmsg);
    auto off = [&](std::uint32_t i) { return load<std::uint32_t>(pkt_.data() + payload + 4 * i); };
    std::uint32_t total = off(n);
    if (total > pkt_.size() - blob) return marshal_error(Errno::ebadmsg);
    for (std::uint32_t i = 0; i < n; i++) {
        if (off(i) > off(i + 1) || off(i + 1) > total) return marshal_error(Errno::ebadmsg);
        if (!is_utf8(std::string_view(reinterpret_cast<const char *>(pkt_.data() + blob + off(i)), off(i + 1) - off(i)))) return marshal_error(Errno::ebadmsg);
    }
    return Strs{pkt_.subspan(payload, 4 * (std::uint64_t{n} + 1)), pkt_.subspan(blob, total), n};
}

Result<Strs> Cursor::get_strs() const noexcept {
    if (is_null() || type_ != type_of(SequenceType::strs)) return marshal_error(Errno::ebadmsg);
    return strs();
}

Result<Value> Cursor::get() const noexcept {
    if (is_null()) return Value(Null{});
    if (is_branch()) return Value(BranchCursor(*this));
    auto seq = [](auto r) -> Result<Value> {
        if (!r) return r.error();
        return Value(*r);
    };
    switch (static_cast<ScalarType>(type_)) {
    case ScalarType::u8:
        return scalar<std::uint8_t>();
    case ScalarType::i8:
        return scalar<std::int8_t>();
    case ScalarType::u16:
        return scalar<std::uint16_t>();
    case ScalarType::i16:
        return scalar<std::int16_t>();
    case ScalarType::u32:
        return scalar<std::uint32_t>();
    case ScalarType::i32:
        return scalar<std::int32_t>();
    case ScalarType::u64:
        return scalar<std::uint64_t>();
    case ScalarType::i64:
        return scalar<std::int64_t>();
    case ScalarType::f32:
        return scalar<float>();
    case ScalarType::f64:
        return scalar<double>();
    case ScalarType::boolean: {
        Result<Value> b = scalar<std::uint8_t>();
        if (!b) return b;
        return Value(std::get<std::uint8_t>(*b) != 0);
    }
    }
    switch (static_cast<SequenceType>(type_)) {
    case SequenceType::u8_array:
    case SequenceType::i8_array:
    case SequenceType::u16_array:
    case SequenceType::i16_array:
    case SequenceType::u32_array:
    case SequenceType::i32_array:
    case SequenceType::u64_array:
    case SequenceType::i64_array:
    case SequenceType::f32_array:
    case SequenceType::f64_array:
        return seq(array(elem_sz(static_cast<SequenceType>(type_))));
    case SequenceType::bitset:
        return seq(bitset());
    case SequenceType::str:
        return seq(str());
    case SequenceType::strs:
        return seq(strs());
    }
    return marshal_error(Errno::ebadmsg);
}

Result<std::uint64_t> Cursor::get_u64() const noexcept {
    return get_as<std::uint64_t>();
}
Result<std::int64_t> Cursor::get_i64() const noexcept {
    return get_as<std::int64_t>();
}
Result<bool> Cursor::get_bool() const noexcept {
    return get_as<bool>();
}

Result<double> Cursor::get_number() const noexcept {
    Result<Value> v = get();
    if (!v) return v.error();
    return std::visit(
        [](const auto &x) -> Result<double> {
            using T = std::decay_t<decltype(x)>;
            if constexpr (std::is_arithmetic_v<T> && !std::is_same_v<T, bool> && sizeof(T) <= 4) return static_cast<double>(x);
            else if constexpr (std::is_same_v<T, double>) return x;
            else return marshal_error(Errno::ebadmsg);
        },
        *v);
}

Result<Cursor> BranchCursor::at(std::uint32_t idx) const noexcept {
    if (idx >= len()) return marshal_error(Errno::efault);
    Result<Desc> d = get_desc(node_.pkt_, base() + WORD_SZ + idx * WORD_SZ);
    if (!d) return d.error();
    auto [rel_off, type] = *d;
    if (rel_off == 0) return Cursor(node_.pkt_, 0, type);
    if (rel_off >= node_.pkt_.size() - base()) return marshal_error(Errno::ebadmsg);
    std::uint32_t child = base() + rel_off;
    std::uint32_t end = end_of(idx);
    if (child < base() + WORD_SZ + len() * WORD_SZ || child >= end) return marshal_error(Errno::ebadmsg);
    return Cursor::at(node_.pkt_.first(end), child, type);
}

std::uint32_t BranchCursor::end_of(std::uint32_t idx) const noexcept {
    std::uint64_t end = node_.pkt_.size();
    for (std::uint32_t j = idx + 1; j < len(); j++) {
        std::uint32_t rel_off = get_word(node_.pkt_, base() + WORD_SZ + j * WORD_SZ)[0];
        if (rel_off != 0) return static_cast<std::uint32_t>(std::min(std::uint64_t{base()} + rel_off, end));
    }
    return static_cast<std::uint32_t>(end);
}

Result<BranchCursor> BranchCursor::at_branch(std::uint32_t idx) const noexcept {
    Result<Cursor> c = at(idx);
    if (!c) return c.error();
    return c->as_branch();
}

Result<TreeReader> TreeReader::make(std::span<const std::uint8_t> pkt) noexcept {
    if (pkt.size() < HDR_SZ) return marshal_error(Errno::ebadmsg);
    Result<std::uint32_t> magic = get_len(pkt, 0);
    if (!magic || *magic != MAGIC) return marshal_error(Errno::ebadmsg);
    Result<Desc> d = get_desc(pkt, WORD_SZ);
    if (!d) return d.error();
    auto [payload_sz, type] = *d;
    if (payload_sz > pkt.size() - HDR_SZ) return marshal_error(Errno::ebadmsg);
    if (payload_sz == 0) return TreeReader(Cursor(pkt, 0, type));
    Result<Cursor> root = Cursor::at(pkt.first(HDR_SZ + payload_sz), HDR_SZ, type);
    if (!root) return root.error();
    return TreeReader(*root);
}

Result<Value> BranchCursor::get_at(std::uint32_t idx) const noexcept {
    Result<Cursor> c = at(idx);
    if (!c) return c.error();
    return c->get();
}

} // namespace xpute::xtp
