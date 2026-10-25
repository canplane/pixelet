// pxl-kernel/lib.rs

//! pixelet's kernel: the world's state and what runs over it. Nothing here
//! knows of WebAssembly, a browser or threads (pxl-wasm does), so the same
//! code runs under `cargo test` and in the client's module.

pub mod domain;
pub mod door;
pub mod profiler;
pub mod shell;
pub mod sys;
pub mod turn;

use pxl_render::frame::draw::DrawList;

use crate::domain::scene::Scene;
use xpute_guest::global::Global;

pub struct Kernel {
    pub scene: Scene,
    /// What the renderer is handed: every layer lists into it.
    pub draw: DrawList,
}

impl Kernel {
    fn new() -> Kernel {
        Kernel {
            scene: Scene::default(),
            draw: DrawList::default(),
        }
    }
}

static KERNEL: Global<Kernel> = Global::new();

pub fn kernel() -> &'static mut Kernel {
    KERNEL.get(Kernel::new)
}

/// None before the memory map is laid (`mem::section::map`), when nothing
/// may take the heap.
pub fn laid() -> Option<&'static mut Kernel> {
    crate::sys::mem::section::map().map(|_| kernel())
}

impl Kernel {
    /// Split into fields so the scene and the draw list borrow at once. None
    /// before the memory map is laid.
    pub fn parts(&mut self) -> Option<(&mut Scene, &mut DrawList)> {
        crate::sys::mem::section::map()?;
        let Kernel { scene, draw } = self;
        Some((scene, draw))
    }
}

/// Native tests run on several threads, and a `Global` is for one.
#[cfg(test)]
pub fn serial() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    LOCK.lock().unwrap_or_else(|e| e.into_inner())
}
