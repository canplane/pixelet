// pxl-render/gpu/put.rs

//! Words as the device is handed them: a value put at its byte or float, and
//! the scratch block a pass puts them in.

use pxl_spec::space::coord::mat4::Mat4;
use xpute_guest::mem::base::ptr_at;
use xpute_guest::mem::heap::{alloc, free};
use xpute_kit::status::bug::OrBug;
use xpute_kit::status::errno::Errno;

pub fn put_f32(buf: &mut [u8], at: usize, v: f32) {
    buf[at..at + 4].copy_from_slice(&v.to_le_bytes());
}

pub fn put_u32(buf: &mut [u8], at: usize, v: u32) {
    buf[at..at + 4].copy_from_slice(&v.to_le_bytes());
}

pub(crate) fn put_mat(out: &mut [f32], at: usize, m: &Mat4) {
    for (i, v) in m.iter().enumerate() {
        out[at + i] = *v as f32;
    }
}

/// Grown to the next power of two when a frame needs more, never shrunk.
#[derive(Default)]
pub struct Scratch {
    at: usize,
    bytes: u32,
}

impl Scratch {
    pub fn take(&mut self, bytes: u32) -> &'static mut [u8] {
        if bytes > self.bytes {
            if self.bytes > 0 {
                // SAFETY: the block this scratch held alone.
                unsafe { free((self.at, self.bytes)) };
            }
            let size = bytes.next_power_of_two().max(4096);
            (self.at, self.bytes) = alloc(size).or_bug(Errno::ENOMEM);
        }
        // SAFETY: a heap block this scratch holds alone, at least `bytes` long.
        unsafe { core::slice::from_raw_parts_mut(ptr_at::<u8>(self.at), bytes as usize) }
    }

    pub fn free(&mut self) {
        if self.bytes > 0 {
            // SAFETY: the block this scratch held alone.
            unsafe { free((self.at, self.bytes)) };
            self.bytes = 0;
        }
    }
}

/// Its block goes with it: the renderer is dropped whole when the GPU is lost.
impl Drop for Scratch {
    fn drop(&mut self) {
        self.free();
    }
}
