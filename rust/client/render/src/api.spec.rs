// pxl-render/api.spec.rs
//
// GENERATED from spec/client/render/api.json
//
// pixelet's conventions over xpute's GPU calls (xpute/spec/gpu/api.json), which
// the renderer writes through xpute's encoder (gpu/api.rs): the stream it writes
// them into, its pipeline records' codes, and what a surface shows.

// Names as the spec spells them, so one record has one name on every side.
// A table is generated whole, whether or not this crate calls an entry.
#![allow(
    non_upper_case_globals,
    non_snake_case,
    clippy::absolute_paths,
    dead_code,
    reason = "generated from the spec"
)]

/// What a canvas the UI hands over shows (gpu/surface.rs): the world, a character alone, or
/// nothing yet, as a canvas opens before the UI says what it draws.
pub mod surface_role {
    pub const WORLD: u32 = 0;
    pub const CHARACTER: u32 = 1;
    pub const NONE: u32 = 2;
}

/// The one place the renderer's calls cross (gpu/api.rs): a stream
/// (xpute-core ipc/stream.rs) in the core's machine's GPU socket, which the
/// core writes its calls into during a turn and the runtime runs once the turn
/// has returned. Its size is the socket's (spec/client/core/sys/machine.json `gpu`).
///
/// Its head, past the stream's own words, is what the runtime writes for the
/// core: the device's block (xpute DEVICE) and the timed spans' reads, three
/// of them (gpu/api.rs `READS`), each its word, then each span's begin and end
/// ticks. gpu/api.rs lays them out, and the stream says the head's length itself.
pub mod stream {
    /// What an upload leaves of the range. The records a turn cannot put off — a
    /// frame's passes and draws, the renderer's pipelines — are never refused, so
    /// an upload, or a shader module made before the renderer (`MODULE_BYTES`),
    /// which can wait a turn, is taken only while this much is left after it.
    pub const RESERVE: u32 = 1048576;
    /// The WGSL a turn hands the device as shader modules before the renderer is
    /// made (gpu/pipeline.rs `PipelineCache::make_modules`): one module a turn, then
    /// more while their sources stay within this and `RESERVE` is left. A page
    /// makes a module on its main thread, so this bounds the task that makes
    /// them, a module larger than it alone in its turn.
    pub const MODULE_BYTES: u32 = 131072;
}

/// What a device's GPU runtime runs the renderer's stream by (xpute-gpu runtime
/// `Config`), which the core hands it (pxl-core lib.rs `GPU`, xpute-gpu
/// `CoreConfig`); a page's browser paces its frames and stages its writes itself.
pub mod device {
    /// The frames drawn and not yet finished on the GPU at once: a turn draws only
    /// while fewer are (xpute-gpu runtime `GpuRuntime::admit`), and a surface queues as
    /// many. Two, as wgpu configures a surface by default
    /// (`Surface::get_default_config`): one frame's lag behind the input, with the
    /// GPU kept busy while the next is drawn.
    pub const FRAME_LATENCY: u32 = 2;
    /// The chunk the runtime stages buffer writes in, each mapped once and reused: a
    /// mesh, the largest write a frame makes, is about a MiB, and a larger write
    /// takes a chunk of its own.
    pub const UPLOAD_CHUNK_BYTES: u32 = 1048576;
}

/// A pipeline record's codes (frame/shader/record.rs, expanded into xpute's
/// descriptors by gpu/api.rs), and HEADER, the words it opens with before its
/// vertex buffers. Then: the topology, the cull mode, the depth compare under
/// reversed Z (LESS is plain depth, for a depth-only pass; NO_DEPTH, no
/// depth attachment), the blend, which vertex entry point of
/// its shader the pipeline enters, a vertex step and format, and a binding's
/// kind (its stages are xpute's SHADER_STAGE) — TEXTURE_UNFILTERED a texture in a
/// format no sampler
/// filters (RGBA32F), read by textureLoad alone; TEXTURE_UINT_ARRAY a u32
/// texture's layers, read by textureLoad; STORAGE_TEXTURE_RW an r32uint
/// texture's layers read and written in place, the one format family WebGPU
/// lets a shader do both to; TEXTURE_3D a filtered volume (the air's,
/// pass/atmosphere.rs); STORAGE_TEXTURE_3D an rgba16float volume a compute
/// pass writes, STORAGE_TEXTURE_2D an rgba16float texture one writes (the
/// exposure's texel, pass/atmosphere.rs); SAMPLER_COMPARISON a sampler that
/// compares a depth texture's texels against a reference (the light's shadow
/// map, pass/shadow.rs).
pub mod record {
    pub const HEADER: u32 = 10;
    pub const TRIANGLES: u32 = 0;
    pub const STRIP: u32 = 1;
    pub const CULL_NONE: u32 = 0;
    pub const CULL_BACK: u32 = 1;
    pub const ALWAYS: u32 = 0;
    pub const GREATER: u32 = 1;
    pub const GREATER_EQUAL: u32 = 2;
    pub const LESS: u32 = 3;
    pub const NO_DEPTH: u32 = 4;
    pub const OPAQUE: u32 = 0;
    pub const ALPHA: u32 = 1;
    pub const ADD: u32 = 2;
    pub const VS: u32 = 0;
    pub const VS_DEPTH: u32 = 1;
    pub const PER_VERTEX: u32 = 0;
    pub const PER_INSTANCE: u32 = 1;
    pub const U32: u32 = 5;
    pub const U32X2: u32 = 6;
    pub const F32X2: u32 = 1;
    pub const F32X3: u32 = 2;
    pub const F32X4: u32 = 3;
    pub const U32X4: u32 = 4;
    pub const UNIFORM: u32 = 0;
    pub const UNIFORM_DYNAMIC: u32 = 1;
    pub const STORAGE: u32 = 2;
    pub const TEXTURE: u32 = 3;
    pub const TEXTURE_ARRAY: u32 = 4;
    pub const DEPTH: u32 = 5;
    pub const SAMPLER: u32 = 6;
    pub const SAMPLER_COMPARISON: u32 = 7;
    pub const TEXTURE_UNFILTERED: u32 = 8;
    pub const STORAGE_RW: u32 = 9;
    pub const TEXTURE_UINT_ARRAY: u32 = 10;
    pub const STORAGE_TEXTURE_RW: u32 = 11;
    pub const TEXTURE_3D: u32 = 12;
    pub const STORAGE_TEXTURE_3D: u32 = 13;
    pub const STORAGE_TEXTURE_2D: u32 = 14;
}
