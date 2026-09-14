// pxl_core/kernel.rs
//
// The kernel's one state: every structure the world is made of — the bakes,
// the volumes and the pools they draw on, the terrain tables, the object
// store, the scheduler, the change log, the raycast — as one value made on
// first use. The modules translated from the client (module/, app/, render/)
// read and write it directly, as their TypeScript read its stores; pxl-wasm
// keeps only the rings and hands commands to it.

use crate::bake::mesher::BakeContext;
use crate::bake::scan::{ColumnRuns, ScanContext};
use crate::bake::terrain::{ChunkSample, Materials};
use crate::bake::volume::{Volumes, BAND_WORDS as VOLUME_BAND_WORDS};
use crate::global::Global;
use crate::world::object::{Painter, Store};
use crate::world::pool::Pool;
use crate::world::raycast::Raycast;
use crate::world::sched::Scheduler;
use pxl_shared::spec::space::pvf3::ivec::{self, gather};
use xpute_core::ipc::change::{ChangeLog, ChangeLogOptions};

/// module/world/change.rs's WorldTopic numbers the store bumps, and the
/// log's shape.
pub const TOPIC_OBJECT_COLUMN: u32 = 0;
pub const TOPIC_DECOR_COLUMN: u32 = 1;
pub const TOPIC_OBJECT_CELL: u32 = 2;
pub const WORLD_TOPICS: u32 = 6;
pub const JOURNAL_CAPACITY: u32 = 4096;

/// Doubles in the frame block, in the order sched::Frame names them: the
/// root matrix (16), the origin (3), the planes (24), the camera (3), the
/// lead (3), then viewport height, fov, delta seconds, budget, hard slack,
/// pixel dither and the retain budget, then the range anchor's x and y,
/// whether there is one, the render distance and the range hysteresis, and
/// whether decor is scattered.
pub const FRAME_WORDS: usize = 62;

pub struct Kernel {
    pub ctx: BakeContext,
    /// The open bake's run set: its address in the memory, and its shape.
    pub input: (usize, u32, u32),
    /// vertex_cnt, index_cnt, the six counters, then the heap's use.
    pub result: [u32; 9],
    pub scan: ScanContext,
    /// The open scan's band table: its address, and how many bands.
    pub bands: (usize, u32),
    /// The pools over the memory's sections, by the id POOL_CREATE replied.
    pub pools: Vec<Pool>,
    /// The volumes, once VOLUME_POOLS has said which pools they draw on.
    pub volumes: Option<Volumes>,
    /// The open scan's bands, when it reads a volume of the module's.
    pub scan_rows: Vec<[u32; VOLUME_BAND_WORDS]>,
    /// The terrain tables, and the last chunk read.
    pub terrain: Materials,
    pub chunk: ChunkSample,
    /// The object store, its painter, and the rows its lookups answer in.
    pub objects: Store,
    pub painter: Painter,
    /// The raycast's column, and the row naming its lanes.
    pub column: ColumnRuns,
    /// The chunks' builds.
    pub sched: Scheduler,
    /// The world's change log (module/world/change.rs's topics).
    pub changes: ChangeLog,
    /// The frame the glue writes before each tick (FRAME_WORDS doubles).
    pub frame: [f64; FRAME_WORDS],
    pub census: [f64; 13],
    pub build_stat: [u32; 11],
    /// The world's raycast and its kept composites.
    pub raycast: Raycast,
}

impl Kernel {
    fn new() -> Kernel {
        Kernel {
            ctx: BakeContext::new(),
            input: (0, 0, 0),
            result: [0; 9],
            scan: ScanContext::new(),
            bands: (0, 0),
            pools: Vec::new(),
            volumes: None,
            scan_rows: Vec::new(),
            terrain: Materials::default(),
            chunk: ChunkSample::default(),
            objects: Store::new(),
            painter: Painter::new(),
            column: ColumnRuns::new(),
            sched: Scheduler::new(),
            raycast: Raycast::default(),
            changes: ChangeLog::new(ChangeLogOptions {
                topics: WORLD_TOPICS,
                capacity: JOURNAL_CAPACITY,
                keys_per_topic: None,
                alloc: None,
            }),
            frame: [0.0; FRAME_WORDS],
            census: [0.0; 13],
            build_stat: [0; 11],
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
pub fn with_world<R>(f: impl FnOnce(&mut crate::world::build::World, &mut crate::world::sched::Scheduler) -> R) -> Option<R> {
    let k = kernel();
    let volumes = k.volumes.as_mut()?;
    let mut world = crate::world::build::World {
        volumes,
        pools: &mut k.pools,
        materials: &k.terrain,
        store: &k.objects,
        painter: &mut k.painter,
    };
    Some(f(&mut world, &mut k.sched))
}
