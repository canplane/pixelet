// cpp/xpute/gpu/link/api.spec.hpp
//
// GENERATED from gpu/link/spec/api.json5 (xpute-gen data.rs)
//
// The calls a guest makes to a GPU it cannot reach, written as records of a stream
// (xpute-gpu-guest stream.rs). The vocabulary is WebGPU's, with its descriptors written out as words, and
// it serves any program. A record is an op, a count, and the op's words in the order the
// op's doc lists them. Bytes go as a count followed by padded words, as the stream lays
// them. Objects are named by the guest's handles (abi/handle). An optional field holds NONE
// when it is unset. Render and compute ops sit side by side.
//
// A transaction (xpute-gpu-guest stream.rs) that does not commit drops its records and gives back the
// handles they took, so the host never hears of those objects. The encoder's begin and
// commit open and close the transaction on both the stream and the handle table.
//
// Where a record names the guest's memory, the bytes it reads or the fence word it answers at
// (OPS), the memory address is a u64 on every platform (link/wit/types.wit `lent`), written as
// two words, its low word first.

#pragma once

#include <array>
#include <cstddef>
#include <cstdint>
#include <optional>
#include <string_view>

namespace xpute::gpu {

// The URI a guest that draws serves its surface under (gpu/wit/surface.wit `surface`),
// which its host opens.
inline constexpr std::array<std::string_view, 2> SURFACE_URI{"xpute:gpu", "surface"};

// The value of an optional field that is unset, such as an absent index or handle.
inline constexpr std::uint32_t NONE = 4294967295u;

// The low bits of a handle, which hold its slot; the high 16 hold the slot's generation.
// Both sides split a handle the same way.
inline constexpr std::uint32_t SLOT_BITS = 16u;

// A value for CONFIGURE's format word that stands for the format the host's surface
// prefers.
inline constexpr std::uint32_t SURFACE_PREFERRED = 255u;

// The length in words of one entry of a bind group layout (BIND_GROUP_LAYOUT).
inline constexpr std::uint32_t BINDING_ENTRY_WORDS = 6u;

// The length in words of one entry of a bind group (BIND_GROUP).
inline constexpr std::uint32_t RESOURCE_ENTRY_WORDS = 5u;

// The ops, numbered from 1; an op of 0 ends a stream.
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

// id, which may name any object an op above made.
inline constexpr std::uint32_t DESTROY = 12u;

// buffer, offset, the bytes.
inline constexpr std::uint32_t WRITE_BUFFER = 13u;

// buffer, offset, address, bytes. The host reads the bytes where they lie in the guest's
// memory when it runs the record, and makes no copy of them first. From the stream's
// flush until `gpu.run` answers, the guest neither rewrites nor frees them (stream.rs).
inline constexpr std::uint32_t WRITE_BUFFER_AT = 14u;

// texture, mip, origin x, y, z, aspect, bytes per row, rows per image, width, height, depth, the bytes.
inline constexpr std::uint32_t WRITE_TEXTURE = 15u;

// The words of WRITE_TEXTURE, except that the bytes are given as an address and a count in
// the guest's memory, which the host reads as it reads WRITE_BUFFER_AT's.
inline constexpr std::uint32_t WRITE_TEXTURE_AT = 16u;

// texture, mip, origin x, y, z, aspect (TEXTURE_ASPECT), flip y (0 or 1), premultiplied (0
// or 1), address, bytes, word, ticket. The bytes are an encoded image (PNG, JPEG, WebP),
// which the host decodes into the texture. The word and the one after it are the
// request's fence and result (RESULT): OK once the image is in the texture, or REFUSED
// when the host cannot decode the image. A request whose texture has been destroyed
// gets no answer.
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

// pass, pipeline; the pipeline is a render or a compute pipeline, as the pass is.
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

// pass. The record ends the pass and lets go of it, so no DESTROY follows.
inline constexpr std::uint32_t END = 41u;

// encoders' count, that many encoders. The record finishes each encoder and lets go of it,
// so no DESTROY follows.
inline constexpr std::uint32_t SUBMIT = 42u;

// buffer, offset, size, address, word, ticket. WebGPU's mapAsync for reading, on a buffer
// the guest made with MAP_READ | COPY_DST and copied into on an encoder it submitted
// earlier. Once the buffer is mapped, the host copies the range to the address and
// unmaps the buffer. The word and the one after it are the request's fence and result
// (RESULT): OK, or REFUSED when the device refused the mapping and nothing was copied.
inline constexpr std::uint32_t MAP_ASYNC = 43u;

// word, ticket. The word is the request's fence (RESULT), answered once the work submitted
// before this record is done.
inline constexpr std::uint32_t QUEUE_DONE = 44u;

// surface, format (TEXTURE_FORMAT, or SURFACE_PREFERRED), usage (TEXTURE_USAGE), alpha mode (CANVAS_ALPHA_MODE).
inline constexpr std::uint32_t CONFIGURE = 45u;

// surface, width, height.
inline constexpr std::uint32_t RESIZE = 46u;

// id, surface. The id names a view of the surface's texture for this entry.
inline constexpr std::uint32_t CURRENT = 47u;

// RENDER_PIPELINE's words, then word, ticket. WebGPU's createRenderPipelineAsync; the word
// is the request's fence (RESULT), answered once the pipeline is made, and until then the
// guest names the pipeline in no other record. When the device refuses the pipeline, the
// host makes it as RENDER_PIPELINE would, reports the error as that op would, and still
// writes the word. A host with no asynchronous call makes the pipeline at once and
// writes the word the same way, before the next entry reads it.
inline constexpr std::uint32_t RENDER_PIPELINE_ASYNC = 49u;

// COMPUTE_PIPELINE's words, then word, ticket. WebGPU's createComputePipelineAsync, whose
// word is answered as RENDER_PIPELINE_ASYNC's is.
inline constexpr std::uint32_t COMPUTE_PIPELINE_ASYNC = 50u;

// id, the label as bytes. The label is the name the device gives the object in its
// messages, as WebGPU's label is. The id may name any object, encoder or pass that an
// earlier record made. A host that cannot name an object after it is made ignores the
// record.
inline constexpr std::uint32_t LABEL = 51u;
} // namespace op

enum class BindingKind : std::uint8_t {
    buffer = 0,
    sampler = 1,
    texture = 2,
    storage_texture = 3,
};

// The member `val` is, or none for a number no member has.
constexpr std::optional<BindingKind> of_BindingKind(std::uint8_t val) noexcept {
    switch (val) {
    case 0: return BindingKind::buffer;
    case 1: return BindingKind::sampler;
    case 2: return BindingKind::texture;
    case 3: return BindingKind::storage_texture;
    }
    return std::nullopt;
}

enum class ResourceKind : std::uint8_t {
    buffer = 0,
    sampler = 1,
    texture_view = 2,
};

// The member `val` is, or none for a number no member has.
constexpr std::optional<ResourceKind> of_ResourceKind(std::uint8_t val) noexcept {
    switch (val) {
    case 0: return ResourceKind::buffer;
    case 1: return ResourceKind::sampler;
    case 2: return ResourceKind::texture_view;
    }
    return std::nullopt;
}

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

// Each address the records of an op name in the guest's memory: the op, and the place
// of the address's low word, counted back from the record's end (`from_end`), as the op's
// words list them: WRITE_BUFFER_AT's bytes, WRITE_TEXTURE_AT's, COPY_EXTERNAL_IMAGE's
// image and fence, MAP_ASYNC's address and fence, and the fence of QUEUE_DONE and of the
// async pipelines, each followed by a count or a ticket. A reader that moves records to
// another block finds their addresses here.
struct OpAddr {
    std::uint32_t op;
    std::uint32_t from_end;
};

inline constexpr std::array<OpAddr, 9> OPS{{{14u, 3u}, {16u, 3u}, {17u, 6u}, {17u, 3u}, {43u, 5u}, {43u, 3u}, {44u, 3u}, {49u, 3u}, {50u, 3u}}};

// The words of the device block, which the guest lends the host to write (gpu/wit/gpu.wit
// `device.lost`), in order: the device's limits (LIMIT_NAME, one word each), then its
// features (FEATURE_NAME, one bit each, packed in words), then its timestamp period (f32
// bits: how many nanoseconds one unit of a resolved timestamp is, 1 when the units are
// nanoseconds as WebGPU's are), then the surface format it prefers (TEXTURE_FORMAT: the
// format SURFACE_PREFERRED stands for, as the web's getPreferredCanvasFormat gives it), then
// the frame latency (the program's GpuConfig `frame_latency`: how many of the guest's frames
// may be unfinished at once, at least one), then the count of uncaptured errors, then the
// count of unfinished frames, then STATUS, the last, which the host stores with release
// after the rest. The uncaptured errors are the errors the device raised that no error
// scope caught. The unfinished frames are the frames the
// guest has drawn whose work the queue has not yet done. Only the host writes the two
// counts: first before STATUS, as it writes the rest of the block, and afterwards with
// release each time a count changes. The guest loads them with acquire once STATUS reads
// READY, and draws a frame only while the unfinished frames are below the block's frame
// latency; both hosts hold a frame to that rule (xpute-gpu-link `admits`).
namespace DEVICE {

inline constexpr std::size_t LIMITS_AT = 0u;

inline constexpr std::size_t FEATS_AT = 31u;

inline constexpr std::size_t PERIOD_AT = 32u;

inline constexpr std::size_t FORMAT_AT = 33u;

inline constexpr std::size_t LATENCY_AT = 34u;

inline constexpr std::size_t ERRORS_AT = 35u;

inline constexpr std::size_t UNFINISHED_AT = 36u;

inline constexpr std::size_t STATUS_AT = 37u;

inline constexpr std::size_t WORDS = 38u;
} // namespace DEVICE

// The last word of the DEVICE block: what the host knows of the device. Only the
// host writes it, and it stores it with release after the rest of the block. The guest
// loads it with acquire before it reads the block.
namespace STATUS {

inline constexpr std::uint32_t READY = 1u;

inline constexpr std::uint32_t LOST = 2u;
} // namespace STATUS

// How a completion is answered. A completion (COPY_EXTERNAL_IMAGE, MAP_ASYNC, QUEUE_DONE,
// RENDER_PIPELINE_ASYNC, COMPUTE_PIPELINE_ASYNC) names a fence word and a ticket. Only the
// guest picks tickets: for each fence word it advances a sequence, which wraps, and it
// keeps one request out on that word at a time. Only the host writes the fence. Once
// what the request guards is written, and after the result in the next word where the
// request can fail (COPY_EXTERNAL_IMAGE, MAP_ASYNC), the host stores the ticket it
// finished into the fence with release. The guest loads the fence with acquire and holds
// the request done when the fence equals the ticket the guest asked with, a test for
// equality that the sequence's wrap does not break. Only then does the guest read the
// result and what the request guards. A late answer to a request that was dropped or
// superseded carries an older ticket and never equals the current one. A lost device
// answers none of its requests.
namespace RESULT {

inline constexpr std::uint32_t OK = 1u;

inline constexpr std::uint32_t REFUSED = 2u;
} // namespace RESULT

} // namespace xpute::gpu
