// pxl-kernel/world/chunk/raycast.rs

//! The world's one raycast, and the little cache of composites it walks.
//!
//! A ray walks the chunks its XY projection crosses (a DDA a chunk wide),
//! and in each the cells it crosses (a DDA a cell wide), testing exactly
//! the boxes the mesher emits: the scan's own column walk over the chunk's
//! composite of the real set — no hover edits, or the raycaster stops
//! hitting what the cursor points at and the hide flickers on and off. The
//! composites are kept between hovers, a few at a time, each valid for the
//! sources its chunk was last told; the cursor does not teleport between
//! frames, so the same chunks come up again and again.

use pxl_render::mesh::volume::{unpack_sub_cell_fraction, MaterialClass, SUB_CELL_NONE};
use pxl_render::mesh::voxel_scan::{BandView, ColumnRuns};
use pxl_spec::space::pyramid::CHUNK_CELLS;

use pxl_spec::space::pvf3::ivec::{self, IVec};
use pxl_spec::space::v3::V3;
use xpute_core::math::vec3::Vec3;

use crate::kernel::kernel;
use crate::world::chunk::node::World;
use crate::world::chunk::table::Scheduler;

/// A chunk's side, in units.
const CHUNK_W: f64 = CHUNK_CELLS as f64;
/// A hover ray crosses at most a few chunks at the interact radius.
const MAX_ENTRIES: usize = 4;
/// Hard budget: at 1.25 MiB each, eight direct bands are about 10 MiB.
const MAX_BANDS: usize = 8;

/// What a ray struck: the box, in the ray's own space, and its face.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hit {
    pub material_class: u8,
    pub normal: [f64; 3],
    pub point: V3,
    pub t: f64,
    pub box_min: V3,
    pub box_max: V3,
}

struct Entry {
    /// The chunk corner it is the composite of, in units.
    base: (i64, i64),
    /// What it was made valid for (the builder's compose_leaf).
    valid: u32,
    volume: usize,
    bands: usize,
    used: u32,
}

#[derive(Default)]
pub struct Raycast {
    entries: Vec<Entry>,
    tick: u32,
    column: ColumnRuns,
    bands: Vec<BandView<'static>>,
}

struct Slab {
    t: f64,
    axis: usize,
    sign: f64,
}

/// Ray/AABB slab test: the entry and exit parameters along the ray, and
/// which axis the entry was on — the caller wants the axis to know which
/// face was hit.
fn ray_aabb(o: &V3, d: &[f64; 3], min: &V3, max: &V3) -> Option<Slab> {
    let mut t_enter = f64::NEG_INFINITY;
    let mut t_exit = f64::INFINITY;
    let mut enter_axis = 0;
    let mut enter_sign = -1.0;
    for axis in 0..3 {
        let dir = d[axis];
        if dir.abs() < 1e-12 {
            if o[axis] < min[axis] || o[axis] > max[axis] {
                return None;
            }
            continue;
        }
        let t_lo = (min[axis] - o[axis]) / dir;
        let t_hi = (max[axis] - o[axis]) / dir;
        let (near, far) = if dir > 0.0 { (t_lo, t_hi) } else { (t_hi, t_lo) };
        if near > t_enter {
            t_enter = near;
            enter_axis = axis;
            enter_sign = if dir > 0.0 { -1.0 } else { 1.0 };
        }
        t_exit = t_exit.min(far);
        if t_enter > t_exit {
            return None;
        }
    }
    if t_exit < 0.0 {
        return None;
    }
    Some(Slab {
        t: t_enter,
        axis: enter_axis,
        sign: enter_sign,
    })
}

/// The box run `i` of the column is, chunk-local: a whole cell where the run
/// carries no sub-cell size, and the sub-cell's own box where it does.
fn run_box(cx: u32, cy: u32, runs: &ColumnRuns, i: usize) -> (V3, V3) {
    let sub = runs.sub[i];
    let sub_sz = (sub >> 24) as u8;
    if sub_sz == SUB_CELL_NONE {
        let min = [cx as f64, cy as f64, runs.z0[i] as f64];
        let max = [(cx + 1) as f64, (cy + 1) as f64, (runs.z1[i] + 1) as f64];
        return (min, max);
    }
    let sz = unpack_sub_cell_fraction(sub_sz);
    let min = [
        (cx as f64 + unpack_sub_cell_fraction((sub & 255) as u8)),
        (cy as f64 + unpack_sub_cell_fraction(((sub >> 8) & 255) as u8)),
        (runs.z0[i] as f64 + unpack_sub_cell_fraction(((sub >> 16) & 255) as u8)),
    ];
    (min, [min[0] + sz, min[1] + sz, min[2] + sz])
}

/// A walk over a square grid of `size` a side from (ox, oy) along
/// (dx, dy), one cell at a time in the order the ray crosses them
/// (Amanatides–Woo): the chunk grid and then the cell grid of a chunk.
struct GridWalk {
    x: f64,
    y: f64,
    step_x: f64,
    step_y: f64,
    t_delta_x: f64,
    t_delta_y: f64,
    t_max_x: f64,
    t_max_y: f64,
}

impl GridWalk {
    fn new(ox: f64, oy: f64, dx: f64, dy: f64, size: f64) -> GridWalk {
        let x = (ox / size).floor();
        let y = (oy / size).floor();
        let axis = |d: f64, c: f64, o: f64| {
            if d.abs() < 1e-9 {
                (f64::INFINITY, f64::INFINITY)
            } else {
                (size / d.abs(), ((if d > 0.0 { c + 1.0 } else { c }) * size - o) / d)
            }
        };
        let (t_delta_x, t_max_x) = axis(dx, x, ox);
        let (t_delta_y, t_max_y) = axis(dy, y, oy);
        GridWalk {
            x,
            y,
            step_x: if dx > 0.0 { 1.0 } else { -1.0 },
            step_y: if dy > 0.0 { 1.0 } else { -1.0 },
            t_delta_x,
            t_delta_y,
            t_max_x,
            t_max_y,
        }
    }

    /// The distance along the ray to the edge of the cell it is in.
    fn next_t(&self) -> f64 {
        self.t_max_x.min(self.t_max_y)
    }

    fn advance(&mut self) {
        if self.t_max_x < self.t_max_y {
            self.x += self.step_x;
            self.t_max_x += self.t_delta_x;
        } else {
            self.y += self.step_y;
            self.t_max_y += self.t_delta_y;
        }
    }
}

impl Raycast {
    /// The nearest box within `max_dist` along the ray from `o` in
    /// direction `d`, both relative to `origin` (the map origin, in units)
    /// on X and Y, or none. Chunks the scheduler holds are tested; a
    /// chunk with no page, or whose composite the heap refused, holds
    /// nothing.
    pub fn hit(&mut self, world: &mut World, sched: &mut Scheduler, origin: [f64; 2], o: V3, d: [f64; 3], max_dist: f64) -> Option<Hit> {
        let (dx, dy) = (d[0], d[1]);
        if dx.abs() < 1e-9 && dy.abs() < 1e-9 {
            return self.test_chunk(world, sched, origin, &o, &d, (o[0] / CHUNK_W).floor(), (o[1] / CHUNK_W).floor(), max_dist);
        }
        let mut walk = GridWalk::new(o[0], o[1], dx, dy, CHUNK_W);
        let max_chunks = (max_dist / CHUNK_W).ceil() as usize + 2;
        for _ in 0..max_chunks {
            if let Some(hit) = self.test_chunk(world, sched, origin, &o, &d, walk.x, walk.y, max_dist) {
                return Some(hit);
            }
            if walk.next_t() > max_dist {
                break;
            }
            walk.advance();
        }
        None
    }

    /// The chunk at grid cell (gx, gy) of the ray's space: its composite,
    /// kept or made, then the cell walk through it.
    #[allow(clippy::too_many_arguments)]
    fn test_chunk(&mut self, world: &mut World, sched: &mut Scheduler, origin: [f64; 2], o: &V3, d: &[f64; 3], gx: f64, gy: f64, max_dist: f64) -> Option<Hit> {
        // A point inside the grid cell, then the chunk it falls in.
        let bx = (((gx + 0.5) * CHUNK_W + origin[0]) / CHUNK_W).floor() * CHUNK_W;
        let by = (((gy + 0.5) * CHUNK_W + origin[1]) / CHUNK_W).floor() * CHUNK_W;
        let (volume, temporary) = self.acquire(world, sched, (bx as i64, by as i64))?;
        let hit = self.hit_chunk(world, volume, o, d, bx - origin[0], by - origin[1], max_dist);
        if temporary {
            world.volumes.release(volume);
        }
        hit
    }

    /// The leaf composite at a chunk corner: the kept one while what it was
    /// made from is unchanged, else made now and kept unless too tall to
    /// keep. Least recently used is let go first.
    fn acquire(&mut self, world: &mut World, sched: &mut Scheduler, base: (i64, i64)) -> Option<(usize, bool)> {
        self.tick = self.tick.wrapping_add(1);
        let at = ivec::gather(base.0, base.1, 0);
        let valid = sched.builder.leaf_valid(at);
        if let Some(k) = self.entries.iter().position(|e| e.base == base) {
            if self.entries[k].valid == valid {
                self.entries[k].used = self.tick;
                return Some((self.entries[k].volume, false));
            }
            self.remove(world, k);
        }
        let (volume, valid) = sched.builder.compose_leaf(world, at)?;
        let bands = world.volumes.band_count(volume);
        if bands > MAX_BANDS {
            return Some((volume, true));
        }
        self.entries.push(Entry {
            base,
            valid,
            volume,
            bands,
            used: self.tick,
        });
        while self.entries.len() > MAX_ENTRIES || self.entries.iter().map(|e| e.bands).sum::<usize>() > MAX_BANDS {
            let oldest = (0..self.entries.len()).min_by_key(|&k| self.entries[k].used).unwrap();
            if oldest == self.entries.len() - 1 {
                break;
            }
            self.remove(world, oldest);
        }
        Some((volume, false))
    }

    fn remove(&mut self, world: &mut World, k: usize) {
        let e = self.entries.swap_remove(k);
        world.volumes.release(e.volume);
    }

    /// Cell-DDA through one chunk at (chunk_ox, chunk_oy) of the ray's space,
    /// testing every run of each cell it steps into and stopping at the
    /// nearest hit.
    #[allow(clippy::too_many_arguments)]
    fn hit_chunk(&mut self, world: &mut World, volume: usize, o: &V3, d: &[f64; 3], chunk_ox: f64, chunk_oy: f64, max_dist: f64) -> Option<Hit> {
        // SAFETY: the composite is sealed and held until released here or kept.
        unsafe { world.volumes.band_views(volume, &mut self.bands) };
        let (bands, column) = (&self.bands, &mut self.column);
        let (ox, oy) = (o[0] - chunk_ox, o[1] - chunk_oy);
        let (dx, dy) = (d[0], d[1]);
        let span = CHUNK_CELLS as f64;

        let test_cell = |column: &mut ColumnRuns, cx: u32, cy: u32| -> Option<Hit> {
            let n = column.walk(bands, cx, cy);
            let mut best: Option<(Slab, u8, V3, V3)> = None;
            for i in 0..n {
                let (mut min, mut max) = run_box(cx, cy, column, i);
                min[0] += chunk_ox;
                min[1] += chunk_oy;
                max[0] += chunk_ox;
                max[1] += chunk_oy;
                let Some(slab) = ray_aabb(o, d, &min, &max) else { continue };
                if slab.t < 0.0 || best.as_ref().is_some_and(|b| slab.t >= b.0.t) {
                    continue;
                }
                best = Some((slab, column.material_class[i], min, max));
            }
            let (slab, material_class, box_min, box_max) = best?;
            if slab.t > max_dist {
                return None;
            }
            let mut normal = [0.0; 3];
            normal[slab.axis] = slab.sign;
            let t = slab.t;
            Some(Hit {
                material_class,
                normal,
                point: [o[0] + d[0] * t, o[1] + d[1] * t, o[2] + d[2] * t],
                t,
                box_min,
                box_max,
            })
        };

        if dx.abs() < 1e-9 && dy.abs() < 1e-9 {
            let cx = (ox).floor();
            let cy = (oy).floor();
            if cx < 0.0 || cx >= span || cy < 0.0 || cy >= span {
                return None;
            }
            return test_cell(column, cx as u32, cy as u32);
        }

        let mut walk = GridWalk::new(ox, oy, dx, dy, 1.0);
        let max_cells = (max_dist.min(CHUNK_W * 1.5)).ceil() as usize + 2;
        for _ in 0..max_cells {
            let (cell_x, cell_y) = (walk.x, walk.y);
            if cell_x >= 0.0 && cell_x < span && cell_y >= 0.0 && cell_y < span {
                if let Some(found) = test_cell(column, cell_x as u32, cell_y as u32) {
                    return Some(found);
                }
            }
            walk.advance();
        }
        None
    }
}

// ---- the decor ----

/// What a ray struck among the decor shown: the chunk it stands in, the
/// entry's band and packed position in that column, and how far along.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DecorHit {
    pub chunk: u32,
    pub band: u32,
    pub pos: u32,
    pub t: f64,
}

/// How far past a chunk's own square an instance standing in it can
/// reach, in units: a generous bound on the models' size, only for
/// skipping chunks the ray never comes near.
const DECOR_REACH: f64 = 16.0;

/// The nearest placed decor instance the ray strikes within `max_dist`,
/// over the decor every on-screen chunk shows, model triangle by triangle
/// after a bounding sphere. Procedural instances — the ones with no entry —
/// are passed over, as the eraser has nothing to remove there; hidden
/// ones are tested, since the hidden one is what the eraser is over.
pub fn hit_decor(world: &World, sched: &Scheduler, origin: [f64; 2], o: V3, d: [f64; 3], max_dist: f64) -> Option<DecorHit> {
    let mem = world.volumes.mem();
    let mut best: Option<DecorHit> = None;
    let mut limit = max_dist;
    for (id, base_x, base_y, decor) in sched.shown_decor() {
        let tx = base_x - origin[0];
        let ty = base_y - origin[1];
        if !ray_meets_square(&o, &d, tx - DECOR_REACH, ty - DECOR_REACH, tx + CHUNK_W + DECOR_REACH, ty + CHUNK_W + DECOR_REACH, limit) {
            continue;
        }
        // SAFETY: the block the scheduler wrote at `at`, of the groups it
        // recorded; every lane read is inside its group's run.
        let words = unsafe { core::slice::from_raw_parts(mem.add(decor.at as usize) as *const u32, decor.bytes as usize / 4) };
        for (g, (offset, count)) in crate::layer::decor::instance::decor_groups(words).enumerate() {
            let Some(Some(model)) = sched.decor_models.get(g) else { continue };
            let Some(mesh) = sched.models.mesh(*model) else { continue };
            let (offset, count) = (offset as usize, count as usize);
            let lane = |k: usize, i: usize| f32::from_bits(words[offset + k * count + i]) as f64;
            let band_at = offset + 5 * count + count.div_ceil(4);
            // SAFETY: the model's block, positions first and the indices
            // past the vertex words.
            let (positions, indices) = unsafe {
                (
                    core::slice::from_raw_parts(mem.add(mesh.at) as *const f32, mesh.vertex_cnt as usize * 3),
                    core::slice::from_raw_parts(
                        (mem.add(mesh.at) as *const u32).add(mesh.vertex_cnt as usize * pxl_render::frame::draw::MODEL_VERTEX_WORDS as usize),
                        mesh.index_cnt as usize,
                    ),
                )
            };
            for i in 0..count {
                let band = words[band_at + i];
                if band == crate::layer::decor::instance::NO_ENTRY {
                    continue;
                }
                let c = [tx + lane(0, i), ty + lane(1, i), lane(2, i)];
                let spin = lane(3, i);
                let s = lane(4, i) * mesh.scale as f64;
                let r = mesh.radius as f64 * s;
                let p = [o[0] - c[0], o[1] - c[1], o[2] - c[2]];
                if !ray_meets_sphere(&p, &d, r, limit) {
                    continue;
                }
                // Into the model's own frame: the spin undone, then the
                // quarter turn about X that stood it up, then the scale.
                let (sn, cs) = (-spin).sin_cos();
                let unturn = |v: [f64; 3]| [v[0] * cs - v[1] * sn, v[0] * sn + v[1] * cs, v[2]];
                let to_model = |v: [f64; 3]| {
                    let u = unturn(v);
                    [u[0] / s, u[2] / s, -u[1] / s]
                };
                let (mo, md) = (to_model(p), to_model(d));
                for tri in indices.as_chunks::<3>().0 {
                    let a = &positions[tri[0] as usize * 3..tri[0] as usize * 3 + 3];
                    let b = &positions[tri[1] as usize * 3..tri[1] as usize * 3 + 3];
                    let cc = &positions[tri[2] as usize * 3..tri[2] as usize * 3 + 3];
                    if let Some(t) = ray_triangle(&mo, &md, a, b, cc) {
                        if t < limit {
                            limit = t;
                            best = Some(DecorHit {
                                chunk: id,
                                band,
                                pos: words[band_at + count + i],
                                t,
                            });
                        }
                    }
                }
            }
        }
    }
    best
}

/// Whether the ray crosses the square [x0, x1] × [y0, y1] at any height
/// within `limit`.
fn ray_meets_square(o: &V3, d: &[f64; 3], x0: f64, y0: f64, x1: f64, y1: f64, limit: f64) -> bool {
    let mut t0 = 0.0f64;
    let mut t1 = limit;
    for (p, v, lo, hi) in [(o[0], d[0], x0, x1), (o[1], d[1], y0, y1)] {
        if v.abs() < 1e-12 {
            if p < lo || p > hi {
                return false;
            }
            continue;
        }
        let (mut a, mut b) = ((lo - p) / v, (hi - p) / v);
        if a > b {
            core::mem::swap(&mut a, &mut b);
        }
        t0 = t0.max(a);
        t1 = t1.min(b);
        if t0 > t1 {
            return false;
        }
    }
    true
}

/// Whether the ray from `p` (relative to the sphere's centre) comes within
/// `r` of it before `limit`.
fn ray_meets_sphere(p: &V3, d: &[f64; 3], r: f64, limit: f64) -> bool {
    let b = p[0] * d[0] + p[1] * d[1] + p[2] * d[2];
    let c = p[0] * p[0] + p[1] * p[1] + p[2] * p[2] - r * r;
    if c <= 0.0 {
        return true;
    }
    let disc = b * b - c;
    if disc < 0.0 {
        return false;
    }
    let t = -b - disc.sqrt();
    t >= 0.0 && t <= limit
}

/// Möller–Trumbore, both sides: the distance along `d` to the triangle.
fn ray_triangle(o: &[f64; 3], d: &[f64; 3], a: &[f32], b: &[f32], c: &[f32]) -> Option<f64> {
    let e1 = [b[0] as f64 - a[0] as f64, b[1] as f64 - a[1] as f64, b[2] as f64 - a[2] as f64];
    let e2 = [c[0] as f64 - a[0] as f64, c[1] as f64 - a[1] as f64, c[2] as f64 - a[2] as f64];
    let p = [d[1] * e2[2] - d[2] * e2[1], d[2] * e2[0] - d[0] * e2[2], d[0] * e2[1] - d[1] * e2[0]];
    let det = e1[0] * p[0] + e1[1] * p[1] + e1[2] * p[2];
    if det.abs() < 1e-12 {
        return None;
    }
    let inv = 1.0 / det;
    let s = [o[0] - a[0] as f64, o[1] - a[1] as f64, o[2] - a[2] as f64];
    let u = (s[0] * p[0] + s[1] * p[1] + s[2] * p[2]) * inv;
    if !(0.0..=1.0).contains(&u) {
        return None;
    }
    let q = [s[1] * e1[2] - s[2] * e1[1], s[2] * e1[0] - s[0] * e1[2], s[0] * e1[1] - s[1] * e1[0]];
    let v = (d[0] * q[0] + d[1] * q[1] + d[2] * q[2]) * inv;
    if v < 0.0 || u + v > 1.0 {
        return None;
    }
    let t = (e2[0] * q[0] + e2[1] * q[1] + e2[2] * q[2]) * inv;
    (t > 0.0).then_some(t)
}

// ---- what the modules call ----
//
// `ray_hit_world` asks the raycast above over the chunks the scheduler holds
// and the composites it keeps, and reads the hit back as doubles;
// `ray_hit_decor` asks `hit_decor` the same way.

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WorldHit {
    pub material_class: MaterialClass,
    pub face_normal: V3,
    pub hit_point: V3,
    pub t: f64,
    pub box_min: V3,
    pub box_max: V3,
}

/// A ray as an origin and a direction: what camera/ray.rs builds from the view.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RayLike {
    pub origin: Vec3,
    pub direction: Vec3,
}

/// The map origin as a ray query carries it, in units.
pub fn ray_origin(map_origin: IVec) -> [f64; 2] {
    [ivec::to_real(ivec::X(map_origin)), ivec::to_real(ivec::Y(map_origin))]
}

/// The nearest box the ray strikes within `max_dist`, or None: the kernel's
/// walk of every chunk the ray's XY projection crosses. The ray is in the
/// local world frame — relative to `map_origin` on X and Y — and so is what
/// comes back.
pub fn ray_hit_world(ray: &RayLike, map_origin: IVec, max_dist: f64) -> Option<WorldHit> {
    let k = kernel();
    let mut world = World {
        volumes: k.volumes.as_mut()?,
        materials: &k.terrain,
        store: &k.objects,
        painter: &mut k.painter,
    };
    let (o, d) = (ray.origin, ray.direction);
    let h = k.raycast.hit(&mut world, &mut k.sched, ray_origin(map_origin), [o.x, o.y, o.z], [d.x, d.y, d.z], max_dist)?;
    Some(WorldHit {
        material_class: h.material_class as MaterialClass,
        face_normal: h.normal,
        hit_point: h.point,
        t: h.t,
        box_min: h.box_min,
        box_max: h.box_max,
    })
}

/// The placed decor entry the ray strikes within `max_dist`, or None:
/// the kernel's hit test over every placed instance the on-screen chunks show.
pub fn ray_hit_decor(ray: &RayLike, map_origin: IVec, max_dist: f64) -> Option<DecorHit> {
    let k = kernel();
    let world = World {
        volumes: k.volumes.as_mut()?,
        materials: &k.terrain,
        store: &k.objects,
        painter: &mut k.painter,
    };
    let (o, d) = (ray.origin, ray.direction);
    hit_decor(&world, &k.sched, ray_origin(map_origin), [o.x, o.y, o.z], [d.x, d.y, d.z], max_dist)
}

#[cfg(test)]
mod decor_tests {
    use super::*;

    #[test]
    fn a_ray_down_the_z_axis_meets_the_triangle_under_it_at_its_height() {
        let t = ray_triangle(&[0.2, 0.2, 5.0], &[0.0, 0.0, -1.0], &[0.0, 0.0, 1.0], &[1.0, 0.0, 1.0], &[0.0, 1.0, 1.0]);
        assert!(t.is_some_and(|t| (t - 4.0).abs() < 1e-12), "{t:?}");
        // From the other side too, and not beside it.
        assert!(ray_triangle(&[0.2, 0.2, -5.0], &[0.0, 0.0, 1.0], &[0.0, 0.0, 1.0], &[1.0, 0.0, 1.0], &[0.0, 1.0, 1.0]).is_some());
        assert!(ray_triangle(&[0.8, 0.8, 5.0], &[0.0, 0.0, -1.0], &[0.0, 0.0, 1.0], &[1.0, 0.0, 1.0], &[0.0, 1.0, 1.0]).is_none());
    }

    #[test]
    fn a_square_is_met_only_where_the_ray_passes_over_it() {
        let d = [0.6, 0.6, -0.3];
        assert!(ray_meets_square(&[0.0, 0.0, 10.0], &d, 5.0, 5.0, 20.0, 20.0, 100.0));
        assert!(!ray_meets_square(&[0.0, 0.0, 10.0], &d, 5.0, 5.0, 20.0, 20.0, 3.0));
        assert!(!ray_meets_square(&[0.0, 0.0, 10.0], &d, -20.0, 5.0, -5.0, 20.0, 100.0));
    }

    #[test]
    fn a_sphere_is_met_within_reach_and_missed_past_it() {
        assert!(ray_meets_sphere(&[-10.0, 0.5, 0.0], &[1.0, 0.0, 0.0], 1.0, 100.0));
        assert!(!ray_meets_sphere(&[-10.0, 2.0, 0.0], &[1.0, 0.0, 0.0], 1.0, 100.0));
        assert!(!ray_meets_sphere(&[-10.0, 0.5, 0.0], &[1.0, 0.0, 0.0], 1.0, 5.0));
    }
}

#[cfg(test)]
#[path = "raycast.test.rs"]
mod test;
