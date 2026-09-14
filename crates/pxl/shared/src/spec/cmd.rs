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
}

/// A chunk's own verbs, past the four every subject has.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum ChunkMinor {
    WANT = 0x10,
    VISIBLE = 0x11,
    LAYOUT = 0x12,
    BLEND = 0x13,
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
}

impl Cmd {
    const ALL: [Cmd; 17] = [
        Cmd::NOP,
        Cmd::CHUNK_CREATE,
        Cmd::CHUNK_UPDATE,
        Cmd::CHUNK_DELETE,
        Cmd::CHUNK_WANT,
        Cmd::PAGE_CREATE,
        Cmd::PAGE_DELETE,
        Cmd::EDIT_UPDATE,
        Cmd::CHUNK_VISIBLE,
        Cmd::CHUNK_LAYOUT,
        Cmd::CHUNK_BLEND,
        Cmd::TIER_CREATE,
        Cmd::TIER_DELETE,
        Cmd::RUNS_UPDATE,
        Cmd::DECOR_CREATE,
        Cmd::DECOR_READ,
        Cmd::RAY_READ,
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
            (TIER_CREATE, Major::TIER as u32, Minor::CREATE as u32),
            (TIER_DELETE, Major::TIER as u32, Minor::DELETE as u32),
            (RUNS_UPDATE, Major::RUNS as u32, Minor::UPDATE as u32),
            (DECOR_CREATE, Major::DECOR as u32, Minor::CREATE as u32),
            (DECOR_READ, Major::DECOR as u32, Minor::READ as u32),
            (RAY_READ, Major::RAY as u32, Minor::READ as u32),
        ];
        for (c, major, minor) in parts {
            assert_eq!(c as u32, cmd(major, minor), "{c:?}");
            assert_eq!(Cmd::of(c as u32), Some(c));
        }
        assert_eq!(Cmd::of(0x0fff), None);
    }
}
