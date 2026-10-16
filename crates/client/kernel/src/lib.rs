// pxl-kernel/lib.rs

//! pixelet's kernel: the world's state and what runs over it. Nothing here
//! knows of WebAssembly, a browser or threads (pxl-wasm does), so the same
//! code runs under `cargo test` and in the client's module.

pub mod abi;
pub mod approach;
pub mod asset;
pub mod billboard;
pub mod chunk;
pub mod control;
pub mod decor;
pub mod door;
pub mod io;
pub mod layer;
pub mod mem;
pub mod model;
pub mod param;
pub mod profiler;
pub mod query;
pub mod scene;
pub mod sched;
pub mod setting;
pub mod shell;
pub mod turn;
pub mod view;

use crate::billboard::field::Billboards;
use crate::model::registry::Models;
use pxl_render::frame::draw::DrawList;

use crate::layer::object::store::Store;
use crate::scene::Scene;
use xpute_guest::global::Global;

pub struct Kernel {
    /// The memory map is laid; nothing of the world is made before it.
    pub laid_out: bool,
    pub objects: Store,
    pub scene: Scene,
    /// What the renderer is handed: every layer lists into it.
    pub draw: DrawList,
    pub models: Models,
    pub billboards: Billboards,
}

impl Kernel {
    fn new() -> Kernel {
        Kernel {
            laid_out: false,
            objects: Store::default(),
            scene: Scene::default(),
            draw: DrawList::default(),
            models: Models::default(),
            billboards: Billboards::default(),
        }
    }
}

static KERNEL: Global<Kernel> = Global::new();

pub fn kernel() -> &'static mut Kernel {
    KERNEL.get(Kernel::new)
}

/// None before the memory map is laid, when nothing may take the heap.
pub fn laid() -> Option<&'static mut Kernel> {
    let k = kernel();
    k.laid_out.then_some(k)
}

impl Kernel {
    /// Split into fields so the scene and the draw list borrow at once. None
    /// before the memory map is laid.
    pub fn parts(&mut self) -> Option<(&mut Scene, &mut DrawList)> {
        let Kernel { laid_out, scene, draw, .. } = self;
        if !*laid_out {
            return None;
        }
        Some((scene, draw))
    }
}

/// Native tests run on several threads, and a `Global` is for one.
#[cfg(test)]
pub fn serial() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

#[cfg(test)]
#[path = "fixture.test.rs"]
pub(crate) mod fixture;
