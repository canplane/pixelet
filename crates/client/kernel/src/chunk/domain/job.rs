// pxl-kernel/chunk/domain/job.rs

//! The goal as the turns see it: one walk started when what it reads moved,
//! run a slice a turn, and published whole.

use pxl_spec::space::coord::world::nearest_box;

use pxl_spec::space::camera::eye::Observation;
use pxl_spec::space::camera::grid::Tau;
use pxl_spec::space::coord::aabb::Aabb;

use crate::layer::residency::floor;
use crate::query::World;
use crate::view::observation::observation;
use crate::view::sight::{EdgeRays, Sight};
use xpute_guest::global::Global;

use crate::chunk::Chunk;
use pxl_spec::space::coord::zoom::Zoom;

use super::walk::Domain;
use super::Price;

#[derive(Default)]
struct Made {
    /// whole
    published: Option<Domain>,
    /// sliced
    job: Option<Domain>,
    spare: Option<Domain>,
    /// What the goal was started from: the observation, the finest step and
    /// the source's top, and each chunk's mesh slots where more than one.
    key: Option<(Observation, Tau, Zoom)>,
    priced: Vec<(Chunk, u32)>,
    /// Where the ground or the objects moved since the goal was started.
    moved_in: Vec<Aabb>,
    /// The squares recast, kept so a start allocates nothing.
    moved: Vec<(f64, f64, f64)>,
    sight: Sight,
    /// rays cast and nodes visited since `take_work`
    work: [u32; 2],
}

static MADE: Global<Made> = Global::new();

/// Starts the next goal's job, casting the screen's rays for it, when the
/// observation, the heights, the target or what a chunk is priced at moved;
/// a job is never restarted. `priced` is sorted.
pub fn start(ladder: &[Tau], priced: &[(Chunk, u32)]) {
    let made = MADE.get(Made::default);
    if made.job.is_some() {
        return;
    }
    let (Some(o), Some(top)) = (observation(), floor::top()) else { return };
    let key = (o, ladder[0], top);
    if made.key == Some(key) && made.moved_in.is_empty() && made.priced == priced && made.published.is_some() {
        return;
    }
    // a new target recasts every patch; moved tiles and written objects the patches over them
    let everything = made.key.is_none_or(|(_, tau, _)| tau != key.1);
    made.key = Some(key);
    let near = |r: &Aabb| {
        let r = nearest_box(r, o.eye[0]);
        (r.min[0], r.min[1], r.max[0] - r.min[0])
    };
    let mut moved = core::mem::take(&mut made.moved);
    moved.clear();
    moved.extend(made.moved_in.drain(..).map(|r| near(&r)));
    made.work[0] += made.sight.observe(&o, &EdgeRays(World::now(), ladder[0]), everything, &moved);
    made.moved = moved;
    made.priced.clear();
    made.priced.extend_from_slice(priced);
    let mut job = made.spare.take().unwrap_or_else(|| Domain::empty(o, World::now()));
    job.begin(o, top, ladder, made.sight.footprints(), made.sight.lacks());
    made.job = Some(job);
}

/// A slice of the started goal; publishes it whole.
pub fn advance(at: usize, price: &dyn Price, stop: &dyn Fn() -> bool) -> bool {
    let made = MADE.get(Made::default);
    let Some(job) = made.job.as_mut() else { return false };
    if !job.run(price, stop, &mut made.work[1]) {
        return false;
    }
    let Some(mut done) = made.job.take() else { return false };
    done.put(at);
    made.spare = made.published.replace(done);
    true
}

/// Objects changed in `regions`: the next goal recasts the patches over them.
/// Where the ground or the objects moved: the next goal recasts the patches over it.
pub fn moved(regions: impl IntoIterator<Item = Aabb>) {
    MADE.get(Made::default).moved_in.extend(regions);
}

pub fn take_work() -> [u32; 2] {
    core::mem::take(&mut MADE.get(Made::default).work)
}

pub fn put(k: usize) {
    if let Some(d) = MADE.get(Made::default).published.as_mut() {
        d.put(k);
    }
}

pub fn walking() -> bool {
    MADE.get(Made::default).job.is_some()
}

pub fn current() -> Option<&'static Domain> {
    MADE.get(Made::default).published.as_ref()
}
