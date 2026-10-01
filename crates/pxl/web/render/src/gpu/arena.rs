// pxl-render/gpu/arena.rs

//! Where chunk meshes live: on the device alone, in a fixed number of
//! vertex buffers of a fixed size, each mesh at an offset its owner decides
//! by arithmetic (the kernel's world/chunk/mesh_slot.rs). Nothing here is
//! allocated per mesh and nothing is given back; a mesh written over another
//! simply takes its place.
//!
//! There is no copy in the memory. A mesh is written from wherever its bake
//! left it the moment it is finished, and the device is the only one to hold
//! it after. A device that goes away takes every mesh with it: the arena's
//! epoch moves, and whoever wrote into it builds again from what the memory
//! does hold, the terrain records.

use crate::gpu::api::{buffer_usage, gpu, object};
use crate::gpu::renderer::device_ready;
use xpute_runtime::global::Global;

/// Buffers the arena can hold: one a group of levels.
pub const ARENA_GROUPS: usize = 8;

struct Arena {
    /// Each group's bytes, as its owner configured them; 0 past the last.
    sizes: [u32; ARENA_GROUPS],
    /// Each group's buffer on the device, 0 while there is none.
    buffers: [u32; ARENA_GROUPS],
    /// Moves whenever what the buffers held is lost: made anew, or gone with
    /// the device.
    epoch: u32,
}

static ARENA: Global<Arena> = Global::new();

fn arena() -> &'static mut Arena {
    ARENA.get(|| Arena {
        sizes: [0; ARENA_GROUPS],
        buffers: [0; ARENA_GROUPS],
        epoch: 0,
    })
}

/// The groups' sizes in bytes, settled once at boot by whoever lays the
/// meshes out.
pub fn arena_configure(sizes: &[u32]) {
    let a = arena();
    xpute_core::ensure!(sizes.len() <= ARENA_GROUPS, ENOTRECOVERABLE, ARENA_GROUPS);
    a.sizes = [0; ARENA_GROUPS];
    a.sizes[..sizes.len()].copy_from_slice(sizes);
}

/// The bytes the arena holds on the device: every group's buffer made.
pub fn arena_bytes() -> u64 {
    let a = arena();
    a.sizes.iter().zip(&a.buffers).filter(|(_, &b)| b != 0).map(|(&size, _)| size as u64).sum()
}

/// What changes whenever the arena's contents are lost: a mesh written under
/// an older epoch is not there.
pub fn arena_epoch() -> u32 {
    arena().epoch
}

/// Writes `bytes` at `offset` in group `group`, making the buffers first where
/// the device has none. False when there is no device to write to.
pub fn arena_write(group: usize, offset: u32, bytes: &[u8]) -> bool {
    let Some(g) = gpu().filter(|_| device_ready()) else {
        return false;
    };
    let a = arena();
    if a.buffers[0] == 0 {
        for i in 0..ARENA_GROUPS {
            if a.sizes[i] > 0 {
                let id = object();
                g.buffer(id, a.sizes[i], buffer_usage::VERTEX | buffer_usage::COPY_DST);
                a.buffers[i] = id;
            }
        }
        a.epoch = a.epoch.wrapping_add(1);
    }
    xpute_core::ensure!(offset as usize + bytes.len() <= a.sizes[group] as usize, ENOSPC);
    g.write_buffer(a.buffers[group], offset, bytes);
    true
}

/// Group `group`'s buffer, or none where the device has not made it. A draw
/// binds it as a vertex buffer and starts at its own first box as its first
/// instance, so there is no bind group over it and nothing to key by a
/// layout.
pub fn arena_buffer(group: usize) -> Option<u32> {
    let id = arena().buffers[group];
    if id == 0 {
        return None;
    }
    Some(id)
}

/// The device went away with every buffer on it.
pub fn arena_lose() {
    let a = arena();
    a.buffers = [0; ARENA_GROUPS];
    a.epoch = a.epoch.wrapping_add(1);
}
