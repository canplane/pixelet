// cpp/gpu/encode.test.cpp

// Holds the C++ encoder to golden/gpu/programs.tsv: the programs
// rust/gpu/src/programs.test.rs writes, the same calls in C++, write the
// words the record holds turn by turn. Each turn ends with a ticket at the
// done word, the turn's number from 1; the last reads the result back.

#include <cstring>
#include <functional>
#include <string>
#include <vector>

#include "../kit/golden.hpp"
#include "../kit/test.hpp"
#include "encode.hpp"

using namespace xpute;
using namespace xpute::gpu;

namespace {

using Encoder = Gpu<64>;
// What ends a turn: the stream published, and whatever runs it.
using Turn = std::function<void(Stream &)>;

constexpr std::uint32_t SIDE = 16;

// Where the record puts what a program reads and writes.
struct At {
    Memory mem;
    std::uint32_t input;
    std::uint32_t input_len;
    std::uint32_t done_at;
    std::uint32_t readback;

    std::span<const std::uint8_t> input_bytes() const { return {mem.at(input), input_len}; }
    std::uint32_t &done() const { return *reinterpret_cast<std::uint32_t *>(mem.at(done_at)); }
    // Reads `n` bytes of `buffer` back to the record's place.
    void read(Stream &s, Id buffer, std::uint32_t n, std::uint32_t ticket) const { call::read_buffer(s, buffer, 0, {mem.at(readback), n}, done(), ticket); }
};

std::vector<std::uint8_t> le(const std::vector<std::uint32_t> &w) {
    std::vector<std::uint8_t> out(4 * w.size());
    std::memcpy(out.data(), w.data(), out.size());
    return out;
}

std::vector<std::uint8_t> le_f32(const std::vector<float> &v) {
    std::vector<std::uint8_t> out(4 * v.size());
    std::memcpy(out.data(), v.data(), out.size());
    return out;
}

std::pair<Id, Id> target(Encoder &gpu, Stream &s) {
    Id t = *gpu.texture(s, {.size = {SIDE, SIDE, 1},
                            .mip_levels = 1,
                            .samples = 1,
                            .dimension = TextureDimension::_2d,
                            .format = TextureFormat::rgba8unorm,
                            .usage = TEXTURE_USAGE::RENDER_ATTACHMENT | TEXTURE_USAGE::COPY_SRC,
                            .view_formats = {}});
    return {t, *gpu.view(s, t, {})};
}

Id pass(Encoder &gpu, Stream &s, Id e, Id view) {
    const ColorAttachment colors[] = {{view, std::nullopt, LoadOp::clear, StoreOp::store, {0.0f, 0.0f, 0.4f, 1.0f}}};
    return *gpu.render_pass(s, e, {.colors = colors, .depth_stencil = std::nullopt, .timestamps = std::nullopt, .occlusion = std::nullopt});
}

void read_target(Encoder &gpu, Stream &s, const At &at, Id e, Id texture, std::uint32_t ticket) {
    Id out = *gpu.buffer(s, {.size = 256 * SIDE, .usage = BUFFER_USAGE::COPY_DST | BUFFER_USAGE::COPY_SRC, .mapped_at_creation = false});
    call::copy_texture_to_buffer(s, e, {texture, 0, {0, 0, 0}, TextureAspect::all}, {out, 0, 256, SIDE}, {SIDE, SIDE, 1});
    const Id encoders[] = {e};
    gpu.submit(s, encoders);
    at.read(s, out, 256 * SIDE, ticket);
}

const ColorTargetState COLOR_TARGET[] = {{TextureFormat::rgba8unorm, std::nullopt, COLOR_WRITE::ALL}};

constexpr PrimitiveState PRIMITIVE{PrimitiveTopology::triangle_list, std::nullopt, FrontFace::ccw, CullMode::none, false};

BindGroupLayoutEntry buffer_entry(std::uint32_t binding, BufferBindingType ty) { return {binding, SHADER_STAGE::COMPUTE, BufferBinding{ty, false, 0}}; }

BindGroupEntry buffer_resource(std::uint32_t binding, Id buffer) { return {binding, BufferResource{buffer, 0, std::nullopt}}; }

void add_arrays(Encoder &gpu, Stream &s, const At &at, const Turn &turn) {
    constexpr std::uint32_t N = 256;
    auto storage = [&] { return *gpu.buffer(s, {.size = 4 * N, .usage = BUFFER_USAGE::STORAGE | BUFFER_USAGE::COPY_DST | BUFFER_USAGE::COPY_SRC, .mapped_at_creation = false}); };
    Id a = storage(), b = storage(), c = storage();
    std::vector<std::uint32_t> va(N), vb(N);
    for (std::uint32_t i = 0; i < N; i++) va[i] = i, vb[i] = 3 * i + 7;
    call::write_buffer(s, a, 0, le(va));
    call::write_buffer(s, b, 0, le(vb));
    Id m = *gpu.shader_module(s, "@group(0) @binding(0) var<storage, read> a: array<u32>;\n"
                                 "@group(0) @binding(1) var<storage, read> b: array<u32>;\n"
                                 "@group(0) @binding(2) var<storage, read_write> c: array<u32>;\n"
                                 "@compute @workgroup_size(64) fn main(@builtin(global_invocation_id) i: vec3u) { c[i.x] = a[i.x] + b[i.x]; }\n");
    const BindGroupLayoutEntry entries[] = {buffer_entry(0, BufferBindingType::read_only_storage), buffer_entry(1, BufferBindingType::read_only_storage),
                                            buffer_entry(2, BufferBindingType::storage)};
    Id bgl = *gpu.bind_group_layout(s, entries);
    const Id layouts[] = {bgl};
    Id pl = *gpu.pipeline_layout(s, layouts);
    const BindGroupEntry resources[] = {buffer_resource(0, a), buffer_resource(1, b), buffer_resource(2, c)};
    Id bg = *gpu.bind_group(s, bgl, resources);
    Id p = *gpu.compute_pipeline(s, {pl, m, "main"});
    Id e = *gpu.encoder(s);
    Id ps = *gpu.compute_pass(s, e, std::nullopt);
    call::set_pipeline(s, ps, p);
    call::set_bind_group(s, ps, 0, bg);
    call::dispatch(s, ps, {N / 64, 1, 1});
    gpu.end(s, ps);
    const Id encoders[] = {e};
    gpu.submit(s, encoders);
    at.read(s, c, 4 * N, 1);
    turn(s);
}

void triangle(Encoder &gpu, Stream &s, const At &at, const Turn &turn) {
    auto [t, v] = target(gpu, s);
    Id m = *gpu.shader_module(s, "@vertex fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4f {\n"
                                 "  var p = array(vec2f(-0.75, -0.78125), vec2f(0.9375, -0.71875), vec2f(-0.75, 0.96875));\n"
                                 "  return vec4f(p[i], 0.0, 1.0);\n"
                                 "}\n"
                                 "@fragment fn fs() -> @location(0) vec4f { return vec4f(1.0, 0.2, 0.0, 1.0); }\n");
    Id p = *gpu.render_pipeline(s, {.layout = std::nullopt,
                                     .vertex = {m, "vs", {}},
                                     .primitive = PRIMITIVE,
                                     .depth_stencil = std::nullopt,
                                     .multisample = {},
                                     .fragment = FragmentState{m, "fs", COLOR_TARGET}});
    Id e = *gpu.encoder(s);
    Id ps = pass(gpu, s, e, v);
    call::set_pipeline(s, ps, p);
    call::draw(s, ps, 3, 1, 0, 0);
    gpu.end(s, ps);
    read_target(gpu, s, at, e, t, 1);
    turn(s);
}

void indexed_instanced(Encoder &gpu, Stream &s, const At &at, const Turn &turn) {
    auto [t, v] = target(gpu, s);
    std::vector<std::uint8_t> corners = le_f32({9.0f, 9.0f, 9.0f, 9.0f, 0.0f, 0.0f, 1.0f, 0.0f, 0.0f, 1.0f, 1.0f, 1.0f});
    std::vector<std::uint8_t> indices;
    for (std::uint16_t i : {0, 1, 2, 2, 1, 3}) indices.insert(indices.end(), {static_cast<std::uint8_t>(i), static_cast<std::uint8_t>(i >> 8)});
    struct Place {
        float offset[2];
        std::uint8_t color[4];
    };
    const Place places[] = {{{0.0f, 0.0f}, {1, 2, 3, 4}},
                            {{-1.0f, -1.0f}, {255, 0, 0, 255}},
                            {{-0.5f, -0.25f}, {0, 255, 0, 255}},
                            {{0.25f, -0.75f}, {10, 20, 30, 255}},
                            {{0.375f, 0.5f}, {200, 100, 50, 255}}};
    std::vector<std::uint8_t> instances;
    for (const Place &pl : places) {
        std::vector<std::uint8_t> o = le_f32({pl.offset[0], pl.offset[1]});
        instances.insert(instances.end(), o.begin(), o.end());
        instances.insert(instances.end(), pl.color, pl.color + 4);
    }
    auto buffer = [&](std::uint32_t usage, const std::vector<std::uint8_t> &bytes) {
        Id b = *gpu.buffer(s, {.size = static_cast<std::uint32_t>(bytes.size()), .usage = usage | BUFFER_USAGE::COPY_DST, .mapped_at_creation = false});
        call::write_buffer(s, b, 0, bytes);
        return b;
    };
    Id vb = buffer(BUFFER_USAGE::VERTEX, corners), ib = buffer(BUFFER_USAGE::INDEX, indices), inst = buffer(BUFFER_USAGE::VERTEX, instances);
    Id m = *gpu.shader_module(s, "struct O { @builtin(position) p: vec4f, @location(0) @interpolate(flat) c: vec4f }\n"
                                 "@vertex fn vs(@location(0) corner: vec2f, @location(1) offset: vec2f, @location(2) color: vec4f) -> O {\n"
                                 "  return O(vec4f(offset + corner * 0.5, 0.0, 1.0), color);\n"
                                 "}\n"
                                 "@fragment fn fs(i: O) -> @location(0) vec4f { return i.c; }\n");
    const VertexAttribute corner[] = {{VertexFormat::float32x2, 0, 0}};
    const VertexAttribute instance[] = {{VertexFormat::float32x2, 0, 1}, {VertexFormat::unorm8x4, 8, 2}};
    const VertexBufferLayout buffers[] = {{8, VertexStepMode::vertex, corner}, {12, VertexStepMode::instance, instance}};
    Id p = *gpu.render_pipeline(s, {.layout = std::nullopt,
                                     .vertex = {m, "vs", buffers},
                                     .primitive = PRIMITIVE,
                                     .depth_stencil = std::nullopt,
                                     .multisample = {},
                                     .fragment = FragmentState{m, "fs", COLOR_TARGET}});
    Id e = *gpu.encoder(s);
    Id ps = pass(gpu, s, e, v);
    call::set_pipeline(s, ps, p);
    call::set_vertex_buffer(s, ps, 0, vb, 0, std::nullopt);
    call::set_vertex_buffer(s, ps, 1, inst, 0, std::nullopt);
    call::set_index_buffer(s, ps, ib, IndexFormat::uint16, 0, std::nullopt);
    call::draw_indexed(s, ps, 6, 4, 0, 2, 1);
    gpu.end(s, ps);
    read_target(gpu, s, at, e, t, 1);
    turn(s);
}

void sampled_image(Encoder &gpu, Stream &s, const At &at, const Turn &turn) {
    Id image = *gpu.texture(s, {.size = {4, 4, 1},
                                .mip_levels = 1,
                                .samples = 1,
                                .dimension = TextureDimension::_2d,
                                .format = TextureFormat::rgba8unorm,
                                .usage = TEXTURE_USAGE::TEXTURE_BINDING | TEXTURE_USAGE::COPY_DST | TEXTURE_USAGE::RENDER_ATTACHMENT,
                                .view_formats = {}});
    call::copy_external_image(s, {image, 0, {0, 0, 0}, TextureAspect::all}, true, false, at.input_bytes(), at.done(), 1);
    turn(s);

    auto [t, v] = target(gpu, s);
    Id iv = *gpu.view(s, image, {});
    Id nearest = *gpu.sampler(s, {.address = {AddressMode::clamp_to_edge, AddressMode::clamp_to_edge, AddressMode::clamp_to_edge},
                                  .mag = FilterMode::nearest,
                                  .min = FilterMode::nearest,
                                  .mipmap = MipmapFilterMode::nearest,
                                  .lod = {0.0f, 32.0f},
                                  .compare = std::nullopt,
                                  .max_anisotropy = 1});
    const BindGroupLayoutEntry entries[] = {{0, SHADER_STAGE::FRAGMENT, TextureBinding{TextureSampleType::float_, TextureViewDimension::_2d, false}},
                                            {1, SHADER_STAGE::FRAGMENT, SamplerBinding{SamplerBindingType::filtering}}};
    Id bgl = *gpu.bind_group_layout(s, entries);
    const Id layouts[] = {bgl};
    Id pl = *gpu.pipeline_layout(s, layouts);
    const BindGroupEntry resources[] = {{0, ViewResource{iv}}, {1, SamplerResource{nearest}}};
    Id bg = *gpu.bind_group(s, bgl, resources);
    Id m = *gpu.shader_module(s, "@group(0) @binding(0) var image: texture_2d<f32>;\n"
                                 "@group(0) @binding(1) var nearest: sampler;\n"
                                 "@vertex fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4f {\n"
                                 "  var p = array(vec2f(-1.0, -1.0), vec2f(3.0, -1.0), vec2f(-1.0, 3.0));\n"
                                 "  return vec4f(p[i], 0.0, 1.0);\n"
                                 "}\n"
                                 "@fragment fn fs(@builtin(position) p: vec4f) -> @location(0) vec4f { return textureSample(image, nearest, p.xy / 16.0); }\n");
    Id p = *gpu.render_pipeline(s, {.layout = pl,
                                     .vertex = {m, "vs", {}},
                                     .primitive = PRIMITIVE,
                                     .depth_stencil = std::nullopt,
                                     .multisample = {},
                                     .fragment = FragmentState{m, "fs", COLOR_TARGET}});
    Id e = *gpu.encoder(s);
    Id ps = pass(gpu, s, e, v);
    call::set_pipeline(s, ps, p);
    call::set_bind_group(s, ps, 0, bg);
    call::draw(s, ps, 3, 1, 0, 0);
    gpu.end(s, ps);
    read_target(gpu, s, at, e, t, 2);
    turn(s);
}

void timestamps(Encoder &gpu, Stream &s, const At &at, const Turn &turn) {
    Id qs = *gpu.query_set(s, QueryType::timestamp, 2);
    Id times = *gpu.buffer(s, {.size = 16, .usage = BUFFER_USAGE::QUERY_RESOLVE | BUFFER_USAGE::STORAGE, .mapped_at_creation = false});
    Id out = *gpu.buffer(s, {.size = 4, .usage = BUFFER_USAGE::STORAGE | BUFFER_USAGE::COPY_SRC, .mapped_at_creation = false});
    Id m = *gpu.shader_module(s, "@group(0) @binding(0) var<storage, read> t: array<vec2u, 2>;\n"
                                 "@group(0) @binding(1) var<storage, read_write> o: array<u32>;\n"
                                 "@compute @workgroup_size(1) fn idle() {}\n"
                                 "@compute @workgroup_size(1) fn check() { o[0] = u32(t[1].y > t[0].y || (t[1].y == t[0].y && t[1].x >= t[0].x)); }\n");
    const BindGroupLayoutEntry entries[] = {buffer_entry(0, BufferBindingType::read_only_storage), buffer_entry(1, BufferBindingType::storage)};
    Id bgl = *gpu.bind_group_layout(s, entries);
    const Id layouts[] = {bgl};
    Id pl = *gpu.pipeline_layout(s, layouts);
    const BindGroupEntry resources[] = {buffer_resource(0, times), buffer_resource(1, out)};
    Id bg = *gpu.bind_group(s, bgl, resources);
    Id idle = *gpu.compute_pipeline(s, {pl, m, "idle"});
    Id check = *gpu.compute_pipeline(s, {pl, m, "check"});
    Id e = *gpu.encoder(s);
    Id timed = *gpu.compute_pass(s, e, TimestampWrites{qs, 0, 1});
    call::set_pipeline(s, timed, idle);
    call::set_bind_group(s, timed, 0, bg);
    call::dispatch(s, timed, {1, 1, 1});
    gpu.end(s, timed);
    call::resolve_query_set(s, e, qs, 0, 2, times, 0);
    Id ps = *gpu.compute_pass(s, e, std::nullopt);
    call::set_pipeline(s, ps, check);
    call::set_bind_group(s, ps, 0, bg);
    call::dispatch(s, ps, {1, 1, 1});
    gpu.end(s, ps);
    const Id encoders[] = {e};
    gpu.submit(s, encoders);
    at.read(s, out, 4, 1);
    turn(s);
}

// A transaction that does not fit dropped with the objects it made, its
// handles given back and taken again by the next one, which writes 7.
void dropped_transaction(Encoder &gpu, Stream &s, const At &at, const Turn &turn) {
    Id out = *gpu.buffer(s, {.size = 4, .usage = BUFFER_USAGE::STORAGE | BUFFER_USAGE::COPY_SRC, .mapped_at_creation = false});
    Id m = *gpu.shader_module(s, "@group(0) @binding(0) var<storage, read_write> o: array<u32>;\n@compute @workgroup_size(1) fn main() { o[0] = 7u; }\n");
    const BindGroupLayoutEntry entries[] = {buffer_entry(0, BufferBindingType::storage)};
    Id bgl = *gpu.bind_group_layout(s, entries);
    const Id layouts[] = {bgl};
    Id pl = *gpu.pipeline_layout(s, layouts);
    const BindGroupEntry resources[] = {buffer_resource(0, out)};
    Id bg = *gpu.bind_group(s, bgl, resources);
    Id p = *gpu.compute_pipeline(s, {pl, m, "main"});
    auto write = [&](bool spill) {
        gpu.begin(s);
        // Held past the commit: the handle a dropped one gives back.
        gpu.buffer(s, {.size = 4, .usage = BUFFER_USAGE::STORAGE, .mapped_at_creation = false});
        Id e = *gpu.encoder(s);
        Id ps = *gpu.compute_pass(s, e, std::nullopt);
        call::set_pipeline(s, ps, p);
        call::set_bind_group(s, ps, 0, bg);
        call::dispatch(s, ps, {1, 1, 1});
        gpu.end(s, ps);
        // More than the stream holds.
        if (spill) call::write_buffer(s, out, 0, std::vector<std::uint8_t>(s.left() + 4));
        const Id encoders[] = {e};
        gpu.submit(s, encoders);
        return gpu.commit(s);
    };
    XPUTE_CHECK(!write(true));
    XPUTE_CHECK(write(false));
    at.read(s, out, 4, 1);
    turn(s);
}

struct Program {
    const char *name;
    void (*encode)(Encoder &, Stream &, const At &, const Turn &);
};

const Program PROGRAMS[] = {
    {"add_arrays", add_arrays}, {"triangle", triangle}, {"indexed_instanced", indexed_instanced}, {"sampled_image", sampled_image}, {"timestamps", timestamps}, {"dropped_transaction", dropped_transaction},
};

} // namespace

XPUTE_TEST(gpu_every_program_writes_the_words_its_record_holds) {
    Golden g = Golden::load("spec/golden/gpu/programs.tsv");
    std::vector<std::string> recorded;
    g.each("program", [&](const std::string &k) { recorded.push_back(g.s(k)); });
    XPUTE_CHECK_EQ(recorded.size(), std::size(PROGRAMS), "the programs, as the record lists them");
    std::uint32_t region = g.u32("region"), stream_words = g.u32("stream.words"), head = g.u32("stream.head");
    std::vector<std::uint32_t> words(region / 4);
    Memory mem(reinterpret_cast<std::uintptr_t>(words.data()), region);
    for (std::size_t i = 0; i < std::size(PROGRAMS); i++) {
        const Program &p = PROGRAMS[i];
        std::string name = p.name;
        XPUTE_CHECK(recorded[i] == name);
        std::fill(words.begin(), words.end(), 0);
        std::vector<std::uint8_t> input = g.has(name + ".input") ? g.bytes(name + ".input") : std::vector<std::uint8_t>{};
        At at{mem, g.u32("input.at"), static_cast<std::uint32_t>(input.size()), g.u32("done.at"), g.u32("readback.at")};
        std::memcpy(mem.at(at.input), input.data(), input.size());
        Stream stream(mem, 0, stream_words, head);
        Encoder gpu;
        std::size_t turn = 0;
        p.encode(gpu, stream, at, [&](Stream &s) {
            s.publish();
            std::uint32_t n = 4 * (head + mem.word(0));
            std::vector<std::uint8_t> got(mem.at(0), mem.at(0) + n);
            std::string key = name + ".turn." + std::to_string(turn);
            XPUTE_CHECK_EQ(got == g.bytes(key), true, key.c_str());
            turn++;
        });
        XPUTE_CHECK_EQ(turn, g.len(name + ".turn"), (name + "'s turns").c_str());
    }
}
