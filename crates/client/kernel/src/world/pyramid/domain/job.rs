// pxl-kernel/world/pyramid/domain/job.rs

//! The goal as the turns see it: one walk started when what it reads moved,
//! run a slice a turn, and published whole.

use pxl_spec::space::coord::world::nearest_box;

use pxl_spec::space::camera::eye::Observation;
use pxl_spec::space::camera::grid::Tau;
use pxl_spec::space::coord::v3::Aabb;
use pxl_spec::space::layer::tile::Tile;

use crate::world::access::{Access, Query};
use crate::world::observation::{ground_known, observation};
use crate::world::observer::{EdgeRays, Sight};
use crate::world::residency::floor;
use xpute_runtime::global::Global;

use crate::world::pyramid::chunk::Chunk;
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
    /// the source's top, the ground's tiles known, and each chunk's mesh slots
    /// where more than one.
    key: Option<(Observation, Tau, Zoom)>,
    terrain: Vec<(Tile, bool)>,
    priced: Vec<(Chunk, u32)>,
    /// The ground's tiles known this turn; kept so a turn allocates nothing.
    now: Vec<(Tile, bool)>,
    /// Where objects were written since the goal was started.
    written: Vec<Aabb>,
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
    ground_known(&mut made.now);
    if made.key == Some(key) && made.now == made.terrain && made.written.is_empty() && made.priced == priced && made.published.is_some() {
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
    moved.extend(changed(&made.terrain, &made.now).map(|p| p.region()).chain(made.written.drain(..)).map(|r| near(&r)));
    made.work[0] += made.sight.observe(&o, &EdgeRays(Access::now(), ladder[0]), everything, &moved);
    made.moved = moved;
    core::mem::swap(&mut made.terrain, &mut made.now);
    made.priced.clear();
    made.priced.extend_from_slice(priced);
    let mut job = made.spare.take().unwrap_or_else(|| Domain::empty(o, Box::new(|node, cell| Access::now().bounds(node, cell))));
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

/// The tiles whose state differs between two sorted lists of what is known.
fn changed<'a>(was: &'a [(Tile, bool)], now: &'a [(Tile, bool)]) -> impl Iterator<Item = &'a Tile> {
    let (mut i, mut j) = (0, 0);
    std::iter::from_fn(move || loop {
        match (was.get(i), now.get(j)) {
            (None, None) => return None,
            (Some(a), None) => {
                i += 1;
                return Some(&a.0);
            }
            (None, Some(b)) => {
                j += 1;
                return Some(&b.0);
            }
            (Some(a), Some(b)) if a.0 < b.0 => {
                i += 1;
                return Some(&a.0);
            }
            (Some(a), Some(b)) if b.0 < a.0 => {
                j += 1;
                return Some(&b.0);
            }
            (Some(a), Some(b)) => {
                (i, j) = (i + 1, j + 1);
                if a.1 != b.1 {
                    return Some(&a.0);
                }
            }
        }
    })
}

/// Objects changed in `regions`: the next goal recasts the patches over them.
pub fn objects_written(regions: &[Aabb]) {
    MADE.get(Made::default).written.extend_from_slice(regions);
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
