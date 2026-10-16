// cpp/gpu/encode.hpp

// A guest's GPU calls written as a stream's records (ipc/stream), in the
// words spec/gpu/api.json gives each op; rust/gpu/src/encode.rs writes the
// same words for the same calls. Objects are handles from one table; a
// record that does not fit an open transaction is not written, and an object
// it would have made is handed back.
//
// Every record is written twice by the same function: once to count its
// words, then into the room the stream reserved for them, so nothing is
// built on the heap first.

#pragma once

#include <array>
#include <cstddef>
#include <cstdint>
#include <cstring>
#include <optional>
#include <span>
#include <string_view>
#include <variant>

#include "../guest/abi/handle.hpp"
#include "../guest/ipc/stream.hpp"
#include "../kit/status/bug.hpp"
#include "api.spec.hpp"
#include "enum.spec.hpp"

namespace xpute::gpu {

// An object the guest made: a handle of the table its encoder holds.
using Id = std::uint32_t;

struct BufferDescriptor {
    std::uint32_t size;
    // BUFFER_USAGE bits.
    std::uint32_t usage;
    bool mapped_at_creation;
};

struct TextureDescriptor {
    // Width, height, and depth or layers.
    std::array<std::uint32_t, 3> size;
    std::uint32_t mip_levels;
    std::uint32_t samples;
    TextureDimension dimension;
    TextureFormat format;
    // TEXTURE_USAGE bits.
    std::uint32_t usage;
    std::span<const TextureFormat> view_formats;
};

struct ViewDescriptor {
    std::optional<TextureFormat> format;
    std::optional<TextureViewDimension> dimension;
    std::optional<TextureAspect> aspect;
    std::uint32_t base_mip;
    std::optional<std::uint32_t> mips;
    std::uint32_t base_layer;
    std::optional<std::uint32_t> layers;
};

struct SamplerDescriptor {
    std::array<AddressMode, 3> address;
    FilterMode mag;
    FilterMode min;
    MipmapFilterMode mipmap;
    std::array<float, 2> lod;
    std::optional<CompareFunction> compare;
    std::uint32_t max_anisotropy;
};

struct BufferBinding {
    BufferBindingType ty;
    bool dynamic_offset;
    std::uint32_t min_binding_size;
};

struct SamplerBinding {
    SamplerBindingType ty;
};

struct TextureBinding {
    TextureSampleType sample;
    TextureViewDimension dimension;
    bool multisampled;
};

struct StorageTextureBinding {
    StorageTextureAccess access;
    TextureFormat format;
    TextureViewDimension dimension;
};

using BindingLayout = std::variant<BufferBinding, SamplerBinding, TextureBinding, StorageTextureBinding>;

struct BindGroupLayoutEntry {
    std::uint32_t binding;
    // SHADER_STAGE bits.
    std::uint32_t visibility;
    BindingLayout layout;
};

struct BufferResource {
    Id buffer;
    std::uint32_t offset;
    std::optional<std::uint32_t> size;
};

struct SamplerResource {
    Id sampler;
};

struct ViewResource {
    Id view;
};

using Resource = std::variant<BufferResource, SamplerResource, ViewResource>;

struct BindGroupEntry {
    std::uint32_t binding;
    Resource resource;
};

struct VertexAttribute {
    VertexFormat format;
    std::uint32_t offset;
    std::uint32_t location;
};

struct VertexBufferLayout {
    std::uint32_t stride;
    VertexStepMode step;
    std::span<const VertexAttribute> attributes;
};

struct VertexState {
    Id module;
    std::string_view entry;
    std::span<const VertexBufferLayout> buffers;
};

struct PrimitiveState {
    PrimitiveTopology topology;
    std::optional<IndexFormat> strip_index_format;
    FrontFace front_face;
    CullMode cull;
    bool unclipped_depth;
};

// Defaulted to what WebGPU defaults a face to: always, keep, keep, keep
// (STENCIL_UNUSED in Rust).
struct StencilFaceState {
    CompareFunction compare = CompareFunction::always;
    StencilOperation fail = StencilOperation::keep;
    StencilOperation depth_fail = StencilOperation::keep;
    StencilOperation pass = StencilOperation::keep;
};

struct DepthStencilState {
    TextureFormat format;
    bool depth_write;
    CompareFunction depth_compare;
    StencilFaceState stencil_front;
    StencilFaceState stencil_back;
    std::uint32_t stencil_read_mask;
    std::uint32_t stencil_write_mask;
    std::int32_t bias;
    float slope_scale;
    float clamp;
};

// Defaulted to one sample, every sample's bit set (MULTISAMPLE_NONE in
// Rust).
struct MultisampleState {
    std::uint32_t count = 1;
    std::uint32_t mask = UINT32_MAX;
    bool alpha_to_coverage = false;
};

struct BlendComponent {
    BlendOperation operation;
    BlendFactor src;
    BlendFactor dst;
};

struct BlendState {
    BlendComponent color;
    BlendComponent alpha;
};

struct ColorTargetState {
    TextureFormat format;
    std::optional<BlendState> blend;
    // COLOR_WRITE bits.
    std::uint32_t write_mask;
};

struct FragmentState {
    Id module;
    std::string_view entry;
    std::span<const ColorTargetState> targets;
};

struct RenderPipelineDescriptor {
    // None for a layout the device makes from the shaders.
    std::optional<Id> layout;
    VertexState vertex;
    PrimitiveState primitive;
    std::optional<DepthStencilState> depth_stencil;
    MultisampleState multisample;
    std::optional<FragmentState> fragment;
};

struct ComputePipelineDescriptor {
    std::optional<Id> layout;
    Id module;
    std::string_view entry;
};

// A texture's place a copy reads or writes.
struct TexelCopyTexture {
    Id texture;
    std::uint32_t mip;
    std::array<std::uint32_t, 3> origin;
    TextureAspect aspect;
};

// A buffer's bytes laid out as texels.
struct TexelCopyBuffer {
    Id buffer;
    std::uint32_t offset;
    std::uint32_t bytes_per_row;
    std::uint32_t rows_per_image;
};

struct ColorAttachment {
    Id view;
    std::optional<Id> resolve;
    LoadOp load;
    StoreOp store;
    std::array<float, 4> clear;
};

struct DepthStencilAttachment {
    Id view;
    LoadOp depth_load;
    StoreOp depth_store;
    float depth_clear;
    bool depth_read_only;
    LoadOp stencil_load;
    StoreOp stencil_store;
    std::uint32_t stencil_clear;
    bool stencil_read_only;
};

struct TimestampWrites {
    Id query_set;
    std::optional<std::uint32_t> begin;
    std::optional<std::uint32_t> end;
};

struct RenderPassDescriptor {
    std::span<const ColorAttachment> colors;
    std::optional<DepthStencilAttachment> depth_stencil;
    std::optional<TimestampWrites> timestamps;
    std::optional<Id> occlusion;
};

namespace detail {

// A record's words counted.
struct Count {
    std::uint32_t n = 0;
    void word(std::uint32_t) noexcept { n++; }
    // A count, then the bytes padded to words.
    void bytes(std::span<const std::uint8_t> b) noexcept { n += 1 + static_cast<std::uint32_t>((b.size() + WORD - 1) / WORD); }
};

// A record's words written where the stream reserved them.
struct Fill {
    Memory mem;
    std::uint32_t at;
    std::uint32_t n = 0;
    void word(std::uint32_t v) noexcept { mem.set_word(at + WORD * n++, v); }
    void bytes(std::span<const std::uint8_t> b) noexcept {
        word(static_cast<std::uint32_t>(b.size()));
        for (std::size_t k = 0; k < b.size(); k += WORD) {
            std::uint32_t w = 0;
            std::memcpy(&w, b.data() + k, b.size() - k < WORD ? b.size() - k : WORD);
            word(w);
        }
    }
};

inline std::uint32_t f(float v) noexcept {
    std::uint32_t w;
    std::memcpy(&w, &v, sizeof w);
    return w;
}

template <class T> std::uint32_t opt(std::optional<T> v) noexcept {
    return v ? static_cast<std::uint32_t>(*v) : NONE;
}

inline std::uint32_t flag(bool v) noexcept { return v ? 1 : 0; }

template <class E> std::uint32_t u(E e) noexcept { return static_cast<std::uint32_t>(e); }

inline std::span<const std::uint8_t> text(std::string_view s) noexcept { return {reinterpret_cast<const std::uint8_t *>(s.data()), s.size()}; }

template <class S> void words(S &s, std::initializer_list<std::uint32_t> w) noexcept {
    for (std::uint32_t v : w) s.word(v);
}

// Writes a record of `op` whose words `put` gives; false where an open
// transaction no longer fits it.
template <class F> bool emit(Stream &stream, std::uint32_t op, F &&put) noexcept {
    Count count;
    put(count);
    std::optional<std::uint32_t> at = stream.reserve(op, count.n);
    if (!at) return false;
    Fill fill{stream.memory(), *at};
    put(fill);
    ensure(fill.n == count.n, Errno::enotrecoverable, op);
    return true;
}

template <class S> void timestamps(S &s, const std::optional<TimestampWrites> &t) noexcept {
    if (t) words(s, {t->query_set, opt(t->begin), opt(t->end)});
    else words(s, {NONE, NONE, NONE});
}

template <class S> void texture_place(S &s, const TexelCopyTexture &t) noexcept {
    words(s, {t.texture, t.mip, t.origin[0], t.origin[1], t.origin[2], u(t.aspect)});
}

template <class S> void buffer_layout(S &s, const TexelCopyBuffer &b) noexcept { words(s, {b.buffer, b.offset, b.bytes_per_row, b.rows_per_image}); }

template <class S> void extent(S &s, std::array<std::uint32_t, 3> e) noexcept { words(s, {e[0], e[1], e[2]}); }

template <class S> void stencil_face(S &s, const StencilFaceState &f) noexcept { words(s, {u(f.compare), u(f.fail), u(f.depth_fail), u(f.pass)}); }

} // namespace detail

// The calls, over a table of `N` handles.
template <std::size_t N> class Gpu {
  public:
    std::optional<Id> buffer(Stream &stream, const BufferDescriptor &d) noexcept {
        return make(stream, op::BUFFER, [&](auto &s) { detail::words(s, {d.size, d.usage, detail::flag(d.mapped_at_creation)}); });
    }

    std::optional<Id> texture(Stream &stream, const TextureDescriptor &d) noexcept {
        using detail::u;
        return make(stream, op::TEXTURE, [&](auto &s) {
            detail::words(s, {d.size[0], d.size[1], d.size[2], d.mip_levels, d.samples, u(d.dimension), u(d.format), d.usage, static_cast<std::uint32_t>(d.view_formats.size())});
            for (TextureFormat f : d.view_formats) s.word(u(f));
        });
    }

    std::optional<Id> view(Stream &stream, Id texture, const ViewDescriptor &d) noexcept {
        using detail::opt;
        return make(stream, op::VIEW, [&](auto &s) {
            detail::words(s, {texture, opt(d.format), opt(d.dimension), detail::u(d.aspect.value_or(TextureAspect::all)), d.base_mip, opt(d.mips), d.base_layer, opt(d.layers)});
        });
    }

    std::optional<Id> sampler(Stream &stream, const SamplerDescriptor &d) noexcept {
        using detail::f;
        using detail::u;
        return make(stream, op::SAMPLER, [&](auto &s) {
            detail::words(s, {u(d.address[0]), u(d.address[1]), u(d.address[2]), u(d.mag), u(d.min), u(d.mipmap), f(d.lod[0]), f(d.lod[1]), detail::opt(d.compare), d.max_anisotropy});
        });
    }

    std::optional<Id> shader_module(Stream &stream, std::string_view wgsl) noexcept {
        return make(stream, op::SHADER_MODULE, [&](auto &s) { s.bytes(detail::text(wgsl)); });
    }

    std::optional<Id> bind_group_layout(Stream &stream, std::span<const BindGroupLayoutEntry> entries) noexcept {
        using detail::flag;
        using detail::u;
        return make(stream, op::BIND_GROUP_LAYOUT, [&](auto &s) {
            s.word(static_cast<std::uint32_t>(entries.size()));
            for (const BindGroupLayoutEntry &e : entries) {
                std::array<std::uint32_t, 4> w;
                if (auto *b = std::get_if<BufferBinding>(&e.layout)) w = {u(BindingKind::buffer), u(b->ty), flag(b->dynamic_offset), b->min_binding_size};
                else if (auto *b = std::get_if<SamplerBinding>(&e.layout)) w = {u(BindingKind::sampler), u(b->ty), 0, 0};
                else if (auto *b = std::get_if<TextureBinding>(&e.layout)) w = {u(BindingKind::texture), u(b->sample), u(b->dimension), flag(b->multisampled)};
                else {
                    auto &t = std::get<StorageTextureBinding>(e.layout);
                    w = {u(BindingKind::storage_texture), u(t.access), u(t.format), u(t.dimension)};
                }
                detail::words(s, {e.binding, e.visibility, w[0], w[1], w[2], w[3]});
            }
        });
    }

    std::optional<Id> pipeline_layout(Stream &stream, std::span<const Id> layouts) noexcept {
        return make(stream, op::PIPELINE_LAYOUT, [&](auto &s) {
            s.word(static_cast<std::uint32_t>(layouts.size()));
            for (Id l : layouts) s.word(l);
        });
    }

    std::optional<Id> bind_group(Stream &stream, Id layout, std::span<const BindGroupEntry> entries) noexcept {
        using detail::u;
        return make(stream, op::BIND_GROUP, [&](auto &s) {
            detail::words(s, {layout, static_cast<std::uint32_t>(entries.size())});
            for (const BindGroupEntry &e : entries) {
                std::array<std::uint32_t, 4> w;
                if (auto *b = std::get_if<BufferResource>(&e.resource)) w = {u(ResourceKind::buffer), b->buffer, b->offset, detail::opt(b->size)};
                else if (auto *r = std::get_if<SamplerResource>(&e.resource)) w = {u(ResourceKind::sampler), r->sampler, 0, 0};
                else w = {u(ResourceKind::texture_view), std::get<ViewResource>(e.resource).view, 0, 0};
                detail::words(s, {e.binding, w[0], w[1], w[2], w[3]});
            }
        });
    }

    std::optional<Id> render_pipeline(Stream &stream, const RenderPipelineDescriptor &d) noexcept {
        using detail::f;
        using detail::flag;
        using detail::opt;
        using detail::u;
        return make(stream, op::RENDER_PIPELINE, [&](auto &s) {
            s.word(opt(d.layout));
            const VertexState &v = d.vertex;
            s.word(v.module);
            s.bytes(detail::text(v.entry));
            s.word(static_cast<std::uint32_t>(v.buffers.size()));
            for (const VertexBufferLayout &b : v.buffers) {
                detail::words(s, {b.stride, u(b.step), static_cast<std::uint32_t>(b.attributes.size())});
                for (const VertexAttribute &a : b.attributes) detail::words(s, {u(a.format), a.offset, a.location});
            }
            const PrimitiveState &p = d.primitive;
            detail::words(s, {u(p.topology), opt(p.strip_index_format), u(p.front_face), u(p.cull), flag(p.unclipped_depth)});
            if (const auto &ds = d.depth_stencil) {
                detail::words(s, {u(ds->format), flag(ds->depth_write), u(ds->depth_compare)});
                detail::stencil_face(s, ds->stencil_front);
                detail::stencil_face(s, ds->stencil_back);
                detail::words(s, {ds->stencil_read_mask, ds->stencil_write_mask, static_cast<std::uint32_t>(ds->bias), f(ds->slope_scale), f(ds->clamp)});
            } else {
                // Present either way, so the record's shape is fixed.
                s.word(NONE);
                for (int k = 0; k < 2 + 8 + 5; k++) s.word(0);
            }
            const MultisampleState &m = d.multisample;
            detail::words(s, {m.count, m.mask, flag(m.alpha_to_coverage)});
            if (const auto &fr = d.fragment) {
                s.word(fr->module);
                s.bytes(detail::text(fr->entry));
                s.word(static_cast<std::uint32_t>(fr->targets.size()));
                for (const ColorTargetState &t : fr->targets) {
                    s.word(u(t.format));
                    if (const auto &b = t.blend) {
                        detail::words(s, {u(b->color.operation), u(b->color.src), u(b->color.dst), u(b->alpha.operation), u(b->alpha.src), u(b->alpha.dst)});
                    } else {
                        s.word(NONE);
                        for (int k = 0; k < 5; k++) s.word(0);
                    }
                    s.word(t.write_mask);
                }
            } else {
                s.word(NONE);
                s.bytes({});
                s.word(0);
            }
        });
    }

    std::optional<Id> compute_pipeline(Stream &stream, const ComputePipelineDescriptor &d) noexcept {
        return make(stream, op::COMPUTE_PIPELINE, [&](auto &s) {
            detail::words(s, {detail::opt(d.layout), d.module});
            s.bytes(detail::text(d.entry));
        });
    }

    std::optional<Id> query_set(Stream &stream, QueryType ty, std::uint32_t count) noexcept {
        return make(stream, op::QUERY_SET, [&](auto &s) { detail::words(s, {detail::u(ty), count}); });
    }

    std::optional<Id> encoder(Stream &stream) noexcept {
        return make(stream, op::ENCODER, [](auto &) {});
    }

    std::optional<Id> render_pass(Stream &stream, Id encoder, const RenderPassDescriptor &d) noexcept {
        using detail::f;
        using detail::flag;
        using detail::u;
        return make(stream, op::RENDER_PASS, [&](auto &s) {
            detail::words(s, {encoder, static_cast<std::uint32_t>(d.colors.size())});
            for (const ColorAttachment &c : d.colors) {
                detail::words(s, {c.view, detail::opt(c.resolve), u(c.load), u(c.store), f(c.clear[0]), f(c.clear[1]), f(c.clear[2]), f(c.clear[3])});
            }
            if (const auto &ds = d.depth_stencil) {
                detail::words(s, {ds->view, u(ds->depth_load), u(ds->depth_store), f(ds->depth_clear), flag(ds->depth_read_only), u(ds->stencil_load), u(ds->stencil_store), ds->stencil_clear,
                                  flag(ds->stencil_read_only)});
            } else {
                s.word(NONE);
                for (int k = 0; k < 8; k++) s.word(0);
            }
            detail::timestamps(s, d.timestamps);
            s.word(detail::opt(d.occlusion));
        });
    }

    std::optional<Id> compute_pass(Stream &stream, Id encoder, const std::optional<TimestampWrites> &t) noexcept {
        return make(stream, op::COMPUTE_PASS, [&](auto &s) {
            s.word(encoder);
            detail::timestamps(s, t);
        });
    }

    // This turn's texture of `surface`, as a view.
    std::optional<Id> current(Stream &stream, std::uint32_t surface) noexcept {
        return make(stream, op::CURRENT, [&](auto &s) { s.word(surface); });
    }

    // Ends a pass: the host lets it go with the record, and its handle is
    // free again.
    bool end(Stream &stream, Id pass) noexcept { return detail::emit(stream, op::END, [&](auto &s) { s.word(pass); }) && handles_.release(pass); }

    // Each encoder finished and submitted, in order, and let go as `end`
    // lets a pass go.
    bool submit(Stream &stream, std::span<const Id> encoders) noexcept {
        bool sent = detail::emit(stream, op::SUBMIT, [&](auto &s) {
            s.word(static_cast<std::uint32_t>(encoders.size()));
            for (Id e : encoders) s.word(e);
        });
        if (sent) {
            for (Id e : encoders) handles_.release(e);
        }
        return sent;
    }

    // Lets the object go: the host destroys it, and its handle is free.
    void destroy(Stream &stream, Id id) noexcept {
        if (handles_.release(id)) detail::emit(stream, op::DESTROY, [&](auto &s) { s.word(id); });
    }

    // Whether `id` is an object this encoder made and has not let go.
    bool live(Id id) const noexcept { return handles_.live(id); }

  private:
    // An object's handle, written by `op` as the record's first word; none
    // where the table is full or the record did not fit.
    template <class F> std::optional<Id> make(Stream &stream, std::uint32_t op, F &&put) noexcept {
        std::optional<std::uint32_t> id = handles_.acquire();
        if (!id) return std::nullopt;
        if (detail::emit(stream, op, [&](auto &s) {
                s.word(*id);
                put(s);
            })) {
            return id;
        }
        handles_.release(*id);
        return std::nullopt;
    }

    HandleTable<N> handles_;
};

// The calls that make no object: an op's words straight to the stream, false
// where an open transaction no longer fits them.
namespace call {

bool write_buffer(Stream &stream, Id buffer, std::uint32_t offset, std::span<const std::uint8_t> bytes) noexcept;
// The bytes read where they lie in the guest's memory when the host runs the
// stream, so they must stay until it has.
bool write_buffer_at(Stream &stream, Id buffer, std::uint32_t offset, std::span<const std::uint8_t> bytes) noexcept;
// `layout`: bytes per row, rows per image.
bool write_texture(Stream &stream, const TexelCopyTexture &place, std::array<std::uint32_t, 2> layout, std::array<std::uint32_t, 3> size, std::span<const std::uint8_t> bytes) noexcept;
bool write_texture_at(Stream &stream, const TexelCopyTexture &place, std::array<std::uint32_t, 2> layout, std::array<std::uint32_t, 3> size, std::span<const std::uint8_t> bytes) noexcept;
// An encoded image (PNG, JPEG, WebP) where it lies in the guest's memory,
// decoded into `to`; `word` where `ticket | DONE` lands once it is there.
bool copy_external_image(Stream &stream, const TexelCopyTexture &to, bool flip_y, bool premultiplied, std::span<const std::uint8_t> image, std::uint32_t &word, std::uint32_t ticket) noexcept;
// `from` and `to`: a buffer and its offset.
bool copy_buffer_to_buffer(Stream &stream, Id encoder, std::array<std::uint32_t, 2> from, std::array<std::uint32_t, 2> to, std::uint32_t size) noexcept;
bool copy_buffer_to_texture(Stream &stream, Id encoder, const TexelCopyBuffer &from, const TexelCopyTexture &to, std::array<std::uint32_t, 3> size) noexcept;
bool copy_texture_to_buffer(Stream &stream, Id encoder, const TexelCopyTexture &from, const TexelCopyBuffer &to, std::array<std::uint32_t, 3> size) noexcept;
bool copy_texture_to_texture(Stream &stream, Id encoder, const TexelCopyTexture &from, const TexelCopyTexture &to, std::array<std::uint32_t, 3> size) noexcept;
bool clear_buffer(Stream &stream, Id encoder, Id buffer, std::uint32_t offset, std::optional<std::uint32_t> size) noexcept;
bool resolve_query_set(Stream &stream, Id encoder, Id query_set, std::uint32_t first, std::uint32_t count, Id buffer, std::uint32_t offset) noexcept;
// A render or a compute pipeline, as the pass is.
bool set_pipeline(Stream &stream, Id pass, Id pipeline) noexcept;
bool set_bind_group(Stream &stream, Id pass, std::uint32_t index, Id group, std::span<const std::uint32_t> dynamic_offsets = {}) noexcept;
bool set_vertex_buffer(Stream &stream, Id pass, std::uint32_t slot, Id buffer, std::uint32_t offset, std::optional<std::uint32_t> size) noexcept;
bool set_index_buffer(Stream &stream, Id pass, Id buffer, IndexFormat format, std::uint32_t offset, std::optional<std::uint32_t> size) noexcept;
// `rect`: x, y, width, height; `depth`: min and max.
bool set_viewport(Stream &stream, Id pass, std::array<float, 4> rect, std::array<float, 2> depth) noexcept;
bool set_scissor(Stream &stream, Id pass, std::array<std::uint32_t, 4> rect) noexcept;
bool set_blend_constant(Stream &stream, Id pass, std::array<float, 4> rgba) noexcept;
bool set_stencil_reference(Stream &stream, Id pass, std::uint32_t reference) noexcept;
bool draw(Stream &stream, Id pass, std::uint32_t vertices, std::uint32_t instances, std::uint32_t first_vertex, std::uint32_t first_instance) noexcept;
bool draw_indexed(Stream &stream, Id pass, std::uint32_t indices, std::uint32_t instances, std::uint32_t first_index, std::int32_t base_vertex, std::uint32_t first_instance) noexcept;
bool draw_indirect(Stream &stream, Id pass, Id buffer, std::uint32_t offset) noexcept;
bool draw_indexed_indirect(Stream &stream, Id pass, Id buffer, std::uint32_t offset) noexcept;
bool dispatch(Stream &stream, Id pass, std::array<std::uint32_t, 3> workgroups) noexcept;
bool dispatch_indirect(Stream &stream, Id pass, Id buffer, std::uint32_t offset) noexcept;
// `into` the bytes' place in the guest's memory, `word` where `ticket | DONE`
// lands once they are there, while the ticket still stands.
bool read_buffer(Stream &stream, Id buffer, std::uint32_t offset, std::span<std::uint8_t> into, std::uint32_t &word, std::uint32_t ticket) noexcept;
// `ticket | DONE` at `word` once what was submitted before is done.
bool queue_done(Stream &stream, std::uint32_t &word, std::uint32_t ticket) noexcept;
// `format` none for the format the host's surface prefers.
bool configure(Stream &stream, std::uint32_t surface, std::optional<TextureFormat> format, std::uint32_t usage, CanvasAlphaMode alpha) noexcept;
bool resize(Stream &stream, std::uint32_t surface, std::array<std::uint32_t, 2> size) noexcept;
// Where the host writes the device's limits, features and status.
bool device(Stream &stream, std::span<std::uint32_t> into) noexcept;

} // namespace call

} // namespace xpute::gpu
