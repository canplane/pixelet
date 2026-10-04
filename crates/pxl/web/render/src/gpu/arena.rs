// pxl-render/gpu/arena.rs

//! Chunk meshes live only on the device, in fixed buffers at offsets their
//! owner computes; nothing is allocated per mesh. When the device is lost the
//! epoch moves and owners rebuild from the terrain records.

use crate::gpu::api::{buffer_usage, gpu, object};
use xpute_runtime::global::Global;

/// One buffer a group of levels.
pub const ARENA_GROUPS: usize = 8;

struct Arena {
    sizes: [u32; ARENA_GROUPS],
    buffers: [u32; ARENA_GROUPS],
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

pub fn arena_configure(sizes: &[u32]) {
    let a = arena();
    xpute_core::ensure!(sizes.len() <= ARENA_GROUPS, ENOTRECOVERABLE, ARENA_GROUPS);
    a.sizes = [0; ARENA_GROUPS];
    a.sizes[..sizes.len()].copy_from_slice(sizes);
}

pub fn arena_bytes() -> u64 {
    let a = arena();
    a.sizes.iter().zip(&a.buffers).filter(|(_, &b)| b != 0).map(|(&size, _)| size as u64).sum()
}

/// A mesh written under an older epoch is not there.
pub fn arena_epoch() -> u32 {
    arena().epoch
}

/// False when there is no device.
pub fn arena_write(group: usize, offset: u32, bytes: &[u8]) -> bool {
    let Some(g) = gpu().filter(|_| crate::gpu::renderer::device_ready()) else {
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

pub fn arena_buffer(group: usize) -> Option<u32> {
    let id = arena().buffers[group];
    if id == 0 {
        return None;
    }
    Some(id)
}

pub fn arena_lose() {
    let a = arena();
    a.buffers = [0; ARENA_GROUPS];
    a.epoch = a.epoch.wrapping_add(1);
}
