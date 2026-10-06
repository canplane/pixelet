// pxl-kernel/kernel.rs

use pxl_render::mesh::paint::object::adapter::Painter;

use crate::layer::object::store::Store;
use crate::world::change::Revisions;
use crate::world::chunk::table::Scheduler;
use xpute_runtime::global::Global;

pub struct Kernel {
    /// The memory map is laid; nothing of the world is made before it.
    pub laid_out: bool,
    pub objects: Store,
    pub painter: Painter,
    pub sched: Scheduler,
    pub changes: Revisions,
}

impl Kernel {
    fn new() -> Kernel {
        Kernel {
            laid_out: false,
            objects: Store::default(),
            painter: Painter::new(),
            sched: Scheduler::new(),
            changes: Revisions::default(),
        }
    }
}

static KERNEL: Global<Kernel> = Global::new();

pub fn kernel() -> &'static mut Kernel {
    KERNEL.get(Kernel::new)
}

impl Kernel {
    /// Split into fields so the world and the table borrow at once. None before
    /// the memory map is laid.
    pub fn world(&mut self) -> Option<(crate::world::chunk::build::World<'_>, &mut Scheduler)> {
        let Kernel {
            laid_out, objects, painter, sched, ..
        } = self;
        if !*laid_out {
            return None;
        }
        Some((crate::world::chunk::build::World { store: objects, painter }, sched))
    }
}

pub fn with_world<R>(f: impl FnOnce(&mut crate::world::chunk::build::World, &mut Scheduler) -> R) -> Option<R> {
    let (mut world, sched) = kernel().world()?;
    Some(f(&mut world, sched))
}
