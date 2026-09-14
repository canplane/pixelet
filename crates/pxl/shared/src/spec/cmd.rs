// pxl_shared/spec/cmd.rs

//! The shell's and the kernel's commands, by number (xpute_core::ipc::cmd:
//! `major << 8 | minor`), and the packet each carries — an XTP branch whose
//! children are listed in order. A message's tag is its subject: a chunk's
//! id where there is one, so no packet repeats it. A command with no
//! children carries no packet.
//!
//! The same number is a command on the submission ring and a signal on the
//! completion ring (its `RES` clear): a chunk the shell creates is a chunk
//! it registers, a chunk the kernel creates is one it mounted. The direction
//! is the ring.

#![allow(non_camel_case_types, clippy::upper_case_acronyms)]

/// What a command is about.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum Major {
    NOP = 0x00,
    CHUNK = 0x01,
    PAGE = 0x02,
    EDIT = 0x03,
    TIER = 0x04,
    RUNS = 0x05,
    DECOR = 0x06,
    RAY = 0x07,
    OBJECT = 0x08,
    STATE = 0x09,
    TERRAIN = 0x0a,
    DECOR_KIND = 0x0b,
    GHOST = 0x0c,
    PUCK = 0x0d,
    BILLBOARD = 0x0e,
    ACTOR = 0x0f,
    DEBUG = 0x10,
    ENV = 0x11,
    MODEL = 0x12,
    CHANGE = 0x13,
}

/// The hover ghost's own verbs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum GhostMinor {
    BOX = 0x10,
    DECOR = 0x11,
}

/// The location puck's own verb.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum PuckMinor {
    BEACON = 0x10,
}

/// A billboard set's own verbs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum BillboardMinor {
    PLACE = 0x10,
    FOLLOW = 0x11,
}

/// An actor's own verb.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum ActorMinor {
    PLAY = 0x10,
}

/// A model's own verbs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum ModelMinor {
    JOINT = 0x10,
    CLIP = 0x11,
    CHANNEL = 0x12,
    SEAL = 0x13,
}

/// The environment's own verb.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum EnvMinor {
    OBSERVE = 0x10,
}

/// An object store's own verb.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum ObjectMinor {
    REINDEX = 0x10,
    LANES = 0x11,
    CELL = 0x12,
    COLUMNS = 0x13,
    STAT = 0x14,
}

/// A decor kind's own verb.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum DecorKindMinor {
    MODEL = 0x10,
}

/// A chunk's own verbs, past the four every subject has.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum ChunkMinor {
    WANT = 0x10,
    VISIBLE = 0x11,
    LAYOUT = 0x12,
    BLEND = 0x13,
    BUILD = 0x14,
    DECOR = 0x15,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum Cmd {
    /// Replies with its packet's first child, a u32, or 0 — the rings' own
    /// test.
    NOP = 0x0000,

    // ---- shell → kernel ----
    /// tag chunk · [base u64, tx0 u32, ty0 u32, base_x i32, base_y i32,
    /// stable_hash u32]
    CHUNK_CREATE = 0x0101,
    /// tag chunk · [page: [land u32, water u32, structure u32, transport u32,
    /// ele u32] addresses | null, show_structures bool]
    CHUNK_UPDATE = 0x0103,
    /// tag chunk · no packet
    CHUNK_DELETE = 0x0104,
    /// tag chunk · [first u32, second u32], a step or 0 — the builder's ask,
    /// past the scheduler, for the tests
    CHUNK_WANT = 0x0110,
    /// [page u64]
    PAGE_CREATE = 0x0201,
    /// [page u64]
    PAGE_DELETE = 0x0204,
    /// [hide: [column u64, band i32, pos u32] | null, carve: [column u64,
    /// lx f32, ly f32, lz f32, size f32] | null]
    EDIT_UPDATE = 0x0303,
    /// [key u64, band_z i32, x u32, y u32, z u32, level i32, value u32] — one
    /// leaf; the chunk's entries are not re-derived until OBJECT_REINDEX
    OBJECT_UPDATE = 0x0803,
    /// no packet — the whole store emptied
    OBJECT_DELETE = 0x0804,
    /// [key u64] — the chunk's entries and cells re-derived, and the change
    /// recorded in the change log: the column in both topics, every cell
    /// whose entries changed
    OBJECT_REINDEX = 0x0810,
    /// [state u32, kind u32 (1 solid, 2 air, 3 decor), material u32 (packed
    /// 0xCC_RRGGBB), layer u32] — what a leaf's state paints as
    STATE_UPDATE = 0x0903,
    /// [base: [r u32, g u32, b u32, layer u32, class u32], channels: [land,
    /// water, structure, transport: [class u32, rgb u8[768], has_color u8[256],
    /// layer u8[256], has_layer u8[256]]]] — every table a cell's material is
    /// read from, whole
    TERRAIN_UPDATE = 0x0a03,
    /// [kinds: [[tag u64, density f64, variant_count u32, align_transport
    /// bool, extent f64, decor_value i32 (-1 none), rules: [land, water,
    /// structure, transport: [mode u32 (0 any, 1 void, 2 one of), types u8[]]]]]]
    /// — every decor kind, in group order, replacing what was taught
    DECOR_KIND_UPDATE = 0x0b03,
    /// [group u32, model u32] — the model a decor group is drawn with, for the
    /// hit test
    DECOR_KIND_MODEL = 0x0b10,

    // ---- the scene: what is drawn beside the chunks ----
    //
    // A lane of floats crosses in the packet, not by address: the glue's
    // scratch is given back as soon as the command is sent, and the command
    // is applied later.
    /// [x f64, y f64, z f64, size f64, value u32, rgb u32, coplanar bool] — the
    /// hover ghost as a box, its min corner at (x, y, z)
    GHOST_BOX = 0x0c10,
    /// [x f64, y f64, z f64, group u32, rotation f64] — the hover ghost as one
    /// instance of a decor group
    GHOST_DECOR = 0x0c11,
    /// no packet — the ghost gone
    GHOST_DELETE = 0x0c04,
    /// [x f64, y f64, z f64, zone f32[7 an instance], snap_dist f64, fog_far
    /// f64] — the puck's zone about its centre
    PUCK_UPDATE = 0x0d03,
    /// [offset_x f64, offset_y f64, night bool]
    PUCK_BEACON = 0x0d10,
    /// no packet — the puck gone
    PUCK_DELETE = 0x0d04,
    /// tag call · [order u32] → result: the set's id
    BILLBOARD_CREATE = 0x0e01,
    /// tag set · [instances f32[10 an instance]], relative to its anchor
    BILLBOARD_UPDATE = 0x0e03,
    /// tag set · [x f64, y f64, z f64] — stood at a point
    BILLBOARD_PLACE = 0x0e10,
    /// tag set · [actor u32, lift f64] — stood above an actor
    BILLBOARD_FOLLOW = 0x0e11,
    /// tag set · no packet
    BILLBOARD_DELETE = 0x0e04,
    /// tag call · [model u32] → result: the actor's id, or -1
    ACTOR_CREATE = 0x0f01,
    /// tag actor · [x f64, y f64, z f64, rotation_z f64, visible bool]
    ACTOR_UPDATE = 0x0f03,
    /// tag actor · [clip u32, fade_s f32]
    ACTOR_PLAY = 0x0f10,
    /// tag actor · no packet
    ACTOR_DELETE = 0x0f04,
    /// [x f64, y f64, z f64, vertices f32[6 a vertex]] — the dev gizmos'
    /// segments about an origin
    DEBUG_UPDATE = 0x1003,
    /// [theme f32[34]] — the theme the environment eases toward
    ENV_UPDATE = 0x1103,
    /// [lat f64, lon f64, unix_ms f64] — the observer and the wall clock
    ENV_OBSERVE = 0x1110,

    // ---- models: a mesh block the glue writes, and its skeleton and clips ----
    /// tag call · [vertex_cnt u32, index_cnt u32, joint_cnt u32] → result: the
    /// model's id, or -1 when the mesh pool refused; with an id, [block_at u32]
    /// — where the glue writes the vertices and indices, in the module's
    /// memory
    MODEL_CREATE = 0x1201,
    /// tag call · [model u32] → result: the renderer's handle for its mesh
    /// block, 0 before it is sealed
    MODEL_READ = 0x1202,
    /// tag model · [joint u32, parent i32, pose f32[42]: rest (10), what
    /// stands above a root (16), inverse bind (16)]
    MODEL_JOINT = 0x1210,
    /// tag call · [model u32, duration f32] → result: the clip's index
    MODEL_CLIP = 0x1211,
    /// tag model · [clip u32, joint u32, path u32 (0 translation, 1 rotation,
    /// 2 scale), step bool, times f32[], values f32[]]
    MODEL_CHANNEL = 0x1212,
    /// tag model · [scale f32] — complete: its block goes to the GPU
    MODEL_SEAL = 0x1213,

    // ---- the change log, the chunks' rebuilds now, the day ----
    /// tag call · [topic u32, key u64] → result: the key's new revision
    CHANGE_UPDATE = 0x1303,
    /// tag call · [topic u32, key u64] → result: the key's revision, 0 for
    /// one never bumped
    CHANGE_READ = 0x1302,
    /// [topic u32, key u64] — the key's revision forgotten
    CHANGE_DELETE = 0x1304,
    /// tag chunk · no packet — its layout built to the end now: a placement
    /// shows in the frame it was made
    CHUNK_BUILD = 0x0114,
    /// tag chunk · [show_decor bool] — its decor rebuilt now for the layout it
    /// shows; the tiers untouched
    CHUNK_DECOR = 0x0115,
    /// tag call · [lat f64, lon f64, day_start_ms f64] → [sunrise f64, sunset
    /// f64], in hours after the day's start
    ENV_READ = 0x1102,

    // ---- kernel → shell (signals; CHUNK_CREATE and CHUNK_DELETE above too) ----
    //
    // CHUNK_CREATE: tag chunk · [page u64, base_x i32, base_y i32] — mounted
    // CHUNK_DELETE: tag chunk · no packet — unmounted
    /// tag chunk · [on bool]
    CHUNK_VISIBLE = 0x0111,
    /// tag chunk · [primary u32, secondary u32 (0 for none), ready bool]
    CHUNK_LAYOUT = 0x0112,
    /// tag chunk · [primary f32, secondary f32, decor_secondary bool]
    CHUNK_BLEND = 0x0113,
    /// tag chunk · [generation u32, step u32, at u32, bytes u32, vertex_cnt u32,
    /// index_cnt u32] — a tier built, in its mesh block
    TIER_CREATE = 0x0401,
    /// tag chunk · [step u32] — a tier evicted
    TIER_DELETE = 0x0404,
    /// tag chunk · [bytes u32]
    RUNS_UPDATE = 0x0503,
    /// tag chunk · [step u32, at u32, bytes u32]
    DECOR_CREATE = 0x0601,

    // ---- queries: ACKREQ, answered in the turn they are asked ----
    //
    // A ray: [ray f64[9]: origin_x, origin_y (the map origin, units), ox, oy,
    // oz, dx, dy, dz, max_dist], in the root frame. The reply's result is 1
    // on a hit and 0 on none.
    /// tag call · [ray f64[9]] → [chunk u32, band u32, pos u32, t f64]: the
    /// placed decor entry struck
    DECOR_READ = 0x0602,
    /// tag call · [ray f64[9]] → [hit f64[14]: material class, normal ×3,
    /// point ×3, t, box min ×3, box max ×3]
    RAY_READ = 0x0702,
    /// tag call · [key u64, band_z i32, x u32, y u32, z u32] → result: the
    /// state stored at the leaf, VOID where nothing is
    OBJECT_READ = 0x0802,
    /// tag call · [key u64] → result: the chunk's entry count, 0 where it
    /// holds none; with entries, [band_at u32, pos_at u32, state_at u32] — the
    /// addresses of its lanes, valid until the next write or reindex
    OBJECT_LANES = 0x0811,
    /// tag call · [key u64, cx u32, cy u32, cz i32] → result 1 where the cell
    /// holds entries, 0 where not; with them, [start u32, count u32, solid u32]
    OBJECT_CELL = 0x0812,
    /// tag call · no packet → [keys u64[]]: the columns holding objects, in
    /// the store's order
    OBJECT_COLUMNS = 0x0813,
    /// tag call · no packet → [chunks u32, bands u32, cells u32]
    OBJECT_STAT = 0x0814,
}

impl Cmd {
    const ALL: [Cmd; 59] = [
        Cmd::NOP,
        Cmd::CHUNK_CREATE,
        Cmd::CHUNK_UPDATE,
        Cmd::CHUNK_DELETE,
        Cmd::CHUNK_WANT,
        Cmd::PAGE_CREATE,
        Cmd::PAGE_DELETE,
        Cmd::EDIT_UPDATE,
        Cmd::OBJECT_UPDATE,
        Cmd::OBJECT_DELETE,
        Cmd::OBJECT_REINDEX,
        Cmd::STATE_UPDATE,
        Cmd::TERRAIN_UPDATE,
        Cmd::DECOR_KIND_UPDATE,
        Cmd::DECOR_KIND_MODEL,
        Cmd::CHUNK_VISIBLE,
        Cmd::CHUNK_LAYOUT,
        Cmd::CHUNK_BLEND,
        Cmd::TIER_CREATE,
        Cmd::TIER_DELETE,
        Cmd::RUNS_UPDATE,
        Cmd::DECOR_CREATE,
        Cmd::DECOR_READ,
        Cmd::RAY_READ,
        Cmd::OBJECT_READ,
        Cmd::OBJECT_LANES,
        Cmd::OBJECT_CELL,
        Cmd::OBJECT_COLUMNS,
        Cmd::OBJECT_STAT,
        Cmd::GHOST_BOX,
        Cmd::GHOST_DECOR,
        Cmd::GHOST_DELETE,
        Cmd::PUCK_UPDATE,
        Cmd::PUCK_BEACON,
        Cmd::PUCK_DELETE,
        Cmd::BILLBOARD_CREATE,
        Cmd::BILLBOARD_UPDATE,
        Cmd::BILLBOARD_PLACE,
        Cmd::BILLBOARD_FOLLOW,
        Cmd::BILLBOARD_DELETE,
        Cmd::ACTOR_CREATE,
        Cmd::ACTOR_UPDATE,
        Cmd::ACTOR_PLAY,
        Cmd::ACTOR_DELETE,
        Cmd::DEBUG_UPDATE,
        Cmd::ENV_UPDATE,
        Cmd::ENV_OBSERVE,
        Cmd::MODEL_CREATE,
        Cmd::MODEL_READ,
        Cmd::MODEL_JOINT,
        Cmd::MODEL_CLIP,
        Cmd::MODEL_CHANNEL,
        Cmd::MODEL_SEAL,
        Cmd::CHANGE_UPDATE,
        Cmd::CHANGE_READ,
        Cmd::CHANGE_DELETE,
        Cmd::CHUNK_BUILD,
        Cmd::CHUNK_DECOR,
        Cmd::ENV_READ,
    ];

    /// The command a message's number names, or none for one nobody numbers.
    pub fn of(number: u32) -> Option<Cmd> {
        Cmd::ALL.into_iter().find(|c| *c as u32 == number)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use xpute_core::ipc::cmd::{cmd, Minor};

    #[test]
    fn every_number_is_its_major_and_minor() {
        use Cmd::*;
        let parts = [
            (CHUNK_CREATE, Major::CHUNK as u32, Minor::CREATE as u32),
            (CHUNK_UPDATE, Major::CHUNK as u32, Minor::UPDATE as u32),
            (CHUNK_DELETE, Major::CHUNK as u32, Minor::DELETE as u32),
            (CHUNK_WANT, Major::CHUNK as u32, ChunkMinor::WANT as u32),
            (CHUNK_VISIBLE, Major::CHUNK as u32, ChunkMinor::VISIBLE as u32),
            (CHUNK_LAYOUT, Major::CHUNK as u32, ChunkMinor::LAYOUT as u32),
            (CHUNK_BLEND, Major::CHUNK as u32, ChunkMinor::BLEND as u32),
            (PAGE_CREATE, Major::PAGE as u32, Minor::CREATE as u32),
            (PAGE_DELETE, Major::PAGE as u32, Minor::DELETE as u32),
            (EDIT_UPDATE, Major::EDIT as u32, Minor::UPDATE as u32),
            (OBJECT_UPDATE, Major::OBJECT as u32, Minor::UPDATE as u32),
            (OBJECT_DELETE, Major::OBJECT as u32, Minor::DELETE as u32),
            (OBJECT_REINDEX, Major::OBJECT as u32, ObjectMinor::REINDEX as u32),
            (STATE_UPDATE, Major::STATE as u32, Minor::UPDATE as u32),
            (TERRAIN_UPDATE, Major::TERRAIN as u32, Minor::UPDATE as u32),
            (DECOR_KIND_UPDATE, Major::DECOR_KIND as u32, Minor::UPDATE as u32),
            (DECOR_KIND_MODEL, Major::DECOR_KIND as u32, DecorKindMinor::MODEL as u32),
            (TIER_CREATE, Major::TIER as u32, Minor::CREATE as u32),
            (TIER_DELETE, Major::TIER as u32, Minor::DELETE as u32),
            (RUNS_UPDATE, Major::RUNS as u32, Minor::UPDATE as u32),
            (DECOR_CREATE, Major::DECOR as u32, Minor::CREATE as u32),
            (DECOR_READ, Major::DECOR as u32, Minor::READ as u32),
            (RAY_READ, Major::RAY as u32, Minor::READ as u32),
            (OBJECT_READ, Major::OBJECT as u32, Minor::READ as u32),
            (OBJECT_LANES, Major::OBJECT as u32, ObjectMinor::LANES as u32),
            (OBJECT_CELL, Major::OBJECT as u32, ObjectMinor::CELL as u32),
            (OBJECT_COLUMNS, Major::OBJECT as u32, ObjectMinor::COLUMNS as u32),
            (OBJECT_STAT, Major::OBJECT as u32, ObjectMinor::STAT as u32),
            (GHOST_BOX, Major::GHOST as u32, GhostMinor::BOX as u32),
            (GHOST_DECOR, Major::GHOST as u32, GhostMinor::DECOR as u32),
            (GHOST_DELETE, Major::GHOST as u32, Minor::DELETE as u32),
            (PUCK_UPDATE, Major::PUCK as u32, Minor::UPDATE as u32),
            (PUCK_BEACON, Major::PUCK as u32, PuckMinor::BEACON as u32),
            (PUCK_DELETE, Major::PUCK as u32, Minor::DELETE as u32),
            (BILLBOARD_CREATE, Major::BILLBOARD as u32, Minor::CREATE as u32),
            (BILLBOARD_UPDATE, Major::BILLBOARD as u32, Minor::UPDATE as u32),
            (BILLBOARD_PLACE, Major::BILLBOARD as u32, BillboardMinor::PLACE as u32),
            (BILLBOARD_FOLLOW, Major::BILLBOARD as u32, BillboardMinor::FOLLOW as u32),
            (BILLBOARD_DELETE, Major::BILLBOARD as u32, Minor::DELETE as u32),
            (ACTOR_CREATE, Major::ACTOR as u32, Minor::CREATE as u32),
            (ACTOR_UPDATE, Major::ACTOR as u32, Minor::UPDATE as u32),
            (ACTOR_PLAY, Major::ACTOR as u32, ActorMinor::PLAY as u32),
            (ACTOR_DELETE, Major::ACTOR as u32, Minor::DELETE as u32),
            (DEBUG_UPDATE, Major::DEBUG as u32, Minor::UPDATE as u32),
            (ENV_UPDATE, Major::ENV as u32, Minor::UPDATE as u32),
            (ENV_OBSERVE, Major::ENV as u32, EnvMinor::OBSERVE as u32),
            (MODEL_CREATE, Major::MODEL as u32, Minor::CREATE as u32),
            (MODEL_READ, Major::MODEL as u32, Minor::READ as u32),
            (MODEL_JOINT, Major::MODEL as u32, ModelMinor::JOINT as u32),
            (MODEL_CLIP, Major::MODEL as u32, ModelMinor::CLIP as u32),
            (MODEL_CHANNEL, Major::MODEL as u32, ModelMinor::CHANNEL as u32),
            (MODEL_SEAL, Major::MODEL as u32, ModelMinor::SEAL as u32),
            (CHANGE_UPDATE, Major::CHANGE as u32, Minor::UPDATE as u32),
            (CHANGE_READ, Major::CHANGE as u32, Minor::READ as u32),
            (CHANGE_DELETE, Major::CHANGE as u32, Minor::DELETE as u32),
            (CHUNK_BUILD, Major::CHUNK as u32, ChunkMinor::BUILD as u32),
            (CHUNK_DECOR, Major::CHUNK as u32, ChunkMinor::DECOR as u32),
            (ENV_READ, Major::ENV as u32, Minor::READ as u32),
        ];
        for (c, major, minor) in parts {
            assert_eq!(c as u32, cmd(major, minor), "{c:?}");
            assert_eq!(Cmd::of(c as u32), Some(c));
        }
        assert_eq!(Cmd::of(0x0fff), None);
    }
}
