// pxl-kernel/kernel.rs
//
// The kernel's one state: the volumes, the terrain tables, the object store
// and its painter, the scheduler, the change log and the raycast — as one
// value made on first use. The kernel's modules read and write it
// directly, and hand the view (pxl-render) what it draws; pxl-wasm keeps only
// the rings and hands commands to it.

use pxl_render::mesh::paint::object::adapter::Painter;
use pxl_render::mesh::paint::terrain::adapter::Materials;
use pxl_render::mesh::volume::Volumes;
use xpute_runtime::ipc::change::{ChangeLog, ChangeLogOptions};

use crate::global::Global;
use crate::layer::object::store::Store;
use crate::world::chunk::raycast::Raycast;
use crate::world::chunk::table::Scheduler;

/// The log's shape: world/change.rs's topics, and how many changes it keeps.
pub const JOURNAL_CAPACITY: u32 = 4096;

pub struct Kernel {
    /// The volumes over the heap, made at boot.
    pub volumes: Option<Volumes>,
    /// The terrain tables.
    pub terrain: Materials,
    /// The object store and its painter.
    pub objects: Store,
    pub painter: Painter,
    /// The chunks' builds.
    pub sched: Scheduler,
    /// The world's change log (world/change.rs's topics).
    pub changes: ChangeLog,
    /// The world's raycast and its kept composites.
    pub raycast: Raycast,
}

impl Kernel {
    fn new() -> Kernel {
        Kernel {
            volumes: None,
            terrain: Materials::default(),
            objects: Store::new(),
            painter: Painter::new(),
            sched: Scheduler::new(),
            raycast: Raycast::default(),
            changes: ChangeLog::new(ChangeLogOptions {
                topics: crate::world::change::WORLD_TOPICS,
                capacity: JOURNAL_CAPACITY,
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

/// The kernel's world and its scheduler, borrowed together for a call that
/// builds, draws or reads the volumes; None before the volumes are opened.
pub fn with_world<R>(f: impl FnOnce(&mut crate::world::chunk::node::World, &mut crate::world::chunk::table::Scheduler) -> R) -> Option<R> {
    let k = kernel();
    let volumes = k.volumes.as_mut()?;
    let mut world = crate::world::chunk::node::World {
        volumes,
        materials: &k.terrain,
        store: &k.objects,
        painter: &mut k.painter,
    };
    Some(f(&mut world, &mut k.sched))
}
