// xpute-gpu/api.spec.rs
//
// GENERATED from spec/gpu/api.json — do not edit.
//
// The calls a guest makes to a GPU it cannot reach, as records of a stream (ipc/stream):
// WebGPU's vocabulary, its descriptors written out as words, for any program. A record is
// an op, a count and the op's words in the order each op's doc gives; bytes go as a count
// and padded words, as the stream lays them. Objects are the guest's handles (abi/handle).
// An optional field holds NONE where unset. Render and compute sit side by side.
// A transaction (ipc/stream) that does not commit drops its records and gives back the handles they took, so the host never hears of the objects: the encoder's begin and commit open and close both.

// Names as the spec spells them, so one record has one name on every side.
// A table is generated whole, whether or not this crate calls an entry.
#![allow(
    non_upper_case_globals,
    non_snake_case,
    clippy::absolute_paths,
    non_camel_case_types,
    dead_code,
    reason = "generated from the spec"
)]

/// An optional field left unset, an index or a handle absent.
pub const NONE: u32 = 4294967295;

/// Or-ed into a ticket where a completion word is written (COPY_EXTERNAL_IMAGE, MAP_ASYNC, QUEUE_DONE).
pub const DONE: u32 = 2147483648;

/// In CONFIGURE's format: the format the host's surface prefers.
pub const SURFACE_PREFERRED: u32 = 255;

/// A bind group layout's entry (BIND_GROUP_LAYOUT).
pub const BINDING_ENTRY_WORDS: u32 = 6;

/// A bind group's entry (BIND_GROUP).
pub const RESOURCE_ENTRY_WORDS: u32 = 5;

/// The ops, numbered from 1: 0 ends a stream.
pub mod op {
    /// id, size, usage (BUFFER_USAGE), mapped at creation (0 or 1).
    pub const BUFFER: u32 = 1;
    /// id, width, height, depth or layers, mip levels, samples, dimension (TEXTURE_DIMENSION), format (TEXTURE_FORMAT), usage (TEXTURE_USAGE), view formats' count, that many formats.
    pub const TEXTURE: u32 = 2;
    /// id, texture, format or NONE, dimension (TEXTURE_VIEW_DIMENSION) or NONE, aspect (TEXTURE_ASPECT), base mip, mips or NONE, base layer, layers or NONE.
    pub const VIEW: u32 = 3;
    /// id, address u, v, w (ADDRESS_MODE), mag and min (FILTER_MODE), mipmap (MIPMAP_FILTER_MODE), lod min and max (f32 bits), compare (COMPARE_FUNCTION) or NONE, max anisotropy.
    pub const SAMPLER: u32 = 4;
    /// id, the WGSL source as bytes.
    pub const SHADER_MODULE: u32 = 5;
    /// id, entries' count, then each entry's BINDING_ENTRY_WORDS: binding, visibility (SHADER_STAGE), kind (BINDING_KIND) and three words the kind reads — a buffer its type (BUFFER_BINDING_TYPE), dynamic offset (0 or 1), min binding size; a sampler its type (SAMPLER_BINDING_TYPE); a texture its sample type (TEXTURE_SAMPLE_TYPE), view dimension, multisampled (0 or 1); a storage texture its access (STORAGE_TEXTURE_ACCESS), format, view dimension.
    pub const BIND_GROUP_LAYOUT: u32 = 6;
    /// id, bind group layouts' count, that many ids.
    pub const PIPELINE_LAYOUT: u32 = 7;
    /// id, layout, entries' count, then each entry's RESOURCE_ENTRY_WORDS: binding, kind (RESOURCE_KIND), the object's id, offset, size or NONE (a buffer's; 0 for the others).
    pub const BIND_GROUP: u32 = 8;
    /// id, layout or NONE for an automatic one; vertex: module, entry (bytes), buffers' count, each: stride, step (VERTEX_STEP_MODE), attributes' count, each: format (VERTEX_FORMAT), offset, location; primitive: topology (PRIMITIVE_TOPOLOGY), strip index format (INDEX_FORMAT) or NONE, front face (FRONT_FACE), cull (CULL_MODE), unclipped depth (0 or 1); depth-stencil: format or NONE for none, the rest present either way: depth write (0 or 1), compare, stencil front and back each compare, fail, depth fail and pass (STENCIL_OPERATION), read mask, write mask, bias (i32), slope scale and clamp (f32 bits); multisample: count, mask, alpha to coverage (0 or 1); fragment: module or NONE for none, the rest present either way (no bytes, no targets): entry (bytes), targets' count, each: format, blend color op, src, dst and alpha op, src, dst (BLEND_OPERATION, BLEND_FACTOR) or NONE in the color op for no blending and the five after it present either way, write mask (COLOR_WRITE).
    pub const RENDER_PIPELINE: u32 = 9;
    /// id, layout or NONE, module, entry (bytes).
    pub const COMPUTE_PIPELINE: u32 = 10;
    /// id, type (QUERY_TYPE), count.
    pub const QUERY_SET: u32 = 11;
    /// id: any object, made by any op above.
    pub const DESTROY: u32 = 12;
    /// buffer, offset, the bytes.
    pub const WRITE_BUFFER: u32 = 13;
    /// buffer, offset, address, bytes: read from the guest's memory when the host runs it.
    pub const WRITE_BUFFER_AT: u32 = 14;
    /// texture, mip, origin x, y, z, aspect, bytes per row, rows per image, width, height, depth, the bytes.
    pub const WRITE_TEXTURE: u32 = 15;
    /// as WRITE_TEXTURE, the bytes an address and a count in the guest's memory.
    pub const WRITE_TEXTURE_AT: u32 = 16;
    /// texture, mip, origin x, y, z, aspect (TEXTURE_ASPECT), flip y (0 or 1), premultiplied (0 or 1), address, bytes, word, ticket: an encoded image (PNG, JPEG, WebP) the host decodes into the texture, and once it is there writes ticket | DONE at the word, where the ticket still stands; an image the host cannot decode is left out and its word not written.
    pub const COPY_EXTERNAL_IMAGE: u32 = 17;
    /// id.
    pub const ENCODER: u32 = 18;
    /// encoder, source, its offset, destination, its offset, size.
    pub const COPY_BUFFER_TO_BUFFER: u32 = 19;
    /// encoder, buffer, offset, bytes per row, rows per image, texture, mip, origin x, y, z, aspect, width, height, depth.
    pub const COPY_BUFFER_TO_TEXTURE: u32 = 20;
    /// encoder, texture, mip, origin x, y, z, aspect, buffer, offset, bytes per row, rows per image, width, height, depth.
    pub const COPY_TEXTURE_TO_BUFFER: u32 = 21;
    /// encoder, source, mip, origin x, y, z, aspect, destination, mip, origin x, y, z, aspect, width, height, depth.
    pub const COPY_TEXTURE_TO_TEXTURE: u32 = 22;
    /// encoder, buffer, offset, size or NONE.
    pub const CLEAR_BUFFER: u32 = 23;
    /// encoder, query set, first, count, buffer, offset.
    pub const RESOLVE_QUERY_SET: u32 = 24;
    /// id, encoder, color attachments' count, each: view, resolve view or NONE, load (LOAD_OP), store (STORE_OP), clear r, g, b, a (f32 bits); depth-stencil: view or NONE for none, the rest present either way: depth load, store, clear (f32 bits), read only (0 or 1), stencil load, store, clear, read only; timestamps: query set or NONE, begin index or NONE, end index or NONE; occlusion query set or NONE.
    pub const RENDER_PASS: u32 = 25;
    /// id, encoder; timestamps: query set or NONE, begin index or NONE, end index or NONE.
    pub const COMPUTE_PASS: u32 = 26;
    /// pass, pipeline: render or compute, as the pass is.
    pub const SET_PIPELINE: u32 = 27;
    /// pass, index, group, dynamic offsets' count, that many offsets.
    pub const SET_BIND_GROUP: u32 = 28;
    /// pass, slot, buffer, offset, size or NONE.
    pub const SET_VERTEX_BUFFER: u32 = 29;
    /// pass, buffer, format (INDEX_FORMAT), offset, size or NONE.
    pub const SET_INDEX_BUFFER: u32 = 30;
    /// pass, x, y, width, height, min depth, max depth (f32 bits).
    pub const SET_VIEWPORT: u32 = 31;
    /// pass, x, y, width, height.
    pub const SET_SCISSOR: u32 = 32;
    /// pass, r, g, b, a (f32 bits).
    pub const SET_BLEND_CONSTANT: u32 = 33;
    /// pass, reference.
    pub const SET_STENCIL_REFERENCE: u32 = 34;
    /// pass, vertices, instances, first vertex, first instance.
    pub const DRAW: u32 = 35;
    /// pass, indices, instances, first index, base vertex (i32), first instance.
    pub const DRAW_INDEXED: u32 = 36;
    /// pass, buffer, offset.
    pub const DRAW_INDIRECT: u32 = 37;
    /// pass, buffer, offset.
    pub const DRAW_INDEXED_INDIRECT: u32 = 38;
    /// pass, workgroups x, y, z.
    pub const DISPATCH: u32 = 39;
    /// pass, buffer, offset.
    pub const DISPATCH_INDIRECT: u32 = 40;
    /// pass: ended, and let go with the record.
    pub const END: u32 = 41;
    /// encoders' count, that many encoders, each finished and let go with the record.
    pub const SUBMIT: u32 = 42;
    /// buffer, offset, size, address, word, ticket: WebGPU's mapAsync for reading, of a buffer the guest made MAP_READ | COPY_DST and copied into on an encoder it submitted before. Once mapped, the range is copied to the address, ticket | DONE written at the word and the buffer unmapped. The word is the mapping's own: the guest maps the buffer again only once it holds DONE, and judges by the ticket whose bytes landed.
    pub const MAP_ASYNC: u32 = 43;
    /// word, ticket: ticket | DONE at the word once what was submitted before it is done, where the ticket still stands.
    pub const QUEUE_DONE: u32 = 44;
    /// surface, format (TEXTURE_FORMAT, or SURFACE_PREFERRED), usage (TEXTURE_USAGE), alpha mode (CANVAS_ALPHA_MODE).
    pub const CONFIGURE: u32 = 45;
    /// surface, width, height.
    pub const RESIZE: u32 = 46;
    /// id, surface: the surface's texture for this turn, as a view.
    pub const CURRENT: u32 = 47;
    /// address, words: the host writes there the device's limits (LIMIT_NAME, a word each), then its features (FEATURE_NAME, a bit each, in words), then its timestamp period (f32 bits: the nanoseconds a resolved timestamp's unit is, 1 where they are nanoseconds as WebGPU's are), then the surface format it prefers (TEXTURE_FORMAT: what SURFACE_PREFERRED names, as the web's getPreferredCanvasFormat), then STATUS.
    pub const DEVICE: u32 = 48;
    pub const KEYS: &[&str] = &[
        "BUFFER",
        "TEXTURE",
        "VIEW",
        "SAMPLER",
        "SHADER_MODULE",
        "BIND_GROUP_LAYOUT",
        "PIPELINE_LAYOUT",
        "BIND_GROUP",
        "RENDER_PIPELINE",
        "COMPUTE_PIPELINE",
        "QUERY_SET",
        "DESTROY",
        "WRITE_BUFFER",
        "WRITE_BUFFER_AT",
        "WRITE_TEXTURE",
        "WRITE_TEXTURE_AT",
        "COPY_EXTERNAL_IMAGE",
        "ENCODER",
        "COPY_BUFFER_TO_BUFFER",
        "COPY_BUFFER_TO_TEXTURE",
        "COPY_TEXTURE_TO_BUFFER",
        "COPY_TEXTURE_TO_TEXTURE",
        "CLEAR_BUFFER",
        "RESOLVE_QUERY_SET",
        "RENDER_PASS",
        "COMPUTE_PASS",
        "SET_PIPELINE",
        "SET_BIND_GROUP",
        "SET_VERTEX_BUFFER",
        "SET_INDEX_BUFFER",
        "SET_VIEWPORT",
        "SET_SCISSOR",
        "SET_BLEND_CONSTANT",
        "SET_STENCIL_REFERENCE",
        "DRAW",
        "DRAW_INDEXED",
        "DRAW_INDIRECT",
        "DRAW_INDEXED_INDIRECT",
        "DISPATCH",
        "DISPATCH_INDIRECT",
        "END",
        "SUBMIT",
        "MAP_ASYNC",
        "QUEUE_DONE",
        "CONFIGURE",
        "RESIZE",
        "CURRENT",
        "DEVICE",
    ];
}

pub const BINDING_KIND: &[&str] = &["buffer", "sampler", "texture", "storage-texture"];
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum BindingKind {
    buffer = 0,
    sampler = 1,
    texture = 2,
    storage_texture = 3,
}

impl BindingKind {
    /// Every member, in the JSON's own order.
    pub const ALL: [BindingKind; 4] = [
        BindingKind::buffer,
        BindingKind::sampler,
        BindingKind::texture,
        BindingKind::storage_texture,
    ];

    /// The member a number names, or none for a number the set does not
    /// give out.
    pub fn of(val: u8) -> Option<BindingKind> {
        BindingKind::ALL
            .into_iter()
            .find(|member| *member as u8 == val)
    }
}

pub const RESOURCE_KIND: &[&str] = &["buffer", "sampler", "texture-view"];
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum ResourceKind {
    buffer = 0,
    sampler = 1,
    texture_view = 2,
}

impl ResourceKind {
    /// Every member, in the JSON's own order.
    pub const ALL: [ResourceKind; 3] = [
        ResourceKind::buffer,
        ResourceKind::sampler,
        ResourceKind::texture_view,
    ];

    /// The member a number names, or none for a number the set does not
    /// give out.
    pub fn of(val: u8) -> Option<ResourceKind> {
        ResourceKind::ALL
            .into_iter()
            .find(|member| *member as u8 == val)
    }
}

/// GPUBufferUsage, its bits as WebGPU numbers them.
pub mod BUFFER_USAGE {
    pub const MAP_READ: u32 = 1;
    pub const MAP_WRITE: u32 = 2;
    pub const COPY_SRC: u32 = 4;
    pub const COPY_DST: u32 = 8;
    pub const INDEX: u32 = 16;
    pub const VERTEX: u32 = 32;
    pub const UNIFORM: u32 = 64;
    pub const STORAGE: u32 = 128;
    pub const INDIRECT: u32 = 256;
    pub const QUERY_RESOLVE: u32 = 512;
    pub const KEYS: &[&str] = &[
        "MAP_READ",
        "MAP_WRITE",
        "COPY_SRC",
        "COPY_DST",
        "INDEX",
        "VERTEX",
        "UNIFORM",
        "STORAGE",
        "INDIRECT",
        "QUERY_RESOLVE",
    ];
}

/// GPUTextureUsage, its bits as WebGPU numbers them.
pub mod TEXTURE_USAGE {
    pub const COPY_SRC: u32 = 1;
    pub const COPY_DST: u32 = 2;
    pub const TEXTURE_BINDING: u32 = 4;
    pub const STORAGE_BINDING: u32 = 8;
    pub const RENDER_ATTACHMENT: u32 = 16;
    pub const KEYS: &[&str] = &[
        "COPY_SRC",
        "COPY_DST",
        "TEXTURE_BINDING",
        "STORAGE_BINDING",
        "RENDER_ATTACHMENT",
    ];
}

/// GPUShaderStage, its bits as WebGPU numbers them.
pub mod SHADER_STAGE {
    pub const VERTEX: u32 = 1;
    pub const FRAGMENT: u32 = 2;
    pub const COMPUTE: u32 = 4;
    pub const KEYS: &[&str] = &["VERTEX", "FRAGMENT", "COMPUTE"];
}

/// GPUColorWrite, its bits as WebGPU numbers them.
pub mod COLOR_WRITE {
    pub const RED: u32 = 1;
    pub const GREEN: u32 = 2;
    pub const BLUE: u32 = 4;
    pub const ALPHA: u32 = 8;
    pub const ALL: u32 = 15;
    pub const KEYS: &[&str] = &["RED", "GREEN", "BLUE", "ALPHA", "ALL"];
}

/// DEVICE's last word: what the host knows of the device.
pub mod STATUS {
    pub const READY: u32 = 1;
    pub const LOST: u32 = 2;
    pub const KEYS: &[&str] = &["READY", "LOST"];
}
