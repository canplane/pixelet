// cpp/gpu/encode.cpp

#include "encode.hpp"

namespace xpute::gpu::call {

using detail::emit;
using detail::f;
using detail::flag;
using detail::opt;
using detail::u;

static bool words(Stream &stream, std::uint32_t op, std::initializer_list<std::uint32_t> w) noexcept {
    return emit(stream, op, [&](auto &s) { detail::words(s, w); });
}

static std::uint32_t off(Stream &stream, std::span<const std::uint8_t> bytes) noexcept { return stream.memory().off_of(bytes); }

bool write_buffer(Stream &stream, Id buffer, std::uint32_t offset, std::span<const std::uint8_t> bytes) noexcept {
    return emit(stream, op::WRITE_BUFFER, [&](auto &s) {
        detail::words(s, {buffer, offset});
        s.bytes(bytes);
    });
}

bool write_buffer_at(Stream &stream, Id buffer, std::uint32_t offset, std::span<const std::uint8_t> bytes) noexcept {
    return words(stream, op::WRITE_BUFFER_AT, {buffer, offset, off(stream, bytes), static_cast<std::uint32_t>(bytes.size())});
}

template <class S> static void texture_write(S &s, const TexelCopyTexture &place, std::array<std::uint32_t, 2> layout, std::array<std::uint32_t, 3> size) noexcept {
    detail::texture_place(s, place);
    detail::words(s, {layout[0], layout[1]});
    detail::extent(s, size);
}

bool write_texture(Stream &stream, const TexelCopyTexture &place, std::array<std::uint32_t, 2> layout, std::array<std::uint32_t, 3> size, std::span<const std::uint8_t> bytes) noexcept {
    return emit(stream, op::WRITE_TEXTURE, [&](auto &s) {
        texture_write(s, place, layout, size);
        s.bytes(bytes);
    });
}

bool write_texture_at(Stream &stream, const TexelCopyTexture &place, std::array<std::uint32_t, 2> layout, std::array<std::uint32_t, 3> size, std::span<const std::uint8_t> bytes) noexcept {
    std::uint32_t at = off(stream, bytes);
    return emit(stream, op::WRITE_TEXTURE_AT, [&](auto &s) {
        texture_write(s, place, layout, size);
        detail::words(s, {at, static_cast<std::uint32_t>(bytes.size())});
    });
}

bool copy_external_image(Stream &stream, const TexelCopyTexture &to, bool flip_y, bool premultiplied, std::span<const std::uint8_t> image, std::uint32_t &word, std::uint32_t ticket) noexcept {
    word = ticket;
    std::uint32_t image_at = off(stream, image), word_at = stream.memory().off_of(&word);
    return emit(stream, op::COPY_EXTERNAL_IMAGE, [&](auto &s) {
        detail::texture_place(s, to);
        detail::words(s, {flag(flip_y), flag(premultiplied), image_at, static_cast<std::uint32_t>(image.size()), word_at, ticket});
    });
}

bool copy_buffer_to_buffer(Stream &stream, Id encoder, std::array<std::uint32_t, 2> from, std::array<std::uint32_t, 2> to, std::uint32_t size) noexcept {
    return words(stream, op::COPY_BUFFER_TO_BUFFER, {encoder, from[0], from[1], to[0], to[1], size});
}

bool copy_buffer_to_texture(Stream &stream, Id encoder, const TexelCopyBuffer &from, const TexelCopyTexture &to, std::array<std::uint32_t, 3> size) noexcept {
    return emit(stream, op::COPY_BUFFER_TO_TEXTURE, [&](auto &s) {
        s.word(encoder);
        detail::buffer_layout(s, from);
        detail::texture_place(s, to);
        detail::extent(s, size);
    });
}

bool copy_texture_to_buffer(Stream &stream, Id encoder, const TexelCopyTexture &from, const TexelCopyBuffer &to, std::array<std::uint32_t, 3> size) noexcept {
    return emit(stream, op::COPY_TEXTURE_TO_BUFFER, [&](auto &s) {
        s.word(encoder);
        detail::texture_place(s, from);
        detail::buffer_layout(s, to);
        detail::extent(s, size);
    });
}

bool copy_texture_to_texture(Stream &stream, Id encoder, const TexelCopyTexture &from, const TexelCopyTexture &to, std::array<std::uint32_t, 3> size) noexcept {
    return emit(stream, op::COPY_TEXTURE_TO_TEXTURE, [&](auto &s) {
        s.word(encoder);
        detail::texture_place(s, from);
        detail::texture_place(s, to);
        detail::extent(s, size);
    });
}

bool clear_buffer(Stream &stream, Id encoder, Id buffer, std::uint32_t offset, std::optional<std::uint32_t> size) noexcept {
    return words(stream, op::CLEAR_BUFFER, {encoder, buffer, offset, opt(size)});
}

bool resolve_query_set(Stream &stream, Id encoder, Id query_set, std::uint32_t first, std::uint32_t count, Id buffer, std::uint32_t offset) noexcept {
    return words(stream, op::RESOLVE_QUERY_SET, {encoder, query_set, first, count, buffer, offset});
}

bool set_pipeline(Stream &stream, Id pass, Id pipeline) noexcept { return words(stream, op::SET_PIPELINE, {pass, pipeline}); }

bool set_bind_group(Stream &stream, Id pass, std::uint32_t index, Id group, std::span<const std::uint32_t> dynamic_offsets) noexcept {
    return emit(stream, op::SET_BIND_GROUP, [&](auto &s) {
        detail::words(s, {pass, index, group, static_cast<std::uint32_t>(dynamic_offsets.size())});
        for (std::uint32_t o : dynamic_offsets) s.word(o);
    });
}

bool set_vertex_buffer(Stream &stream, Id pass, std::uint32_t slot, Id buffer, std::uint32_t offset, std::optional<std::uint32_t> size) noexcept {
    return words(stream, op::SET_VERTEX_BUFFER, {pass, slot, buffer, offset, opt(size)});
}

bool set_index_buffer(Stream &stream, Id pass, Id buffer, IndexFormat format, std::uint32_t offset, std::optional<std::uint32_t> size) noexcept {
    return words(stream, op::SET_INDEX_BUFFER, {pass, buffer, u(format), offset, opt(size)});
}

bool set_viewport(Stream &stream, Id pass, std::array<float, 4> rect, std::array<float, 2> depth) noexcept {
    return words(stream, op::SET_VIEWPORT, {pass, f(rect[0]), f(rect[1]), f(rect[2]), f(rect[3]), f(depth[0]), f(depth[1])});
}

bool set_scissor(Stream &stream, Id pass, std::array<std::uint32_t, 4> rect) noexcept { return words(stream, op::SET_SCISSOR, {pass, rect[0], rect[1], rect[2], rect[3]}); }

bool set_blend_constant(Stream &stream, Id pass, std::array<float, 4> rgba) noexcept {
    return words(stream, op::SET_BLEND_CONSTANT, {pass, f(rgba[0]), f(rgba[1]), f(rgba[2]), f(rgba[3])});
}

bool set_stencil_reference(Stream &stream, Id pass, std::uint32_t reference) noexcept { return words(stream, op::SET_STENCIL_REFERENCE, {pass, reference}); }

bool draw(Stream &stream, Id pass, std::uint32_t vertices, std::uint32_t instances, std::uint32_t first_vertex, std::uint32_t first_instance) noexcept {
    return words(stream, op::DRAW, {pass, vertices, instances, first_vertex, first_instance});
}

bool draw_indexed(Stream &stream, Id pass, std::uint32_t indices, std::uint32_t instances, std::uint32_t first_index, std::int32_t base_vertex, std::uint32_t first_instance) noexcept {
    return words(stream, op::DRAW_INDEXED, {pass, indices, instances, first_index, static_cast<std::uint32_t>(base_vertex), first_instance});
}

bool draw_indirect(Stream &stream, Id pass, Id buffer, std::uint32_t offset) noexcept { return words(stream, op::DRAW_INDIRECT, {pass, buffer, offset}); }

bool draw_indexed_indirect(Stream &stream, Id pass, Id buffer, std::uint32_t offset) noexcept { return words(stream, op::DRAW_INDEXED_INDIRECT, {pass, buffer, offset}); }

bool dispatch(Stream &stream, Id pass, std::array<std::uint32_t, 3> workgroups) noexcept {
    return words(stream, op::DISPATCH, {pass, workgroups[0], workgroups[1], workgroups[2]});
}

bool dispatch_indirect(Stream &stream, Id pass, Id buffer, std::uint32_t offset) noexcept { return words(stream, op::DISPATCH_INDIRECT, {pass, buffer, offset}); }

bool read_buffer(Stream &stream, Id buffer, std::uint32_t offset, std::span<std::uint8_t> into, std::uint32_t &word, std::uint32_t ticket) noexcept {
    word = ticket;
    Memory mem = stream.memory();
    return words(stream, op::READ_BUFFER, {buffer, offset, static_cast<std::uint32_t>(into.size()), mem.off_of(into), mem.off_of(&word), ticket});
}

bool queue_done(Stream &stream, std::uint32_t &word, std::uint32_t ticket) noexcept {
    word = ticket;
    return words(stream, op::QUEUE_DONE, {stream.memory().off_of(&word), ticket});
}

bool configure(Stream &stream, std::uint32_t surface, std::optional<TextureFormat> format, std::uint32_t usage, CanvasAlphaMode alpha) noexcept {
    return words(stream, op::CONFIGURE, {surface, format ? u(*format) : SURFACE_PREFERRED, usage, u(alpha)});
}

bool resize(Stream &stream, std::uint32_t surface, std::array<std::uint32_t, 2> size) noexcept { return words(stream, op::RESIZE, {surface, size[0], size[1]}); }

bool device(Stream &stream, std::span<std::uint32_t> into) noexcept {
    return words(stream, op::DEVICE, {stream.memory().off_of(into), static_cast<std::uint32_t>(into.size())});
}

} // namespace xpute::gpu::call
