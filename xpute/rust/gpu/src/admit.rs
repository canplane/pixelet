// xpute-gpu/admit.rs

//! Whether the GPU's host takes a frame now without blocking: a guest draws a
//! frame only while it is admitted. A host in the guest's own process hands
//! the guest's renderer its answer (`host` `GpuHost::admit`); a page's admits
//! every frame (`every`), since the browser paces a page's frames and taking
//! a canvas's texture never blocks.

pub type Admit = fn() -> bool;

pub fn every() -> bool {
    true
}
