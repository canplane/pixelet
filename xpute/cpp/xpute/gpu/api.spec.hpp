// cpp/xpute/gpu/api.spec.hpp
//
// GENERATED from spec/gpu/api.json
//
// The calls a guest makes to a GPU it cannot reach, as records of a stream (ipc/stream):
// WebGPU's vocabulary, its descriptors written out as words, for any program. A record is
// an op, a count and the op's words in the order each op's doc gives; bytes go as a count
// and padded words, as the stream lays them. Objects are the guest's handles (abi/handle).
// An optional field holds NONE where unset. Render and compute sit side by side.
// A transaction (ipc/stream) that does not commit drops its records and gives back the handles they took, so the host never hears of the objects: the encoder's begin and commit open and close both.

#pragma once

#include <array>
#include <cstdint>
#include <string_view>

namespace xpute::gpu {

// An optional field left unset, an index or a handle absent.
inline constexpr std::uint32_t NONE = 4294967295u;

// Or-ed into a ticket where a completion word is written (COPY_EXTERNAL_IMAGE, MAP_ASYNC, QUEUE_DONE).
inline constexpr std::uint32_t DONE = 2147483648u;

// In CONFIGURE's format: the format the host's surface prefers.
inline constexpr std::uint32_t SURFACE_PREFERRED = 255u;

// A bind group layout's entry (BIND_GROUP_LAYOUT).
inline constexpr std::uint32_t BINDING_ENTRY_WORDS = 6u;

// A bind group's entry (BIND_GROUP).
inline constexpr std::uint32_t RESOURCE_ENTRY_WORDS = 5u;

// The ops, numbered from 1: 0 ends a stream.
namespace op {

// id, size, usage (BUFFER_USAGE), mapped at creation (0 or 1).
inline constexpr std::uint32_t BUFFER = 1u;

// id, width, height, depth or layers, mip levels, samples, dimension (TEXTURE_DIMENSION), format (TEXTURE_FORMAT), usage (TEXTURE_USAGE), view formats' count, that many formats.
inline constexpr std::uint32_t TEXTURE = 2u;

// id, texture, format or NONE, dimension (TEXTURE_VIEW_DIMENSION) or NONE, aspect (TEXTURE_ASPECT), base mip, mips or NONE, base layer, layers or NONE.
inline constexpr std::uint32_t VIEW = 3u;

// id, address u, v, w (ADDRESS_MODE), mag and min (FILTER_MODE), mipmap (MIPMAP_FILTER_MODE), lod min and max (f32 bits), compare (COMPARE_FUNCTION) or NONE, max anisotropy.
inline constexpr std::uint32_t SAMPLER = 4u;

// id, the WGSL source as bytes.
inline constexpr std::uint32_t SHADER_MODULE = 5u;

// id, entries' count, then each entry's BINDING_ENTRY_WORDS: binding, visibility (SHADER_STAGE), kind (BINDING_KIND) and three words the kind reads — a buffer its type (BUFFER_BINDING_TYPE), dynamic offset (0 or 1), min binding size; a sampler its type (SAMPLER_BINDING_TYPE); a texture its sample type (TEXTURE_SAMPLE_TYPE), view dimension, multisampled (0 or 1); a storage texture its access (STORAGE_TEXTURE_ACCESS), format, view dimension.
inline constexpr std::uint32_t BIND_GROUP_LAYOUT = 6u;

// id, bind group layouts' count, that many ids.
inline constexpr std::uint32_t PIPELINE_LAYOUT = 7u;

// id, layout, entries' count, then each entry's RESOURCE_ENTRY_WORDS: binding, kind (RESOURCE_KIND), the object's id, offset, size or NONE (a buffer's; 0 for the others).
inline constexpr std::uint32_t BIND_GROUP = 8u;

// id, layout or NONE for an automatic one; vertex: module, entry (bytes), buffers' count, each: stride, step (VERTEX_STEP_MODE), attributes' count, each: format (VERTEX_FORMAT), offset, location; primitive: topology (PRIMITIVE_TOPOLOGY), strip index format (INDEX_FORMAT) or NONE, front face (FRONT_FACE), cull (CULL_MODE), unclipped depth (0 or 1); depth-stencil: format or NONE for none, the rest present either way: depth write (0 or 1), compare, stencil front and back each compare, fail, depth fail and pass (STENCIL_OPERATION), read mask, write mask, bias (i32), slope scale and clamp (f32 bits); multisample: count, mask, alpha to coverage (0 or 1); fragment: module or NONE for none, the rest present either way (no bytes, no targets): entry (bytes), targets' count, each: format, blend color op, src, dst and alpha op, src, dst (BLEND_OPERATION, BLEND_FACTOR) or NONE in the color op for no blending and the five after it present either way, write mask (COLOR_WRITE).
inline constexpr std::uint32_t RENDER_PIPELINE = 9u;

// id, layout or NONE, module, entry (bytes).
inline constexpr std::uint32_t COMPUTE_PIPELINE = 10u;

// id, type (QUERY_TYPE), count.
inline constexpr std::uint32_t QUERY_SET = 11u;

// id: any object, made by any op above.
inline constexpr std::uint32_t DESTROY = 12u;

// buffer, offset, the bytes.
inline constexpr std::uint32_t WRITE_BUFFER = 13u;

// buffer, offset, address, bytes: read from the guest's memory when the host runs it.
inline constexpr std::uint32_t WRITE_BUFFER_AT = 14u;

// texture, mip, origin x, y, z, aspect, bytes per row, rows per image, width, height, depth, the bytes.
inline constexpr std::uint32_t WRITE_TEXTURE = 15u;

// as WRITE_TEXTURE, the bytes an address and a count in the guest's memory.
inline constexpr std::uint32_t WRITE_TEXTURE_AT = 16u;

// texture, mip, origin x, y, z, aspect (TEXTURE_ASPECT), flip y (0 or 1), premultiplied (0 or 1), address, bytes, word, ticket: an encoded image (PNG, JPEG, WebP) the host decodes into the texture, and once it is there writes ticket | DONE at the word, where the ticket still stands; an image the host cannot decode is left out and its word not written.
inline constexpr std::uint32_t COPY_EXTERNAL_IMAGE = 17u;

// id.
inline constexpr std::uint32_t ENCODER = 18u;

// encoder, source, its offset, destination, its offset, size.
inline constexpr std::uint32_t COPY_BUFFER_TO_BUFFER = 19u;

// encoder, buffer, offset, bytes per row, rows per image, texture, mip, origin x, y, z, aspect, width, height, depth.
inline constexpr std::uint32_t COPY_BUFFER_TO_TEXTURE = 20u;

// encoder, texture, mip, origin x, y, z, aspect, buffer, offset, bytes per row, rows per image, width, height, depth.
inline constexpr std::uint32_t COPY_TEXTURE_TO_BUFFER = 21u;

// encoder, source, mip, origin x, y, z, aspect, destination, mip, origin x, y, z, aspect, width, height, depth.
inline constexpr std::uint32_t COPY_TEXTURE_TO_TEXTURE = 22u;

// encoder, buffer, offset, size or NONE.
inline constexpr std::uint32_t CLEAR_BUFFER = 23u;

// encoder, query set, first, count, buffer, offset.
inline constexpr std::uint32_t RESOLVE_QUERY_SET = 24u;

// id, encoder, color attachments' count, each: view, resolve view or NONE, load (LOAD_OP), store (STORE_OP), clear r, g, b, a (f32 bits); depth-stencil: view or NONE for none, the rest present either way: depth load, store, clear (f32 bits), read only (0 or 1), stencil load, store, clear, read only; timestamps: query set or NONE, begin index or NONE, end index or NONE; occlusion query set or NONE.
inline constexpr std::uint32_t RENDER_PASS = 25u;

// id, encoder; timestamps: query set or NONE, begin index or NONE, end index or NONE.
inline constexpr std::uint32_t COMPUTE_PASS = 26u;

// pass, pipeline: render or compute, as the pass is.
inline constexpr std::uint32_t SET_PIPELINE = 27u;

// pass, index, group, dynamic offsets' count, that many offsets.
inline constexpr std::uint32_t SET_BIND_GROUP = 28u;

// pass, slot, buffer, offset, size or NONE.
inline constexpr std::uint32_t SET_VERTEX_BUFFER = 29u;

// pass, buffer, format (INDEX_FORMAT), offset, size or NONE.
inline constexpr std::uint32_t SET_INDEX_BUFFER = 30u;

// pass, x, y, width, height, min depth, max depth (f32 bits).
inline constexpr std::uint32_t SET_VIEWPORT = 31u;

// pass, x, y, width, height.
inline constexpr std::uint32_t SET_SCISSOR = 32u;

// pass, r, g, b, a (f32 bits).
inline constexpr std::uint32_t SET_BLEND_CONSTANT = 33u;

// pass, reference.
inline constexpr std::uint32_t SET_STENCIL_REFERENCE = 34u;

// pass, vertices, instances, first vertex, first instance.
inline constexpr std::uint32_t DRAW = 35u;

// pass, indices, instances, first index, base vertex (i32), first instance.
inline constexpr std::uint32_t DRAW_INDEXED = 36u;

// pass, buffer, offset.
inline constexpr std::uint32_t DRAW_INDIRECT = 37u;

// pass, buffer, offset.
inline constexpr std::uint32_t DRAW_INDEXED_INDIRECT = 38u;

// pass, workgroups x, y, z.
inline constexpr std::uint32_t DISPATCH = 39u;

// pass, buffer, offset.
inline constexpr std::uint32_t DISPATCH_INDIRECT = 40u;

// pass: ended, and let go with the record.
inline constexpr std::uint32_t END = 41u;

// encoders' count, that many encoders, each finished and let go with the record.
inline constexpr std::uint32_t SUBMIT = 42u;

// buffer, offset, size, address, word, ticket: WebGPU's mapAsync for reading, of a buffer the guest made MAP_READ | COPY_DST and copied into on an encoder it submitted before. Once mapped, the range is copied to the address, ticket | DONE written at the word and the buffer unmapped. The word is the mapping's own: the guest maps the buffer again only once it holds DONE, and judges by the ticket whose bytes landed.
inline constexpr std::uint32_t MAP_ASYNC = 43u;

// word, ticket: ticket | DONE at the word once what was submitted before it is done, where the ticket still stands.
inline constexpr std::uint32_t QUEUE_DONE = 44u;

// surface, format (TEXTURE_FORMAT, or SURFACE_PREFERRED), usage (TEXTURE_USAGE), alpha mode (CANVAS_ALPHA_MODE).
inline constexpr std::uint32_t CONFIGURE = 45u;

// surface, width, height.
inline constexpr std::uint32_t RESIZE = 46u;

// id, surface: the surface's texture for this turn, as a view.
inline constexpr std::uint32_t CURRENT = 47u;

// address, words: the host writes there the device's limits (LIMIT_NAME, a word each), then its features (FEATURE_NAME, a bit each, in words), then its timestamp period (f32 bits: the nanoseconds a resolved timestamp's unit is, 1 where they are nanoseconds as WebGPU's are), then the surface format it prefers (TEXTURE_FORMAT: what SURFACE_PREFERRED names, as the web's getPreferredCanvasFormat), then STATUS.
inline constexpr std::uint32_t DEVICE = 48u;
} // namespace op

enum class BindingKind : std::uint8_t {
    buffer = 0,
    sampler = 1,
    texture = 2,
    storage_texture = 3,
};

inline constexpr std::array<std::string_view, 4> BINDING_KIND{"buffer", "sampler", "texture", "storage-texture"};

enum class ResourceKind : std::uint8_t {
    buffer = 0,
    sampler = 1,
    texture_view = 2,
};

inline constexpr std::array<std::string_view, 3> RESOURCE_KIND{"buffer", "sampler", "texture-view"};

// GPUBufferUsage, its bits as WebGPU numbers them.
namespace BUFFER_USAGE {

inline constexpr std::uint32_t MAP_READ = 1u;

inline constexpr std::uint32_t MAP_WRITE = 2u;

inline constexpr std::uint32_t COPY_SRC = 4u;

inline constexpr std::uint32_t COPY_DST = 8u;

inline constexpr std::uint32_t INDEX = 16u;

inline constexpr std::uint32_t VERTEX = 32u;

inline constexpr std::uint32_t UNIFORM = 64u;

inline constexpr std::uint32_t STORAGE = 128u;

inline constexpr std::uint32_t INDIRECT = 256u;

inline constexpr std::uint32_t QUERY_RESOLVE = 512u;
} // namespace BUFFER_USAGE

// GPUTextureUsage, its bits as WebGPU numbers them.
namespace TEXTURE_USAGE {

inline constexpr std::uint32_t COPY_SRC = 1u;

inline constexpr std::uint32_t COPY_DST = 2u;

inline constexpr std::uint32_t TEXTURE_BINDING = 4u;

inline constexpr std::uint32_t STORAGE_BINDING = 8u;

inline constexpr std::uint32_t RENDER_ATTACHMENT = 16u;
} // namespace TEXTURE_USAGE

// GPUShaderStage, its bits as WebGPU numbers them.
namespace SHADER_STAGE {

inline constexpr std::uint32_t VERTEX = 1u;

inline constexpr std::uint32_t FRAGMENT = 2u;

inline constexpr std::uint32_t COMPUTE = 4u;
} // namespace SHADER_STAGE

// GPUColorWrite, its bits as WebGPU numbers them.
namespace COLOR_WRITE {

inline constexpr std::uint32_t RED = 1u;

inline constexpr std::uint32_t GREEN = 2u;

inline constexpr std::uint32_t BLUE = 4u;

inline constexpr std::uint32_t ALPHA = 8u;

inline constexpr std::uint32_t ALL = 15u;
} // namespace COLOR_WRITE

// DEVICE's last word: what the host knows of the device.
namespace STATUS {

inline constexpr std::uint32_t READY = 1u;

inline constexpr std::uint32_t LOST = 2u;
} // namespace STATUS

} // namespace xpute::gpu
