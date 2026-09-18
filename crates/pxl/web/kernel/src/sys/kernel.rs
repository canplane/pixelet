// pxl-kernel/sys/kernel.rs
//
// The kernel's one state: the volumes, the terrain tables, the object store
// and its painter, the scheduler, the change log and the raycast — as one
// value made on first use. The kernel's modules read and write it
// directly, and hand the view (pxl-render) what it draws; pxl-wasm keeps only
// the rings and hands commands to it.

use pxl_render::mesh::paint::object::adapter::Painter;
use pxl_render::mesh::paint::terrain::adapter::Materials;
use pxl_render::mesh::volume::Volumes;
use pxl_spec::space::pvf3::ivec::{self, gather};
use xpute_core::ipc::change::{ChangeLog, ChangeLogOptions};

use crate::layer::object::store::Store;
use crate::sys::global::Global;
use crate::world::chunk::raycast::Raycast;
use crate::world::chunk::table::Scheduler;

/// world/change.rs's WorldTopic numbers the store bumps, and the
/// log's shape.
pub const TOPIC_OBJECT_COLUMN: u32 = 0;
pub const TOPIC_DECOR_COLUMN: u32 = 1;
pub const TOPIC_OBJECT_CELL: u32 = 2;
pub const WORLD_TOPICS: u32 = 6;
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
                topics: WORLD_TOPICS,
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

/// Re-derives the chunk's entries and cells, and records the change: the
/// column in both of its topics, and every cell whose entries changed, by
/// its corner.
pub fn reindex_object(k: &mut Kernel, key: u64) {
    k.objects.reindex(key);
    crate::world::chunk::node::chunk_sources_changed();
    k.changes.bump(TOPIC_OBJECT_COLUMN, key as i64);
    k.changes.bump(TOPIC_DECOR_COLUMN, key as i64);
    let (at, n) = k.objects.take_touched();
    let (x0, y0) = (ivec::X(key), ivec::Y(key));
    for r in 0..n {
        // SAFETY: the rows take_touched just handed back, three words each.
        let row = unsafe { core::slice::from_raw_parts(at.add(r * 3), 3) };
        let corner = gather(x0 + row[0] as i64, y0 + row[1] as i64, row[2] as i64);
        k.changes.bump(TOPIC_OBJECT_CELL, corner as i64);
    }
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
