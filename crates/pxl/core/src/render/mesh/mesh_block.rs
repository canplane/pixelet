// pxl_core/render/mesh/mesh_block.rs

//! A chunk mesh's block as the mesher lays it out
//! (crates/pxl/core/src/bake/mesher.rs): every lane of a baked mesh in one
//! block, in this fixed order, then the visibility lane, then the index lane.
//! They share a lifetime exactly (the whole mesh dies at once, when its owner
//! does), so one allocation rather than eight — measured: a zoom-out that
//! rebuilds ~2000 chunks moved +344MB in a single frame, 8.4s of it off-thread,
//! when each lane was its own. The renderer uploads the block whole by its
//! address; the lanes' offsets are here for whoever reads one.

/// position, normal, local_pos, box_scale, rgb (3 each), layer_taps (4), faces (1).
pub const F32_PER_VERTEX: u32 = 3 + 3 + 3 + 3 + 3 + 4 + 1;
pub const VIS_F32_PER_VERTEX: u32 = 1;

fn block_bytes(vertex_total: u32, index_total: u32) -> u32 {
    (vertex_total * (F32_PER_VERTEX + VIS_F32_PER_VERTEX) + index_total) * 4
}

/// The bytes one mesh's block holds, visibility's span included. What a mesh
/// copied across a thread boundary has to carry.
pub fn baked_mesh_bytes(vertex_cnt: u32, index_cnt: u32) -> u32 {
    block_bytes(vertex_cnt, index_cnt)
}

/// One lane of a baked mesh: its byte offset from the block's own start — never
/// the memory's: a region is shared with other live chunks' blocks — and its
/// element count.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Lane {
    pub byte_offset: u32,
    pub len: u32,
}

/// One baked chunk mesh's lanes. position/normal are the real chunk-local
/// vertices every box's own faces sit at; the rest are per-vertex material and
/// shading inputs the chunk shader reads directly. `faces` is the box's face
/// mask — an edge of the top face is a silhouette exactly when the side face in
/// that direction survived culling, so the shader shades only the edges that
/// border open air. `visibility` is the per-vertex LOD selector's own span, not
/// baked: a recycled block arrives dirty, so it must be written before the mesh
/// is drawn. The index lane is u32 — a dense chunk can exceed a u16's 65535
/// vertices once every box bakes 20-24 real vertices.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BakedChunkMesh {
    pub vertex_cnt: u32,
    pub position: Lane,
    pub normal: Lane,
    pub local_pos: Lane,
    pub box_scale: Lane,
    pub rgb: Lane,
    pub layer_taps: Lane,
    pub faces: Lane,
    pub visibility: Lane,
    pub index: Lane,
}

/// A mesh's lanes over a block laid out by this module.
pub fn carve_baked_mesh(vertex_cnt: u32, index_cnt: u32) -> BakedChunkMesh {
    let mut at = 0u32;
    let mut cut = |width: u32| {
        let lane = Lane { byte_offset: at, len: vertex_cnt * width };
        at += vertex_cnt * width * 4;
        lane
    };
    let position = cut(3);
    let normal = cut(3);
    let local_pos = cut(3);
    let box_scale = cut(3);
    let rgb = cut(3);
    let layer_taps = cut(4);
    let faces = cut(1);
    let visibility = cut(VIS_F32_PER_VERTEX);
    BakedChunkMesh {
        vertex_cnt,
        position,
        normal,
        local_pos,
        box_scale,
        rgb,
        layer_taps,
        faces,
        visibility,
        index: Lane { byte_offset: at, len: index_cnt },
    }
}
