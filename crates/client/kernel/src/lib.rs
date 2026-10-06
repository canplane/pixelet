// pxl-kernel/lib.rs

//! pixelet's kernel: the world's state and what runs over it. Nothing here
//! knows of WebAssembly, a browser or threads (pxl-wasm does), so the same
//! code runs under `cargo test` and in the client's module.

pub mod abi;
pub mod control;
pub mod door;
pub mod io;
pub mod layer;
pub mod mem;
pub mod param;
pub mod scene;
pub mod sched;
pub mod shell;
pub mod world;

use pxl_render::mesh::paint::object::Painter;

use crate::layer::object::store::Store;
use crate::scene::Scene;
use crate::world::chunk::table::Scheduler;
use xpute_runtime::global::Global;

pub struct Kernel {
    /// The memory map is laid; nothing of the world is made before it.
    pub laid_out: bool,
    pub objects: Store,
    pub painter: Painter,
    pub sched: Scheduler,
    pub scene: Scene,
}

impl Kernel {
    fn new() -> Kernel {
        Kernel {
            laid_out: false,
            objects: Store::default(),
            painter: Painter::new(),
            sched: Scheduler::new(),
            scene: Scene::default(),
        }
    }
}

static KERNEL: Global<Kernel> = Global::new();

pub fn kernel() -> &'static mut Kernel {
    KERNEL.get(Kernel::new)
}

/// The objects and the painter, borrowed together for a turn's build.
pub struct World<'a> {
    pub store: &'a crate::layer::object::store::Store,
    pub painter: &'a mut pxl_render::mesh::paint::object::Painter,
}

impl Kernel {
    /// Split into fields so the world and the table borrow at once. None before
    /// the memory map is laid.
    pub fn world(&mut self) -> Option<(World<'_>, &mut Scheduler, &mut Scene)> {
        let Kernel {
            laid_out,
            objects,
            painter,
            sched,
            scene,
        } = self;
        if !*laid_out {
            return None;
        }
        Some((World { store: objects, painter }, sched, scene))
    }
}

pub fn with_world<R>(f: impl FnOnce(&mut World, &mut Scheduler, &mut Scene) -> R) -> Option<R> {
    let (mut world, sched, scene) = kernel().world()?;
    Some(f(&mut world, sched, scene))
}

/// Native tests run on several threads, and a `Global` is for one.
#[cfg(test)]
pub fn serial() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

#[cfg(test)]
#[path = "allocations.test.rs"]
pub mod allocations;
