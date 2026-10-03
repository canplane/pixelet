// pxl-kernel/kernel.rs

use pxl_render::mesh::paint::object::adapter::Painter;
use pxl_render::mesh::volume::Volumes;

use crate::layer::object::store::Store;
use crate::world::change::Revisions;
use crate::world::chunk::table::Scheduler;
use xpute_runtime::global::Global;

pub struct Kernel {
    pub volumes: Option<Volumes>,
    pub objects: Store,
    pub painter: Painter,
    pub sched: Scheduler,
    pub changes: Revisions,
}

impl Kernel {
    fn new() -> Kernel {
        Kernel {
            volumes: None,
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
    /// the volumes are opened.
    pub fn world(&mut self) -> Option<(crate::world::chunk::build::World<'_>, &mut Scheduler)> {
        let Kernel { volumes, objects, painter, sched, .. } = self;
        let world = crate::world::chunk::build::World {
            volumes: volumes.as_mut()?,
            store: objects,
            painter,
        };
        Some((world, sched))
    }
}

pub fn with_world<R>(f: impl FnOnce(&mut crate::world::chunk::build::World, &mut Scheduler) -> R) -> Option<R> {
    let (mut world, sched) = kernel().world()?;
    Some(f(&mut world, sched))
}
