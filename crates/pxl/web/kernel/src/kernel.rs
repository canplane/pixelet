// pxl-kernel/kernel.rs
//
// The kernel's one state: the volumes, the object store and its painter, the
// scheduler and the change log — as one value made on first use. The
// kernel's modules read and write it directly, and hand the view
// (pxl-render) what it draws; pxl-wasm keeps only the rings and hands
// commands to it.

use pxl_render::mesh::paint::object::adapter::Painter;
use pxl_render::mesh::volume::Volumes;
use xpute_runtime::ipc::change::{ChangeLog, ChangeLogOptions};

use crate::global::Global;
use crate::layer::object::store::Store;
use crate::world::chunk::table::Scheduler;

pub struct Kernel {
    /// The volumes over the heap, made at boot.
    pub volumes: Option<Volumes>,
    /// The object store and its painter.
    pub objects: Store,
    pub painter: Painter,
    /// The chunks' builds.
    pub sched: Scheduler,
    /// The world's change log (world/change.rs's topics).
    pub changes: ChangeLog,
}

impl Kernel {
    fn new() -> Kernel {
        Kernel {
            volumes: None,
            objects: Store::new(),
            painter: Painter::new(),
            sched: Scheduler::new(),
            changes: ChangeLog::new(ChangeLogOptions {
                topics: crate::world::change::WORLD_TOPICS,
                capacity: crate::param::JOURNAL_CAPACITY,
                keys_per_topic: None,
            }),
        }
    }
}

static KERNEL: Global<Kernel> = Global::new();

/// The kernel.
pub fn kernel() -> &'static mut Kernel {
    KERNEL.get(Kernel::new)
}

impl Kernel {
    /// The world over the kernel's volumes, objects and painter, with the
    /// scheduler beside it: each a field of its own, so the two are borrowed
    /// at once. None before the volumes are opened.
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

/// The kernel's world and its scheduler, borrowed together for a call that
/// builds, draws or reads the volumes; None before the volumes are opened.
pub fn with_world<R>(f: impl FnOnce(&mut crate::world::chunk::build::World, &mut Scheduler) -> R) -> Option<R> {
    let (mut world, sched) = kernel().world()?;
    Some(f(&mut world, sched))
}
