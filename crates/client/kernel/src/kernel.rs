// pxl-kernel/kernel.rs

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
