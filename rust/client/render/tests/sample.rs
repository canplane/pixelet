//! The samples' run by hand (queue_walk.wgsl, queue_ask.wgsl, queue_last.wgsl over samples.wgsl),
//! the eye's and the light's (sample.eye.wgsl, sample.light.wgsl), answers as `cast::walk` from the root
//! answers the same ray: over the real world of tests/real_world, off what the beam's passes
//! leave there, each sample's stop as the Rust's, and the next frame off the node cache the first
//! left. The missed pixels' walks, the drawable entries the samples' stops set and the draw draws, and the boxes
//! the light's samples draw answer alike. Skipped where no adapter answers.

mod gpu;
mod real_world;

use std::collections::{BTreeMap, BTreeSet};

use gpu::{
    at_edge, bytes, cross, edge_axes, edges_within, entry_held, entry_node, lay_kept, list_every,
    margins, read_texture, texture, unit, Gpu, Kinds, LightBox,
};
use pxl_engine::answer::{self, Kind};
use pxl_engine::beam::{self, Grid};
use pxl_engine::cast::{self, Detail, End, Keep, Node, Ray, Spacing, Trace, Until, DEPTH};
use pxl_engine::light::{self, Light, Starts};
use pxl_engine::node_cache;
use pxl_engine::overwrite::Cut;
use pxl_engine::param;
use pxl_engine::world::Inheritance;
use pxl_engine::{count, eye, raster, ray, record, value, Zoom, FRAC_W, LEAF_CELL};
use pxl_render::frame::shader::record::{src, SHADER_CELL, SHADER_LIGHT_CELL};
use pxl_render::gpu::shadow;

use real_world::{q8, store_of, truth, Asking, Built, World, WorldBufs, FINEST};

/// East of X = 0 by the seam, and west of it.
const EYE: [i32; 2] = [(1 << 23) - 300, (40 << 16) + 30];
const EYE_WEST: [i32; 2] = [-(1 << 23) + 300, (40 << 16) + 30];
/// By X = 0, where X's roots part.
const EYE_ZERO: [i32; 2] = [150, (40 << 16) + 30];
const SIZE: u32 = 256;
const FRAME: u32 = 9;
const CAST: u32 = 7;
const WORKGROUP: u32 = beam::SAMPLE_SIDE;
/// Missed pixels apart from the reference tried from a start a step over, at most: what the
/// GPU's own way of an eye's ray (walk_node.wgsl `eye_aim`) takes across an edge.
const STEPS_TRIED: u32 = 32;
/// A sample not walked: its words stand.
const UNTOUCHED: u32 = 0xdead_beef;

/// An observer's samples as the test lays them: its rays and words, as eye.rs `Eye` and light.rs
/// `Map` give them, and the passes over it.
struct Observed {
    name: String,
    eye: Vec<u32>,
    head: Vec<u32>,
    words: Vec<u32>,
    lattice: [u32; 2],
    /// Whether its passes read the light's head (`Observer::apart`).
    apart: bool,
    /// The hit pass's groups.
    groups: [u32; 2],
    cuts: [Cut; 2],
    rays: Rays,
}

/// Where an observer's rays start and go: the eye's grid or the light's box.
enum Rays {
    Eye {
        origin: [i32; 3],
        axes: [[f32; 3]; 3],
        far: f32,
        range: [Zoom; 2],
        grid: Grid,
    },
    Light {
        light: Light,
        starts: Starts,
        range: [Zoom; 2],
        reads: Vec<bool>,
        /// The map's side, in texels.
        size: u32,
        /// The samples the threads take, where the cast does not begin its epoch.
        taken: Option<Vec<[u32; 2]>>,
    },
}

impl Observed {
    fn aim(&self, s: [u32; 2]) -> ([i32; 3], Ray, Detail) {
        match &self.rays {
            Rays::Eye {
                origin,
                axes,
                far,
                range,
                grid,
            } => {
                let at = grid.sample(s);
                let d = cast::through(axes[0], axes[1], axes[2], at[0], at[1]);
                (
                    *origin,
                    Ray::new(d, *far),
                    Detail {
                        range: *range,
                        spacing: Spacing {
                            angle: grid.s,
                            across: 0.0,
                        },
                    },
                )
            }
            Rays::Light {
                light: l,
                starts,
                range,
                size,
                ..
            } => {
                let d = param::light::SAMPLE;
                let p = l.texel(*size, [s[0] * d, s[1] * d]);
                let (o, r) = light::ray(l, starts, p);
                let tile = [(p[0] / light::TILE) as i32, (p[1] / light::TILE) as i32];
                let band = l.band(*size, *range, tile);
                (
                    o,
                    r,
                    Detail {
                        range: [band, band],
                        spacing: l.spacing(),
                    },
                )
            }
        }
    }

    /// The band sample `s`'s ray asked, the light's (`raster::key`); the eye's 0.
    fn band(&self, s: [u32; 2]) -> u32 {
        match &self.rays {
            Rays::Eye { .. } => 0,
            Rays::Light { .. } => self.aim(s).2.range[0],
        }
    }

    fn sample_at(&self, s: [u32; 2]) -> u32 {
        match &self.rays {
            Rays::Eye { grid, .. } => grid.sample_at(s),
            Rays::Light { size, .. } => light::sample(*size, s[1] * self.lattice[0] + s[0]),
        }
    }

    /// The samples the hit pass's threads take, each once.
    fn taken(&self) -> Vec<[u32; 2]> {
        if let Rays::Light {
            taken: Some(list), ..
        } = &self.rays
        {
            return list.clone();
        }
        let [a, d] = self.lattice;
        (0..d).flat_map(|j| (0..a).map(move |i| [i, j])).collect()
    }

    /// Whether the cast needs sample `s` (`Observer::needs`).
    fn needed(&self, s: [u32; 2]) -> bool {
        let Rays::Light {
            light: l,
            reads,
            size,
            ..
        } = &self.rays
        else {
            return true;
        };
        let d = param::light::SAMPLE;
        let p = l.texel(*size, [s[0] * d, s[1] * d]);
        let n = light::tiles(*size) as i32;
        let on = [(p[0] / light::TILE) as i32, (p[1] / light::TILE) as i32];
        (on[1] - 1..=on[1] + 1).any(|j| {
            (on[0] - 1..=on[0] + 1)
                .any(|i| i >= 0 && j >= 0 && i < n && j < n && reads[(j * n + i) as usize])
        })
    }

    /// Sample `s`'s walk as `cast::walk` from the root takes it: the reference's answer.
    fn walk(&self, w: World, s: [u32; 2]) -> Trace<Inheritance> {
        let (o, ray, detail) = self.aim(s);
        walk_from(w, self.cuts, o, &ray, detail)
    }
}

/// A ray's walk from `o` as `cast::walk` from the root takes it, the world asked of each node.
fn walk_from(
    w: World,
    cuts: [Cut; 2],
    o: [i32; 3],
    ray: &Ray,
    detail: Detail,
) -> Trace<Inheritance> {
    let mut q = Asking { w, cuts };
    cast::walk_from_root(&mut q, o, ray, detail, Until::Opaque, &mut ())
}

/// An eye over `built`: where it stands and looks, and the zooms it asks.
struct EyeCase {
    name: &'static str,
    origin: [i32; 3],
    look: [f32; 3],
    tan: [f32; 2],
    lattice: u32,
    far: f32,
    range: [Zoom; 2],
    cuts: [Cut; 2],
}

/// A light's box over `built`.
struct LightCase {
    name: &'static str,
    origin: [i32; 3],
    dir: [f32; 3],
    side: f32,
    back: f32,
    t_max: f32,
    range: [Zoom; 2],
    corner: [i32; 2],
    cuts: [Cut; 2],
    /// The map's side, in texels.
    size: u32,
    /// Which casts: the whole lattice, its epoch beginning, or tiles listed, one in so many
    /// (`light_beams`).
    listed: Option<u32>,
}

/// The eye's words at `c`, one step moved along each axis by `moved`.
fn eye_of(c: &EyeCase, moved: [i32; 3]) -> Observed {
    let forward = unit(c.look);
    let right = unit(cross(forward, [0.0, 0.0, 1.0]));
    let up = cross(right, forward);
    let origin = [
        c.origin[0].wrapping_add(moved[0]),
        c.origin[1].wrapping_add(moved[1]),
        c.origin[2].wrapping_add(moved[2]),
    ];
    let grid = Grid::of(c.tan, c.lattice);
    let mut words = vec![0u32; eye::WORDS];
    for k in 0..3 {
        words[eye::ORIGIN + k] = origin[k] as u32;
        words[eye::FORWARD + k] = forward[k].to_bits();
        words[eye::RIGHT + k] = right[k].to_bits();
        words[eye::UP + k] = up[k].to_bits();
    }
    words[eye::FAR] = c.far.to_bits();
    words[eye::TAN] = c.tan[0].to_bits();
    words[eye::TAN + 1] = c.tan[1].to_bits();
    words[eye::RANGE] = c.range[0];
    words[eye::RANGE + 1] = c.range[1];
    words[eye::GRID..eye::GRID + eye::GRID_WORDS].copy_from_slice(&grid.words());
    for (at, cut) in [(eye::CARVE, c.cuts[0]), (eye::HIDE, c.cuts[1])] {
        words[at] = cut.at[0] as u32;
        words[at + 1] = cut.at[1] as u32;
        words[at + 2] = cut.at[2] as u32;
        words[at + 3] = cut.zoom;
    }
    Observed {
        name: c.name.to_string(),
        eye: words,
        head: vec![0; light::HEAD_WORDS],
        words: vec![0; grid.size() as usize],
        lattice: grid.samples,
        apart: false,
        groups: [
            grid.samples[0].div_ceil(WORKGROUP),
            grid.samples[1].div_ceil(WORKGROUP),
        ],
        cuts: c.cuts,
        rays: Rays::Eye {
            origin,
            axes: [forward, right, up],
            far: c.far,
            range: c.range,
            grid,
        },
    }
}

/// The eye's beam passes over `obs`: the beam rays' run, then the cells' starts.
fn eye_beams(gpu: &Gpu, b: &WorldBufs, obs: &Observed, table: &wgpu::Buffer) -> Vec<u32> {
    let Rays::Eye { grid, .. } = &obs.rays else {
        unreachable!()
    };
    let eye = gpu.buffer(&obs.eye, wgpu::BufferUsages::UNIFORM);
    let head = gpu.buffer(&obs.head, wgpu::BufferUsages::UNIFORM);
    let beams = gpu.buffer(&obs.words, store_of());
    gpu.corners(false, &world_binds(b, &eye, &beams, table), table);
    gpu.dispatch(
        &gpu.pipeline(src(SHADER_CELL), "cell_cs"),
        &[(0, eye.as_entire_binding()), (8, beams.as_entire_binding())],
        [
            grid.span[0].div_ceil(WORKGROUP),
            grid.span[1].div_ceil(WORKGROUP),
            1,
        ],
    );
    let _ = head;
    gpu.read(&beams)
}

fn world_binds<'a>(
    b: &'a WorldBufs,
    eye: &'a wgpu::Buffer,
    beams: &'a wgpu::Buffer,
    table: &'a wgpu::Buffer,
) -> Vec<(u32, wgpu::BindingResource<'a>)> {
    vec![
        (0, eye.as_entire_binding()),
        (1, b.clip.as_entire_binding()),
        (2, b.ground.as_entire_binding()),
        (4, b.palette.as_entire_binding()),
        (5, b.shards.as_entire_binding()),
        (6, b.placed.as_entire_binding()),
        (7, b.pool.as_entire_binding()),
        (8, beams.as_entire_binding()),
        (9, table.as_entire_binding()),
    ]
}

/// A light box and where its rays start, its corner moved by `moved`.
fn box_of(c: &LightCase, moved: [i32; 2]) -> (Light, Starts, [i32; 2]) {
    let corner = [c.corner[0] + moved[0], c.corner[1] + moved[1]];
    let b = LightBox {
        size: c.size,
        origin: c.origin,
        dir: c.dir,
        side: c.side,
        back: c.back,
        t_max: c.t_max,
        corner,
        reach_ndc: 1.0,
    };
    let (l, starts) = b.lay();
    (l, starts, corner)
}

/// The light's words at `c`, its lines and cells yet to be laid: each cast tile a third of them.
fn light_of(c: &LightCase, moved: [i32; 2]) -> Observed {
    let (l, starts, _) = box_of(c, moved);
    let mut words = vec![0u32; eye::WORDS];
    words[eye::LIGHT..eye::LIGHT + eye::LIGHT_WORDS].copy_from_slice(&l.words());
    words[eye::RANGE] = c.range[0];
    words[eye::RANGE + 1] = c.range[1];
    for (at, cut) in [(eye::CARVE, c.cuts[0]), (eye::HIDE, c.cuts[1])] {
        words[at] = cut.at[0] as u32;
        words[at + 1] = cut.at[1] as u32;
        words[at + 2] = cut.at[2] as u32;
        words[at + 3] = cut.zoom;
    }
    let mut head = vec![0u32; light::HEAD_WORDS];
    head[light::HEAD_CAST] = CAST;
    head[light::HEAD_EPOCH] = CAST;
    for (k, v) in light::head_layout(c.size) {
        head[k] = v;
    }
    head[light::STARTS_AT * 4..(light::STARTS_AT + light::STARTS_VEC4S) * 4]
        .copy_from_slice(&starts.words());
    let n = c.size / param::light::SAMPLE;
    Observed {
        name: c.name.to_string(),
        eye: words,
        head,
        words: vec![0; light::list_words(c.size) as usize],
        lattice: [n, n],
        apart: true,
        groups: [n.div_ceil(WORKGROUP), n.div_ceil(WORKGROUP)],
        cuts: c.cuts,
        rays: Rays::Light {
            light: l,
            starts,
            range: c.range,
            reads: vec![true; (light::tiles(c.size) * light::tiles(c.size)) as usize],
            size: c.size,
            taken: None,
        },
    }
}

/// The light's beam rays and cells whole, then, where `listed`, the cast cut down to the tiles
/// one in that many fed (`Map::take`), the rest not read: some samples not taken, some not needed.
fn light_beams(
    gpu: &Gpu,
    b: &WorldBufs,
    obs: &mut Observed,
    table: &wgpu::Buffer,
    listed: Option<u32>,
) -> Vec<u32> {
    let Rays::Light { size, .. } = obs.rays else {
        unreachable!()
    };
    let eye = gpu.buffer(&obs.eye, wgpu::BufferUsages::UNIFORM);
    let head = gpu.buffer(&obs.head, wgpu::BufferUsages::UNIFORM);
    let tiles = light::tiles(size);
    let mut list = obs.words.clone();
    let reads_at = light::reads_at(size) as usize;
    list[reads_at..reads_at + (tiles * tiles) as usize].fill(CAST);
    // Every tile fed, as a cast that claims them all lists them; its samples' passes over the
    // list, a tile's samples a side each (`list_every`).
    let fed = list_every(&mut list, size, light::LISTED_FED, 1);
    obs.groups = list_every(
        &mut list,
        size,
        light::LISTED_FED,
        light::TILE / param::light::SAMPLE,
    );
    let beams = gpu.buffer(&list, store_of());
    let mut world = world_binds(b, &eye, &beams, table);
    world.push((11, head.as_entire_binding()));
    gpu.light_beam(&world, &beams, table, size);
    gpu.dispatch(
        &gpu.pipeline(src(SHADER_LIGHT_CELL), "light_cell_cs"),
        &[
            (0, eye.as_entire_binding()),
            (8, beams.as_entire_binding()),
            (11, head.as_entire_binding()),
        ],
        [fed[0], fed[1], 1],
    );
    let mut words = gpu.read(&beams);
    let Rays::Light {
        light: l,
        reads,
        taken,
        ..
    } = &mut obs.rays
    else {
        unreachable!()
    };
    if let Some(every) = listed {
        // Some tiles read, the box's corners among them; listed, the tiles that feed one, as
        // the propagation lists them (light_propagate.wgsl `enlist`), one in `every` of the
        // box and its edges.
        obs.head[light::HEAD_EPOCH] = CAST - 3;
        let side = light::TILE / param::light::SAMPLE;
        let at = light::listed_at(size, light::LISTED_FED) as usize;
        let read = |i: u32, j: u32| {
            (i % 5 == 1 && j % 4 == 2)
                || (i == 0 && (j == 0 || j == 13))
                || (i == tiles - 1 && j == tiles - 1)
        };
        let feeds = |i: u32, j: u32| {
            (i.saturating_sub(1)..=(i + 1).min(tiles - 1))
                .any(|a| (j.saturating_sub(1)..=(j + 1).min(tiles - 1)).any(|b| read(a, b)))
        };
        let mut entries = Vec::new();
        for j in 0..tiles {
            for i in 0..tiles {
                let some = (i + 2 * j).is_multiple_of(every) || i == 0 || j == tiles - 1;
                if some && feeds(i, j) {
                    entries.push(j << 16 | i);
                }
                reads[(j * tiles + i) as usize] = read(i, j);
                words[reads_at + (j * tiles + i) as usize] =
                    if read(i, j) { CAST } else { CAST - 1 };
            }
        }
        // In reverse, and the room past them an unlisted tile's: a read past the list, or a
        // place over, walks what none takes.
        entries.reverse();
        words[at] = entries.len() as u32;
        words[at + 1..at + 1 + entries.len()].copy_from_slice(&entries);
        let room = light::listed_room(size, light::LISTED_FED) as usize;
        if entries.len() < room {
            let unlisted = (0..tiles * tiles)
                .map(|k| ((k / tiles) << 16) | (k % tiles))
                .find(|e| !entries.contains(e))
                .expect("a tile unlisted");
            words[at + 1 + entries.len()..at + 1 + room].fill(unlisted);
        }
        let mut samples = Vec::new();
        let d = param::light::SAMPLE;
        for e in &entries {
            let t = [e & 0xffff, e >> 16];
            let slot = [0, 1].map(|k| {
                ((t[k] * light::TILE) as i32).wrapping_add(l.corner[k]) as u32 & (size - 1)
            });
            for y in 0..side {
                for x in 0..side {
                    samples.push([slot[0] / d + x, slot[1] / d + y]);
                }
            }
        }
        *taken = Some(samples);
        let threads = entries.len() as u32 * side * side;
        let rows = threads.div_ceil(light::LIST_ACROSS * light::LISTED_LANES);
        // Eight threads a side a workgroup, as the pass's own dispatch does.
        obs.groups = [light::LIST_ACROSS, rows];
    }
    words
}

/// One frame over `table0`: the samples' run, each sample's stop against `cast::walk`'s for its
/// ray, and no sample put that no thread takes. The table the passes left. The run's last walk,
/// which asks the world, over `asked`, where it is not the table the walks over the node cache
/// alone and their asks read.
#[expect(
    clippy::too_many_arguments,
    reason = "a frame's device, world, observer, words, tables, what it tells and its passes"
)]
fn run_frame(
    gpu: &Gpu,
    b: &WorldBufs,
    w: World,
    obs: &Observed,
    words: &[u32],
    table0: &[u32],
    asked: Option<&[u32]>,
    what: &str,
    kinds: &mut Kinds,
    failed: &mut Vec<String>,
) -> Vec<u32> {
    let name = format!("{} ({what})", obs.name);
    let eye_buf = gpu.buffer(&obs.eye, wgpu::BufferUsages::UNIFORM);
    let head_buf = gpu.buffer(&obs.head, wgpu::BufferUsages::UNIFORM);
    let taken = obs.taken();

    let mut laid = words.to_vec();
    for j in 0..obs.lattice[1] {
        for i in 0..obs.lattice[0] {
            let at = obs.sample_at([i, j]) as usize;
            laid[at..at + beam::SAMPLE_WORDS as usize].fill(UNTOUCHED);
        }
    }
    let beams = gpu.buffer(&laid, store_of());
    let table = gpu.buffer(table0, store_of());
    let asked = asked.map(|t| gpu.buffer(t, store_of()));
    let mut binds = world_binds(b, &eye_buf, &beams, &table);
    if obs.apart {
        binds.push((11, head_buf.as_entire_binding()));
    }
    gpu.samples_asked(obs.apart, &binds, &table, asked.as_ref());
    let table = asked.unwrap_or(table);
    let out = gpu.read(&beams);

    // Each sample's stop: a needed one's as its ray's walk answers, one not needed none.
    let (mut same, mut differ, mut axis_over, mut edges) = (0u64, 0u64, 0u64, 0u64);
    let mut first = None;
    for &s in &taken {
        let at = obs.sample_at(s) as usize;
        let node = at + beam::SAMPLE_NODE as usize;
        let zoom_word = node + beam::NODE_ZOOM as usize;
        if !obs.needed(s) {
            if out[zoom_word] != beam::SAMPLE_NONE {
                differ += 1;
                first.get_or_insert((s, out[at..at + 12].to_vec()));
            }
            continue;
        }
        let met = obs.walk(w, s);
        let want_zoom = if met.ends == End::Miss {
            beam::SAMPLE_NONE
        } else {
            met.hit.node.zoom
        };
        let mut ok = out[zoom_word] == want_zoom;
        if met.ends != End::Miss {
            ok =
                ok && (0..3).all(|a| {
                    out[node + beam::NODE_CORNER as usize + a] == met.hit.node.corner[a]
                        && out[node + beam::NODE_ROOTS as usize + a] == met.hit.node.roots[a] as u32
                }) && out[at + beam::SAMPLE_ASSET as usize] == met.hit.asset[0]
                    && out[at + beam::SAMPLE_ASSET as usize + 1] == met.hit.asset[1];
            if ok && out[at + beam::SAMPLE_AXIS as usize] != met.hit.axis {
                // Entering at an edge, the axis another as a GPU rounds its planes.
                axis_over += 1;
            }
        } else {
            ok = ok
                && out[at + beam::SAMPLE_ASSET as usize] == 0
                && out[at + beam::SAMPLE_ASSET as usize + 1] == 0
                && out[at + beam::SAMPLE_AXIS as usize] == 0;
        }
        ok = ok && out[at + beam::SAMPLE_LACKS as usize] == u32::from(met.ends == End::Lacks);
        // A ray the device's rounding takes across an edge stops either side of it: the cell
        // the walk stopped at, or the one the device stopped at, entered at an edge
        // (`at_edge`, as tests/cast.rs takes a pixel at an edge).
        if !ok {
            let (o, ray, _) = obs.aim(s);
            let edge = |n: &Node, asset: u32| {
                at_edge(&cast::entry_with(o, &ray, ray.inverse(), n, asset), ray.dir)
            };
            let theirs = out[zoom_word] != beam::SAMPLE_NONE && {
                let word = |k: u32, a: usize| out[node + k as usize + a];
                let n = Node {
                    corner: [0, 1, 2].map(|a| word(beam::NODE_CORNER, a)),
                    zoom: out[zoom_word],
                    roots: [0, 1, 2].map(|a| word(beam::NODE_ROOTS, a) as i32),
                };
                edge(&n, out[at + beam::SAMPLE_ASSET as usize])
            };
            if theirs || (met.ends == End::Hit && edge(&met.hit.node, met.hit.asset[0])) {
                edges += 1;
                ok = true;
            }
        }
        if ok {
            same += 1;
        } else {
            differ += 1;
            if first.is_none() {
                eprintln!(
                    "DIFFERS {name} {s:?}: want {:?} z{} {:?} {:?} t{} axis {} asset {:x?}; got {:x?}",
                    met.ends, met.hit.node.zoom, met.hit.node.corner, met.hit.node.roots, met.hit.t, met.hit.axis, met.hit.asset, &out[at..at + beam::SAMPLE_WORDS as usize]
                );
            }
            first.get_or_insert((s, out[at..at + 12].to_vec()));
        }
        let hits = met.ends == End::Hit;
        let a = met.hit.asset[0];
        kinds.add("hit", hits);
        kinds.add("past its reach", met.ends == End::Miss);
        kinds.add("lacks", met.ends == End::Lacks);
        kinds.add("hit, placed", hits && answer::is_placed(a));
        kinds.add(
            "hit, short of its cube",
            hits && answer::fill_eighths(a) < 8,
        );
        kinds.add("hit, a way's", hits && value::transports(answer::val(a)));
        kinds.add("ended in another root", met.hit.node.roots != [0; 3]);
    }
    // No other sample put.
    let wanted: BTreeSet<[u32; 2]> = taken.iter().copied().collect();
    let mut stray = 0u64;
    for j in 0..obs.lattice[1] {
        for i in 0..obs.lattice[0] {
            if !wanted.contains(&[i, j]) {
                let at = obs.sample_at([i, j]) as usize;
                stray += u64::from(
                    out[at..at + beam::SAMPLE_WORDS as usize]
                        .iter()
                        .any(|&v| v != UNTOUCHED),
                );
            }
        }
    }
    eprintln!(
        "{name}: {same} alike ({axis_over} an axis over, {edges} at an edge), {differ} differ of {} samples, {stray} stray",
        taken.len()
    );
    let mut fail = |what: String| failed.push(format!("{name}: {what}"));
    if differ != 0 {
        fail(format!("{differ} samples differ, first {first:?}"));
    }
    if same == 0 {
        fail("no sample compared".to_string());
    }
    if stray != 0 {
        fail(format!("{stray} samples put that no thread takes"));
    }
    if !edges_within(edges, same) {
        fail(format!("too many at an edge: {edges}"));
    }
    if axis_over > 256 {
        fail(format!("too many an axis over: {axis_over}"));
    }
    gpu.read(&table)
}

/// An observer's two frames, the second a step moved, off the node cache the first left.
#[expect(
    clippy::too_many_arguments,
    reason = "the device, the world, two frames' observers, their lines, what to tell and what failed"
)]
fn two_frames(
    gpu: &Gpu,
    b: &WorldBufs,
    w: World,
    first: &mut Observed,
    next: &mut Observed,
    lines: &mut dyn FnMut(&mut Observed, &wgpu::Buffer) -> Vec<u32>,
    kinds: &mut Kinds,
    failed: &mut Vec<String>,
) {
    let table_buf = gpu.buffer(&frame_table(), store_of());
    let words = lines(first, &table_buf);
    let after_beam = gpu.read(&table_buf);
    let mut table1 = run_frame(
        gpu,
        b,
        w,
        first,
        &words,
        &after_beam,
        None,
        "first frame",
        kinds,
        failed,
    );
    table1[node_cache::FRAME as usize] = FRAME + 1;
    let table_buf = gpu.buffer(&table1, store_of());
    let words1 = lines(next, &table_buf);
    let after_beam1 = gpu.read(&table_buf);
    run_frame(
        gpu,
        b,
        w,
        next,
        &words1,
        &after_beam1,
        None,
        "next frame",
        kinds,
        failed,
    );
}

/// What a case set must meet among its rays, so its answers say something of each.
fn met_each(kinds: &Kinds, what: &[&str], failed: &mut Vec<String>) {
    eprintln!("  KINDS {kinds:?}");
    for k in what {
        if kinds.get(k) == 0 {
            failed.push(format!("none of {k} to say"));
        }
    }
}

fn world_bufs(gpu: &Gpu, built: &Built) -> WorldBufs {
    let store = store_of();
    let flat_u = |rows: &[[u32; 4]]| rows.iter().flatten().copied().collect::<Vec<u32>>();
    let clip: Vec<u32> = built.clipmap.iter().flatten().map(|&v| v as u32).collect();
    WorldBufs {
        clip: gpu.buffer(&clip, store),
        ground: gpu.buffer(&built.ground, store),
        palette: gpu.buffer(&built.palette, store),
        shards: gpu.buffer(&flat_u(&built.shards), store),
        placed: gpu.buffer(&built.overwrites, store),
        pool: gpu.buffer(&built.pool, store),
    }
}

fn device() -> Option<Gpu> {
    // A hand shader's loop may hang the device's wait: the run gives up.
    std::thread::spawn(|| {
        std::thread::sleep(std::time::Duration::from_secs(1200));
        eprintln!("gave up after 20 minutes");
        std::process::exit(124);
    });
    let gpu = Gpu::new();
    if gpu.is_none() {
        eprintln!("no adapter: skipped");
    }
    gpu
}

/// The world's spots the cases stand on: over the ground, on a bridge, down a bore, on what is
/// placed.
struct Spots {
    bridge: [i32; 3],
    bore: [i32; 3],
    on_placed: [i32; 2],
    leaf_zoom: Zoom,
    leaf_c: [i32; 3],
}

fn spots(b: &Built) -> Spots {
    let none = [Cut::NONE; 2];
    let way = |lines: [u32; 2]| {
        let of_way = |p: [i32; 3]| {
            let c = [real_world::wrap(p[0]), p[1], p[2]];
            let zooms = [LEAF_CELL, LEAF_CELL];
            value::transports(answer::val(
                truth(b.world(), none, LEAF_CELL, c, zooms).0.asset[0],
            ))
        };
        b.way_points
            .iter()
            .filter(|p| p.0 == LEAF_CELL && lines.contains(&p.1) && p.2[0] > 0)
            .flat_map(|p| (-16..4).map(|d| [p.2[0], p.2[1], p.2[2] + d]))
            .find(|&p| of_way(p))
            .expect("a way the world answers")
    };
    let (leaf_zoom, leaf_c) = b
        .placed
        .iter()
        .copied()
        .find(|p| p.0 == 25)
        .expect("something placed finer than a cell");
    Spots {
        bridge: way([0, 3]),
        bore: way([1, 4]),
        on_placed: [leaf_c[0] >> 1, leaf_c[1] >> 1],
        leaf_zoom,
        leaf_c,
    }
}

/// A first word no walk writes, a marked pixel's before its walk ends it (`ray::first_word`
/// sets no bit past the fill share's, and a ray that met nothing writes 0).
const UNWALKED: u32 = u32::MAX;

#[test]
fn hand_eye_samples_answer_as_cast_walk() {
    let Some(gpu) = device() else { return };
    let built = real_world::build(EYE);
    let b = world_bufs(&gpu, &built);
    let w = built.world();
    let at = spots(&built);
    let none = [Cut::NONE; 2];
    let over = |g: [i32; 2], up: i32| q8([g[0], g[1], built.top_at(g) + up]);
    let cases = vec![
        EyeCase {
            name: "the eye, over the ground",
            origin: over([EYE[0] + 90, EYE[1] + 90], 60),
            look: [0.5, 0.4, -0.77],
            tan: [0.3, 0.2],
            lattice: 1,
            far: 2000.0,
            range: [8, FINEST],
            cuts: none,
        },
        EyeCase {
            name: "the eye, lattice 2, over the seam",
            origin: over([EYE[0] + 280, EYE[1] + 90], 40),
            look: [0.8, 0.2, -0.55],
            tan: [0.4, 0.3],
            lattice: 2,
            far: 2000.0,
            range: [8, FINEST],
            cuts: none,
        },
        EyeCase {
            name: "the eye, on what is placed, cut and hid",
            origin: over(at.on_placed, 20),
            look: [0.02, 0.03, -1.0],
            tan: [0.3, 0.4],
            lattice: 1,
            far: 400.0,
            range: [LEAF_CELL, FINEST],
            cuts: [
                Cut {
                    zoom: 22,
                    at: [at.leaf_c[0] >> 3, at.leaf_c[1] >> 3, at.leaf_c[2] >> 3],
                },
                Cut {
                    zoom: at.leaf_zoom,
                    at: at.leaf_c,
                },
            ],
        },
        EyeCase {
            name: "the eye, on a bridge",
            origin: q8([at.bridge[0], at.bridge[1], at.bridge[2] + 12]),
            look: [0.5, -0.3, -0.8],
            tan: [0.3, 0.3],
            lattice: 1,
            far: 600.0,
            range: [LEAF_CELL, FINEST],
            cuts: none,
        },
        EyeCase {
            name: "the eye, down a bore, under the ground",
            origin: q8([at.bore[0], at.bore[1], at.bore[2]]),
            look: [0.6, 0.2, -0.3],
            tan: [0.3, 0.3],
            lattice: 1,
            far: 300.0,
            range: [LEAF_CELL, FINEST],
            cuts: none,
        },
        EyeCase {
            name: "the eye, where no clip level holds the ground",
            origin: q8([1 << 15, 1 << 15, 40]),
            look: [0.3, 0.2, -0.93],
            tan: [0.4, 0.3],
            lattice: 1,
            far: 800.0,
            range: [8, FINEST],
            cuts: none,
        },
        EyeCase {
            name: "the eye, high over the clip levels",
            origin: over([EYE[0] + 2000, EYE[1] + 2000], 2500),
            look: [0.3, -0.2, -0.9],
            tan: [0.3, 0.25],
            lattice: 1,
            far: 20000.0,
            range: [4, FINEST],
            cuts: none,
        },
    ];
    let mut kinds = Kinds::new();
    let mut failed = Vec::new();
    let step = 40 << FRAC_W;
    for c in &cases {
        let mut first = eye_of(c, [0; 3]);
        let mut next = eye_of(c, [step, step / 2, 9 << FRAC_W]);
        two_frames(
            &gpu,
            &b,
            w,
            &mut first,
            &mut next,
            &mut |o, table| eye_beams(&gpu, &b, o, table),
            &mut kinds,
            &mut failed,
        );
    }
    // West of X = 0.
    let west = real_world::build(EYE_WEST);
    let bw = world_bufs(&gpu, &west);
    let over_w = |g: [i32; 2], up: i32| q8([g[0], g[1], west.top_at(g) + up]);
    let c = EyeCase {
        name: "the eye, west of X = 0",
        origin: over_w([EYE_WEST[0] + 90, EYE_WEST[1] + 90], 60),
        look: [-0.5, 0.4, -0.77],
        tan: [0.3, 0.2],
        lattice: 1,
        far: 2000.0,
        range: [8, FINEST],
        cuts: none,
    };
    let mut first = eye_of(&c, [0; 3]);
    let mut next = eye_of(&c, [-step, step / 2, 9 << FRAC_W]);
    two_frames(
        &gpu,
        &bw,
        west.world(),
        &mut first,
        &mut next,
        &mut |o, table| eye_beams(&gpu, &bw, o, table),
        &mut kinds,
        &mut failed,
    );
    // Over the seam, where the world's X wraps.
    let c = EyeCase {
        name: "the eye, looking over the seam",
        origin: over_w([EYE_WEST[0] - 280, EYE_WEST[1] + 90], 40),
        look: [-0.9, 0.1, -0.3],
        tan: [0.3, 0.2],
        lattice: 1,
        far: 2000.0,
        range: [8, FINEST],
        cuts: none,
    };
    let mut first = eye_of(&c, [0; 3]);
    let mut next = eye_of(&c, [-step, step / 2, 9 << FRAC_W]);
    two_frames(
        &gpu,
        &bw,
        west.world(),
        &mut first,
        &mut next,
        &mut |o, table| eye_beams(&gpu, &bw, o, table),
        &mut kinds,
        &mut failed,
    );
    // Over X = 0, where its roots part: its rays end in the next root.
    let zero = real_world::build(EYE_ZERO);
    let bz = world_bufs(&gpu, &zero);
    let over_z = |g: [i32; 2], up: i32| q8([g[0], g[1], zero.top_at(g) + up]);
    let c = EyeCase {
        name: "the eye, looking over X = 0",
        origin: over_z([20, EYE_ZERO[1] + 90], 40),
        look: [-0.9, 0.1, -0.3],
        tan: [0.3, 0.2],
        lattice: 1,
        far: 2000.0,
        range: [8, FINEST],
        cuts: none,
    };
    let mut first = eye_of(&c, [0; 3]);
    let mut next = eye_of(&c, [-step, step / 2, 9 << FRAC_W]);
    two_frames(
        &gpu,
        &bz,
        zero.world(),
        &mut first,
        &mut next,
        &mut |o, table| eye_beams(&gpu, &bz, o, table),
        &mut kinds,
        &mut failed,
    );
    met_each(
        &kinds,
        &[
            "hit",
            "past its reach",
            "lacks",
            "hit, placed",
            "hit, short of its cube",
            "hit, a way's",
        ],
        &mut failed,
    );
    assert!(failed.is_empty(), "{}", failed.join("\n"));
}

/// A sample whose cell's window holds a beam ray the budget parked waits on it: the samples'
/// run parks it before its first step (samples.wgsl `item_waits`, pxl-engine
/// `beam::HAND_PARKED`) and walks none of it, every other sample answering as it does where no
/// cell waits; and the next cast's run, the beam ray ended and its parked samples going on
/// (`node_cache::ROLL_RESUMES`), answers every sample as that whole run does.
#[test]
fn eye_samples_wait_on_a_cell_whose_beam_ray_has_not_ended() {
    let Some(gpu) = device() else { return };
    let built = real_world::build(EYE);
    let b = world_bufs(&gpu, &built);
    let over = |g: [i32; 2], up: i32| q8([g[0], g[1], built.top_at(g) + up]);
    let c = EyeCase {
        name: "the eye, over the ground",
        origin: over([EYE[0] + 90, EYE[1] + 90], 60),
        look: [0.5, 0.4, -0.77],
        tan: [0.3, 0.2],
        lattice: 1,
        far: 2000.0,
        range: [8, FINEST],
        cuts: [Cut::NONE; 2],
    };
    let obs = eye_of(&c, [0; 3]);
    let table_buf = gpu.buffer(&frame_table(), store_of());
    let words = eye_beams(&gpu, &b, &obs, &table_buf);
    let table = gpu.read(&table_buf);
    let Rays::Eye { grid, .. } = &obs.rays else {
        unreachable!()
    };
    // Every third cell held over a beam ray not ended.
    let waits = |cell: [u32; 2]| (cell[0] + cell[1]).is_multiple_of(3);
    let mut held = words.clone();
    for y in 0..grid.span[1] {
        for x in 0..grid.span[0] {
            if waits([x, y]) {
                held[(grid.cell_at([x, y]) + beam::CELL_RESUMES) as usize] = beam::CELL_WAITS;
            }
        }
    }
    let whole = stopped(&gpu, &b, &obs, &words, None, &table);
    let first = stopped(&gpu, &b, &obs, &held, None, &table);
    let mut resumed = table.clone();
    resumed[node_cache::ROLL_RESUMES as usize] = 1;
    let next = stopped(&gpu, &b, &obs, &words, Some(&first), &resumed);
    let (mut waited, mut walked, mut differ) = (0u32, 0u32, Vec::new());
    for s in obs.taken() {
        let at = obs.sample_at(s) as usize;
        let of = |v: &[u32]| v[at..at + beam::SAMPLE_WORDS as usize].to_vec();
        if waits(grid.sample_cell(s)) {
            let zoom = first[at + (beam::SAMPLE_NODE + beam::NODE_ZOOM) as usize];
            let state = first[at + beam::SAMPLE_PARKED_STATE as usize];
            if zoom != beam::SAMPLE_PARKED || state & 63 != 63 {
                differ.push(format!("{s:?} did not wait: {:x?}", of(&first)));
            }
            waited += 1;
        } else {
            if of(&first) != of(&whole) {
                differ.push(format!("{s:?} answered apart from the whole run"));
            }
            walked += 1;
        }
        if of(&next) != of(&whole) {
            differ.push(format!(
                "{s:?} went on apart from the whole run: {:x?}",
                of(&next)
            ));
        }
    }
    eprintln!("{waited} samples waited, {walked} walked");
    assert!(
        waited > 100 && walked > 100,
        "{waited} waited, {walked} walked"
    );
    assert!(
        differ.is_empty(),
        "{} differ, first {}",
        differ.len(),
        differ[0]
    );
}

/// A walk that asks reads a slot as another walk may be writing it, its key no order over its
/// words (pxl-engine node_cache.rs): an entry holding words of two writes answers nothing. Every
/// entry a first frame kept has one word of its answer changed, its key and its node standing, and
/// the next frame's samples, all left to their asking walk off a table that kept nothing, answer
/// as `cast::walk` still.
#[test]
fn entry_of_two_writes_answers_nothing() {
    let Some(gpu) = device() else { return };
    let built = real_world::build(EYE);
    let b = world_bufs(&gpu, &built);
    let w = built.world();
    let over = |g: [i32; 2], up: i32| q8([g[0], g[1], built.top_at(g) + up]);
    let c = EyeCase {
        name: "the eye, its entries written twice",
        origin: over([EYE[0] + 90, EYE[1] + 90], 60),
        look: [0.5, 0.4, -0.77],
        tan: [0.3, 0.2],
        lattice: 1,
        far: 2000.0,
        range: [8, FINEST],
        cuts: [Cut::NONE; 2],
    };
    let (mut kinds, mut failed) = (Kinds::new(), Vec::new());
    let obs = eye_of(&c, [0; 3]);
    let empty = frame_table();
    let table = gpu.buffer(&empty, store_of());
    let words = eye_beams(&gpu, &b, &obs, &table);
    let first = gpu.read(&table);
    let mut torn = run_frame(
        &gpu,
        &b,
        w,
        &obs,
        &words,
        &first,
        None,
        "first frame",
        &mut kinds,
        &mut failed,
    );
    torn[node_cache::FRAME as usize] = FRAME + 1;
    let answer_words = node_cache::W_NODE - node_cache::W_KIND;
    let mut changed = 0u32;
    for e in 0..node_cache::ENTRIES {
        let slot = node_cache::ENTRIES_AT + e * node_cache::ENTRY_WORDS;
        if torn[(slot + node_cache::W_KEY) as usize] != 0 {
            let at = slot + node_cache::W_KIND + e % answer_words;
            torn[at as usize] ^= 1 << 8;
            changed += 1;
        }
    }
    assert!(changed > 1000, "{changed} entries kept to change");
    let mut next_empty = empty;
    next_empty[node_cache::FRAME as usize] = FRAME + 1;
    let table = gpu.buffer(&next_empty, store_of());
    let words = eye_beams(&gpu, &b, &obs, &table);
    let next_first = gpu.read(&table);
    run_frame(
        &gpu,
        &b,
        w,
        &obs,
        &words,
        &next_first,
        Some(&torn),
        "next frame",
        &mut kinds,
        &mut failed,
    );
    assert!(failed.is_empty(), "{}", failed.join("\n"));
}

#[test]
fn hand_light_samples_answer_as_cast_walk() {
    let Some(gpu) = device() else { return };
    let built = real_world::build(EYE);
    let b = world_bufs(&gpu, &built);
    let w = built.world();
    let at = spots(&built);
    let none = [Cut::NONE; 2];
    let over = |g: [i32; 2], up: i32| q8([g[0], g[1], built.top_at(g) + up]);
    let cases = vec![
        LightCase {
            name: "the light, about the eye",
            origin: over([EYE[0] + 90, EYE[1] + 90], 40),
            dir: [0.3, 0.2, -0.93],
            side: 1.0,
            back: 60.0,
            t_max: 400.0,
            range: [8, FINEST],
            corner: [1304, -896],
            cuts: none,
            size: SIZE,
            listed: None,
        },
        LightCase {
            name: "the light, its tiles listed",
            origin: over([EYE[0] + 90, EYE[1] + 90], 40),
            dir: [0.3, 0.2, -0.93],
            side: 1.0,
            back: 60.0,
            t_max: 400.0,
            range: [8, FINEST],
            corner: [1304, -896],
            cuts: none,
            size: SIZE,
            listed: Some(3),
        },
        LightCase {
            name: "the light, every tile listed, the list's row full",
            origin: over([EYE[0] + 90, EYE[1] + 90], 40),
            dir: [0.3, 0.2, -0.93],
            side: 1.0,
            back: 60.0,
            t_max: 400.0,
            range: [8, FINEST],
            corner: [1304, -896],
            cuts: none,
            size: SIZE,
            listed: Some(1),
        },
        LightCase {
            name: "the light, its tiles listed, the shipped map's size",
            origin: over([EYE[0] + 90, EYE[1] + 90], 40),
            dir: [0.3, 0.2, -0.93],
            side: 0.5,
            back: 60.0,
            t_max: 400.0,
            range: [8, FINEST],
            corner: [1304, -896],
            cuts: none,
            size: shadow::SIZE,
            listed: Some(23),
        },
        LightCase {
            name: "the light, one band everywhere",
            origin: over([EYE[0] + 90, EYE[1] + 90], 40),
            dir: [0.3, 0.2, -0.93],
            side: 1.0,
            back: 60.0,
            t_max: 400.0,
            range: [LEAF_CELL, LEAF_CELL],
            corner: [1304, -896],
            cuts: none,
            size: SIZE,
            listed: None,
        },
        LightCase {
            name: "the light, where no clip level holds the ground",
            origin: q8([1 << 15, 1 << 15, 40]),
            dir: [0.3, 0.2, -0.93],
            side: 8.0,
            back: 100.0,
            t_max: 800.0,
            range: [8, FINEST],
            corner: [0, 0],
            cuts: none,
            size: SIZE,
            listed: None,
        },
        LightCase {
            name: "the light, wide, over the clip levels",
            origin: over([EYE[0] + 3000, EYE[1] + 3000], 900),
            dir: [0.2, -0.3, -0.93],
            side: 32.0,
            back: 200.0,
            t_max: 4000.0,
            range: [8, FINEST],
            corner: [80, 8],
            cuts: none,
            size: SIZE,
            listed: None,
        },
        LightCase {
            name: "the light, on what is placed, cut and hid",
            origin: over(at.on_placed, 30),
            dir: [0.1, 0.25, -0.96],
            side: 0.25,
            back: 10.0,
            t_max: 120.0,
            range: [LEAF_CELL, FINEST],
            corner: [8, 16],
            cuts: [
                Cut {
                    zoom: 22,
                    at: [at.leaf_c[0] >> 3, at.leaf_c[1] >> 3, at.leaf_c[2] >> 3],
                },
                Cut {
                    zoom: at.leaf_zoom,
                    at: at.leaf_c,
                },
            ],
            size: SIZE,
            listed: None,
        },
        LightCase {
            name: "the light, on a bridge, along an axis",
            origin: q8([at.bridge[0], at.bridge[1], at.bridge[2] + 30]),
            dir: [0.0, 0.0, -1.0],
            side: 0.5,
            back: 10.0,
            t_max: 200.0,
            range: [LEAF_CELL, FINEST],
            corner: [-16, 32],
            cuts: none,
            size: SIZE,
            listed: None,
        },
        LightCase {
            name: "the light, on a bridge, listed, the box wrapping",
            origin: q8([at.bridge[0], at.bridge[1], at.bridge[2] + 30]),
            dir: [0.3, -0.1, -0.95],
            side: 0.5,
            back: 10.0,
            t_max: 200.0,
            range: [LEAF_CELL, FINEST],
            corner: [-16, 32],
            cuts: none,
            size: SIZE,
            listed: Some(3),
        },
    ];
    let mut kinds = Kinds::new();
    let mut failed = Vec::new();
    for c in &cases {
        let mut first = light_of(c, [0, 0]);
        let mut next = light_of(c, [0, 0]);
        two_frames(
            &gpu,
            &b,
            w,
            &mut first,
            &mut next,
            &mut |o, table| light_beams(&gpu, &b, o, table, c.listed),
            &mut kinds,
            &mut failed,
        );
    }
    // A box over the seam, where the world's X wraps, one band so its cells resume.
    let west = real_world::build(EYE_WEST);
    let bw = world_bufs(&gpu, &west);
    let c = LightCase {
        name: "the light, over the seam",
        origin: q8([
            EYE_WEST[0] - 300,
            EYE_WEST[1] + 90,
            west.top_at([EYE_WEST[0] + 20, EYE_WEST[1] + 90]) + 40,
        ]),
        dir: [0.3, 0.2, -0.93],
        side: 1.0,
        back: 60.0,
        t_max: 400.0,
        range: [LEAF_CELL, LEAF_CELL],
        corner: [1304, -896],
        cuts: none,
        size: SIZE,
        listed: None,
    };
    let mut first = light_of(&c, [0, 0]);
    let mut next = light_of(&c, [0, 0]);
    two_frames(
        &gpu,
        &bw,
        west.world(),
        &mut first,
        &mut next,
        &mut |o, table| light_beams(&gpu, &bw, o, table, c.listed),
        &mut kinds,
        &mut failed,
    );
    // A box over X = 0, where its roots part, one band so its cells resume: a line's start and
    // a ray's lie in different roots.
    let zero = real_world::build(EYE_ZERO);
    let bz = world_bufs(&gpu, &zero);
    let c = LightCase {
        name: "the light, over X = 0",
        origin: q8([
            10,
            EYE_ZERO[1] + 90,
            zero.top_at([10, EYE_ZERO[1] + 90]) + 40,
        ]),
        dir: [0.3, 0.2, -0.93],
        side: 1.0,
        back: 60.0,
        t_max: 400.0,
        range: [LEAF_CELL, LEAF_CELL],
        corner: [1304, -896],
        cuts: none,
        size: SIZE,
        listed: None,
    };
    let mut first = light_of(&c, [0, 0]);
    let mut next = light_of(&c, [0, 0]);
    two_frames(
        &gpu,
        &bz,
        zero.world(),
        &mut first,
        &mut next,
        &mut |o, table| light_beams(&gpu, &bz, o, table, c.listed),
        &mut kinds,
        &mut failed,
    );
    // Slanting east across X = 0, narrow: a ray resumes off a line from the root over, the
    // node the line holds held from the ray's origin a root apart (`moved_roots`). Wider, every
    // such cell starts at 0, a beam ray stopped at X = 0 within its margin.
    let c = LightCase {
        name: "the light, slanting across X = 0",
        dir: [0.9, 0.05, -0.44],
        side: 0.25,
        back: 10.0,
        ..c
    };
    let mut first = light_of(&c, [0, 0]);
    let mut next = light_of(&c, [0, 0]);
    two_frames(
        &gpu,
        &bz,
        zero.world(),
        &mut first,
        &mut next,
        &mut |o, table| light_beams(&gpu, &bz, o, table, c.listed),
        &mut kinds,
        &mut failed,
    );
    met_each(
        &kinds,
        &[
            "hit",
            "past its reach",
            "lacks",
            "hit, placed",
            "hit, short of its cube",
            "hit, a way's",
            "ended in another root",
        ],
        &mut failed,
    );
    assert!(failed.is_empty(), "{}", failed.join("\n"));
}

/// A screen's pixels the cast marked missed, their rays as the walks again take them: how many
/// across and down, the eye's jitter, and one in so many marked, its walked and owed bits set
/// (pxl-engine `ray::hole_bit`).
struct Screen {
    name: &'static str,
    size: [u32; 2],
    jitter: [f32; 2],
    marked_in: u32,
}

/// The tangents across and up of pixel `p`'s ray through its jittered middle, row 0 at the
/// top: the rule every pass that walks a drawn pixel takes it by (grid.wgsl `px_tan`).
fn px_tan(s: &Screen, tan: [f32; 2], p: [u32; 2]) -> [f32; 2] {
    let ndc_x = (p[0] as f32 + 0.5 + s.jitter[0]) / s.size[0] as f32 * 2.0 - 1.0;
    let ndc_y = 1.0 - (p[1] as f32 + 0.5 + s.jitter[1]) / s.size[1] as f32 * 2.0;
    [ndc_x * tan[0], ndc_y * tan[1]]
}

/// The walked and the owed bits of the eye's holes `holes`, from their first word, of pixels
/// `marked` of a target `size`, each its row times the width and its column.
fn set_holes(holes: &mut [u32], size: [u32; 2], marked: &[u32]) {
    let owed = ray::owed_bits_at(size) as usize;
    for &p in marked {
        let (w, bit) = ray::hole_bit([p % size[0], p / size[0]], size);
        holes[w as usize] |= 1 << bit;
        holes[owed + w as usize] |= 1 << bit;
    }
}

/// Pixel `p`'s ray and the zooms it asks.
fn px_aim(obs: &Observed, s: &Screen, p: [u32; 2]) -> (Ray, Detail) {
    let Rays::Eye {
        axes,
        far,
        range,
        grid,
        ..
    } = &obs.rays
    else {
        unreachable!()
    };
    let at = px_tan(s, grid.tan, p);
    let d = cast::through(axes[0], axes[1], axes[2], at[0], at[1]);
    let detail = Detail {
        range: *range,
        spacing: Spacing {
            angle: grid.s,
            across: 0.0,
        },
    };
    (Ray::new(d, *far), detail)
}

/// The holes' queue over the eye's marked pixels (queue.wgsl, queue_walk.wgsl, queue_ask.wgsl,
/// queue_last.wgsl): every marked pixel's words as `cast::walk` from the root answers its ray,
/// and no other pixel written.
#[test]
fn hand_missed_pixels_answer_as_cast_walk() {
    let Some(gpu) = device() else { return };
    let built = real_world::build(EYE);
    let b = world_bufs(&gpu, &built);
    let w = built.world();
    let at = spots(&built);
    let none = [Cut::NONE; 2];
    let over = |g: [i32; 2], up: i32| q8([g[0], g[1], built.top_at(g) + up]);
    let eyes = [
        EyeCase {
            name: "the eye, over the ground",
            origin: over([EYE[0] + 90, EYE[1] + 90], 60),
            look: [0.5, 0.4, -0.77],
            tan: [0.3, 0.2],
            lattice: 1,
            far: 2000.0,
            range: [8, FINEST],
            cuts: none,
        },
        EyeCase {
            name: "the eye, lattice 2, over the seam",
            origin: over([EYE[0] + 280, EYE[1] + 90], 40),
            look: [0.8, 0.2, -0.55],
            tan: [0.4, 0.3],
            lattice: 2,
            far: 2000.0,
            range: [8, FINEST],
            cuts: none,
        },
        EyeCase {
            name: "the eye, on a bridge",
            origin: q8([at.bridge[0], at.bridge[1], at.bridge[2] + 12]),
            look: [0.5, -0.3, -0.8],
            tan: [0.3, 0.3],
            lattice: 1,
            far: 600.0,
            range: [LEAF_CELL, FINEST],
            cuts: none,
        },
    ];
    let screens = [
        Screen {
            name: "every pixel marked and listed",
            size: [96, 61],
            jitter: [0.0, 0.0],
            marked_in: 1,
        },
        Screen {
            name: "one in three marked, jittered",
            size: [131, 77],
            jitter: [0.25, -0.375],
            marked_in: 3,
        },
    ];
    let mut kinds = Kinds::new();
    let mut failed = Vec::new();
    for c in &eyes {
        for s in &screens {
            let name = format!("{}, {}", c.name, s.name);
            let mut obs = eye_of(c, [0; 3]);
            for k in 0..2 {
                obs.eye[eye::JITTER + k] = s.jitter[k].to_bits();
            }
            let mut table0 = vec![0u32; node_cache::WORDS as usize];
            table0[node_cache::FRAME as usize] = FRAME;
            let table_buf = gpu.buffer(&table0, store_of());
            let mut words = eye_beams(&gpu, &b, &obs, &table_buf);
            let table = gpu.read(&table_buf);
            let Rays::Eye { origin, grid, .. } = &obs.rays else {
                unreachable!()
            };
            let (origin, grid) = (*origin, *grid);
            // The eye's holes past the grid's words, a cast's run's (`ray::FROM_WALKED`).
            let walked = grid.walked_at() as usize;
            words.resize(walked + ray::holes_words(s.size) as usize, 0);

            // The pixels the cast marked, their walked and owed bits set.
            let [wd, ht] = s.size;
            let texels = (wd * ht) as usize;
            let mark = |p: u32| (p.wrapping_mul(0x9e37_79b9) >> 7).is_multiple_of(s.marked_in);
            let marked: Vec<u32> = (0..wd * ht).filter(|&p| mark(p)).collect();
            set_holes(&mut words[walked..], s.size, &marked);
            let mut ray_words: Vec<u32> = (0..ray::WORDS as usize * texels)
                .map(|k| (k as u32).wrapping_mul(0x2c1b_3c6d) >> 2)
                .collect();
            for &p in &marked {
                ray_words[p as usize] = UNWALKED;
            }

            // The holes' queue over them.
            let rays = texture(
                &gpu.device,
                &gpu.queue,
                s.size,
                ray::WORDS,
                wgpu::TextureUsages::STORAGE_BINDING,
                &ray_words,
            );
            let view = rays.create_view(&wgpu::TextureViewDescriptor {
                dimension: Some(wgpu::TextureViewDimension::D2Array),
                ..Default::default()
            });
            let eye_buf = gpu.buffer(&obs.eye, wgpu::BufferUsages::UNIFORM);
            // The eye's head, all zero (gpu/ray/cast.rs `eye_head`).
            let head = gpu.buffer(&vec![0u32; light::HEAD_WORDS], wgpu::BufferUsages::UNIFORM);
            let beams = gpu.buffer(&words, store_of());
            let table_buf = gpu.buffer(&table, store_of());
            let mut binds = world_binds(&b, &eye_buf, &beams, &table_buf);
            binds.push((3, wgpu::BindingResource::TextureView(&view)));
            binds.push((11, head.as_entire_binding()));
            let pool = (walked as u32) + ray::pool_at(s.size, 0) + ray::POOL_CNT;
            gpu.holes(&binds, &table_buf, true, Some((&beams, pool)));
            let out = read_texture(&gpu.device, &gpu.queue, &rays);

            // Each marked pixel's stop as `cast::walk` from the root answers its ray.
            let (same, differ, stepped) =
                marked_answers(w, &obs, s, origin, &marked, &out, &name, &mut kinds);
            // A pixel the cast did not mark stands.
            let untouched = (0..wd * ht).filter(|&p| !mark(p)).all(|p| {
                (0..ray::WORDS as usize)
                    .all(|l| out[l * texels + p as usize] == ray_words[l * texels + p as usize])
            });
            if !untouched {
                failed.push(format!("{name}: a pixel not marked was written"));
            }
            eprintln!("{name}: {same} alike, {differ} differ, {stepped} a step over");
            if differ > 0 {
                failed.push(format!("{name}: {differ} pixels differ"));
            }
        }
    }
    met_each(&kinds, &["hit", "past its reach"], &mut failed);
    assert!(failed.is_empty(), "{}", failed.join("\n"));
}

/// A marked pixel whose cell's window holds a beam ray the budget parked waits on it: the holes'
/// queue parks its ray before its first step (queue.wgsl `item_waits`) and writes nothing of
/// it, so the pixel keeps the words it held, the picture it showed; every other pixel answers
/// as it does where no cell waits, its stop's number, which the order the walks end in gives,
/// apart.
#[test]
fn eye_holes_wait_on_a_cell_whose_beam_ray_has_not_ended() {
    let Some(gpu) = device() else { return };
    let built = real_world::build(EYE);
    let b = world_bufs(&gpu, &built);
    let over = |g: [i32; 2], up: i32| q8([g[0], g[1], built.top_at(g) + up]);
    let c = EyeCase {
        name: "the eye, over the ground",
        origin: over([EYE[0] + 90, EYE[1] + 90], 60),
        look: [0.5, 0.4, -0.77],
        tan: [0.3, 0.2],
        lattice: 1,
        far: 2000.0,
        range: [8, FINEST],
        cuts: [Cut::NONE; 2],
    };
    let s = Screen {
        name: "every pixel marked and listed",
        size: [96, 61],
        jitter: [0.0, 0.0],
        marked_in: 1,
    };
    let obs = eye_of(&c, [0; 3]);
    let mut table0 = vec![0u32; node_cache::WORDS as usize];
    table0[node_cache::FRAME as usize] = FRAME;
    let table_buf = gpu.buffer(&table0, store_of());
    let mut words = eye_beams(&gpu, &b, &obs, &table_buf);
    let table = gpu.read(&table_buf);
    let Rays::Eye { grid, .. } = &obs.rays else {
        unreachable!()
    };
    let grid = *grid;
    let [wd, ht] = s.size;
    let (drawn, texels) = (wd * ht, (wd * ht) as usize);
    // The eye's holes past the grid's words, every pixel's walked and owed bits set.
    let walked = grid.walked_at() as usize;
    words.resize(walked + ray::holes_words(s.size) as usize, 0);
    let every: Vec<u32> = (0..drawn).collect();
    set_holes(&mut words[walked..], s.size, &every);
    let pool = walked + ray::pool_at(s.size, 0) as usize;
    // Every third cell held over a beam ray not ended.
    let waits = |cell: [u32; 2]| (cell[0] + cell[1]).is_multiple_of(3);
    let mut held = words.clone();
    for y in 0..grid.span[1] {
        for x in 0..grid.span[0] {
            if waits([x, y]) {
                held[(grid.cell_at([x, y]) + beam::CELL_RESUMES) as usize] = beam::CELL_WAITS;
            }
        }
    }
    let ray_words: Vec<u32> = (0..ray::WORDS as usize * texels)
        .map(|k| (k as u32).wrapping_mul(0x2c1b_3c6d) >> 2)
        .collect();
    let eye_buf = gpu.buffer(&obs.eye, wgpu::BufferUsages::UNIFORM);
    let head = gpu.buffer(&vec![0u32; light::HEAD_WORDS], wgpu::BufferUsages::UNIFORM);
    let run = |beams_words: &[u32]| {
        let rays = texture(
            &gpu.device,
            &gpu.queue,
            s.size,
            ray::WORDS,
            wgpu::TextureUsages::STORAGE_BINDING,
            &ray_words,
        );
        let view = rays.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            ..Default::default()
        });
        let beams = gpu.buffer(beams_words, store_of());
        let table_buf = gpu.buffer(&table, store_of());
        let mut binds = world_binds(&b, &eye_buf, &beams, &table_buf);
        binds.push((3, wgpu::BindingResource::TextureView(&view)));
        binds.push((11, head.as_entire_binding()));
        let cnt = (pool + ray::POOL_CNT as usize) as u32;
        gpu.holes(&binds, &table_buf, true, Some((&beams, cnt)));
        (
            read_texture(&gpu.device, &gpu.queue, &rays),
            gpu.read(&beams),
        )
    };
    let (whole, _) = run(&words);
    let (out, states) = run(&held);
    // The pixels whose rays the run left parked waiting in its pool.
    let parked = states[pool + ray::POOL_CNT as usize].min(ray::PARKED_ROOM);
    let waiting: BTreeSet<u32> = (0..parked)
        .filter_map(|k| {
            let at = pool + (ray::POOL_STATES + k * ray::PARK_WORDS) as usize;
            let state = states[at + 4];
            let p = states[at + ray::PARK_PX as usize];
            (state & ray::PARKED != 0 && state & 63 == 63).then_some((p >> 16) * wd + (p & 0xffff))
        })
        .collect();
    let px = |p: u32| [p % wd, p / wd];
    // The cell the pixel's ray goes through, and whether a nudge of its tangents takes another,
    // where the device's rounding may.
    let cell_of = |p: u32, d: f32| {
        let t = px_tan(&s, grid.tan, px(p));
        grid.cell([t[0] + d, t[1] + d])
    };
    let (mut waited, mut answered, mut differ) = (0u32, 0u32, Vec::new());
    for p in 0..drawn {
        let words_of = |v: &[u32]| {
            (0..ray::WORDS as usize)
                .map(|l| v[l * texels + p as usize])
                .collect::<Vec<_>>()
        };
        let kept = words_of(&out) == words_of(&ray_words);
        let parked_waiting = waiting.contains(&p);
        let at_edge = cell_of(p, 1e-5) != cell_of(p, -1e-5);
        if kept && parked_waiting {
            waited += 1;
            if !waits(cell_of(p, 0.0)) && !at_edge {
                differ.push(format!("{:?} waited in a cell that does not", px(p)));
            }
        } else if words_of(&out)[..3] == words_of(&whole)[..3] {
            answered += 1;
            if waits(cell_of(p, 0.0)) && !at_edge {
                differ.push(format!("{:?} walked in a cell that waits", px(p)));
            }
        } else {
            differ.push(format!(
                "{:?} neither waited nor answered as the whole run",
                px(p)
            ));
        }
    }
    eprintln!("{waited} pixels waited, {answered} answered");
    assert!(
        waited > 100 && answered > 100,
        "{waited} waited, {answered} answered"
    );
    assert!(
        differ.is_empty(),
        "{} differ, first {}",
        differ.len(),
        differ[0]
    );
}

/// Each of the `marked` pixels' words in the rays `out` as `cast::walk` from the root answers
/// its ray from `origin`, a start a step over as a GPU rounds it taken where the first is not,
/// `STEPS_TRIED` at most: how many alike, how many differ and how many a step over; what each
/// met counted in `kinds`.
#[expect(
    clippy::too_many_arguments,
    reason = "the world, the observer, its screen and origin, the pixels, their words and what to tell"
)]
fn marked_answers(
    w: World,
    obs: &Observed,
    s: &Screen,
    origin: [i32; 3],
    marked: &[u32],
    out: &[u32],
    name: &str,
    kinds: &mut Kinds,
) -> (u32, u32, u32) {
    let [wd, ht] = s.size;
    let texels = (wd * ht) as usize;
    let (mut same, mut differ, mut stepped) = (0u32, 0u32, 0u32);
    for &p in marked {
        let px = [p % wd, p / wd];
        let (r, d) = px_aim(obs, s, px);
        let met = walk_from(w, obs.cuts, origin, &r, d);
        let got = |l: u32| out[l as usize * texels + p as usize];
        let face = 2 * met.hit.axis + u32::from(r.dir[met.hit.axis as usize] > 0.0);
        let mut ok = got(0) != UNWALKED;
        if met.ends == End::Hit {
            // The face it enters across, or, at an edge between two axes' planes, the other's as
            // the device's rounding of the ray may take it (`edge_axes`, as tests/cast.rs takes
            // a pixel at an edge).
            let e = cast::entry_with(origin, &r, r.inverse(), &met.hit.node, met.hit.asset[0]);
            let axes = edge_axes(&e, r.dir);
            let entered = |axis: u32| {
                got(0)
                    == ray::first_word(met.hit.asset[0], ray::face(r.dir, axis), met.hit.node.zoom)
            };
            let other = !entered(met.hit.axis) && axes.iter().any(|&a| entered(a));
            kinds.add("at an edge", axes.len() > 1);
            kinds.add("at an edge, the other axis's face", other);
            ok = ok && (entered(met.hit.axis) || other) && got(2) == met.hit.asset[1];
            let t = f32::from_bits(got(1));
            ok = ok && (t - met.hit.t).abs() <= 1e-4 * met.hit.t.max(1.0);
        } else {
            ok = ok && got(0) == 0 && got(1) == 0;
        }
        if !ok && stepped <= STEPS_TRIED {
            // A start a step over, as a GPU rounds it.
            let o = origin;
            let again = (0..27).any(|m| {
                let from = [o[0] + m % 3 - 1, o[1] + (m / 3) % 3 - 1, o[2] + m / 9 - 1];
                let m = walk_from(w, obs.cuts, from, &r, d);
                m.ends == met.ends
                    && (m.ends != End::Hit
                        || got(0) == ray::first_word(m.hit.asset[0], face, m.hit.node.zoom))
            });
            if again {
                stepped += 1;
                ok = true;
            }
        }
        if ok {
            same += 1;
        } else {
            differ += 1;
            if differ <= 3 {
                eprintln!(
                    "DIFFERS {name} {px:?}: want {:?} z{} {:?} t{} axis {} asset {:x?}; got {:x?}",
                    met.ends,
                    met.hit.node.zoom,
                    met.hit.node.corner,
                    met.hit.t,
                    met.hit.axis,
                    met.hit.asset,
                    [got(0), got(1), got(2), got(3)]
                );
            }
            assert!(
                std::env::var_os("MUT_WGSL").is_none(),
                "{name}: {px:?} differs"
            );
        }
        kinds.add("hit", met.ends == End::Hit);
        kinds.add("past its reach", met.ends == End::Miss);
    }
    (same, differ, stepped)
}

/// The eye's holes' queue run alone frame after frame while its rays stand, as the host runs it
/// where the eye is owed (gpu/ray/cast.rs `eye_queue`), each frame's budget of visits and asks
/// far less than its holes take (`node_cache::caps_at`): the first frame over the cast's holes,
/// each after over the owed bits and the pool the one before left (`ray::from_at`,
/// `ray::pool_at`), its pool's count copied in as the host copies it, the node cache's frame
/// numbered on and the frame's work set to none each frame. What a frame leaves owed, its owed
/// bits and its pool's parked rays, holds each pixel once, every one owed by the frame before,
/// so a pixel a walk finished is never walked again, and its words then stand; the eye owes
/// nothing within
/// `FRAMES_TO_REST` frames, more than one of them; and every marked pixel's words are then as
/// `cast::walk` from the root answers its ray, a ray at an edge between two axes' planes across
/// either face as the device's rounding of the ray takes it (`edge_axes`), one such ray met, no
/// other pixel written.
#[test]
fn eye_queue_alone_comes_to_rest_as_cast_walk() {
    /// The frame's budget, far less than the holes take over an empty node cache.
    const CAPS: [u32; 2] = [1 << 12, 1 << 7];
    let Some(gpu) = device() else { return };
    let built = real_world::build(EYE);
    let b = world_bufs(&gpu, &built);
    let w = built.world();
    let over = |g: [i32; 2], up: i32| q8([g[0], g[1], built.top_at(g) + up]);
    let looking = |name, look| EyeCase {
        name,
        origin: over([EYE[0] + 90, EYE[1] + 90], 60),
        look,
        tan: [0.3, 0.2],
        lattice: 1,
        far: 150.0,
        range: [8, FINEST],
        cuts: [Cut::NONE; 2],
    };
    // Up over the hills, a ray meets the edge of two faces of a cell, its two planes' t a bit
    // apart, which the device's rounding of its way and reciprocal breaks the other way
    // (pixel [15, 39]).
    let cases = [
        looking("the eye, over the ground to its reach", [0.6, 0.5, -0.12]),
        looking(
            "the eye, up over the hills, a ray at an edge",
            [0.6, 0.5, 0.1],
        ),
    ];
    let s = Screen {
        name: "every pixel marked and listed",
        size: [131, 77],
        jitter: [0.25, -0.375],
        marked_in: 1,
    };
    let mut kinds = Kinds::new();
    let mut failed = Vec::new();
    for c in &cases {
        let (frames, _) = queue_alone(&gpu, &b, w, (c, &s), CAPS, &mut kinds, &mut failed);
        if frames < 2 {
            failed.push(format!(
                "{}: the budget took the holes in one frame",
                c.name
            ));
        }
    }
    met_each(
        &kinds,
        &["hit", "past its reach", "at an edge"],
        &mut failed,
    );
    assert!(failed.is_empty(), "{}", failed.join("\n"));
}

/// The eye's holes' queue run alone over every pixel of `s` marked, from the eye of case `c`,
/// frame after frame under the budget `caps` until it owes nothing (`FRAMES_TO_REST` at most),
/// held as `eye_queue_alone_comes_to_rest_as_cast_walk` holds it: how many frames it took and
/// the stops its first frame counted (`node_cache::HITS_CNT`).
fn queue_alone(
    gpu: &Gpu,
    b: &WorldBufs,
    w: World,
    (c, s): (&EyeCase, &Screen),
    caps: [u32; 2],
    kinds: &mut Kinds,
    failed: &mut Vec<String>,
) -> (u32, u32) {
    /// The frames the eye's queue may take to owe nothing over its holes.
    const FRAMES_TO_REST: u32 = 256;
    let mut first_stops = 0;
    let name = format!("{}, {}, its queue alone", c.name, s.name);
    let mut obs = eye_of(c, [0; 3]);
    for k in 0..2 {
        obs.eye[eye::JITTER + k] = s.jitter[k].to_bits();
    }
    let mut table0 = vec![0u32; node_cache::WORDS as usize];
    table0[node_cache::FRAME as usize] = FRAME;
    let table_buf = gpu.buffer(&table0, store_of());
    let mut words = eye_beams(gpu, b, &obs, &table_buf);
    let table = gpu.read(&table_buf);
    let Rays::Eye { origin, grid, .. } = &obs.rays else {
        unreachable!()
    };
    let (origin, grid) = (*origin, *grid);
    let drawn = s.size[0] * s.size[1];
    let walked = grid.walked_at() as usize;
    words.resize(walked + ray::holes_words(s.size) as usize, 0);
    let marked: Vec<u32> = (0..drawn).collect();
    set_holes(&mut words[walked..], s.size, &marked);
    let texels = drawn as usize;
    let ray_words: Vec<u32> = (0..ray::WORDS as usize * texels)
        .map(|k| {
            if k < texels {
                UNWALKED
            } else {
                (k as u32).wrapping_mul(0x2c1b_3c6d) >> 2
            }
        })
        .collect();
    let rays = texture(
        &gpu.device,
        &gpu.queue,
        s.size,
        ray::WORDS,
        wgpu::TextureUsages::STORAGE_BINDING,
        &ray_words,
    );
    let view = rays.create_view(&wgpu::TextureViewDescriptor {
        dimension: Some(wgpu::TextureViewDimension::D2Array),
        ..Default::default()
    });
    let eye_buf = gpu.buffer(&obs.eye, wgpu::BufferUsages::UNIFORM);
    // The eye's head, all zero (gpu/ray/cast.rs `eye_head`).
    let head = gpu.buffer(&vec![0u32; light::HEAD_WORDS], wgpu::BufferUsages::UNIFORM);
    let beams = gpu.buffer(&words, store_of());
    let table_buf = gpu.buffer(&table, store_of());
    let mut binds = world_binds(b, &eye_buf, &beams, &table_buf);
    binds.push((3, wgpu::BindingResource::TextureView(&view)));
    binds.push((11, head.as_entire_binding()));

    let word = |k: u32| u64::from(k) * 4;
    let from_at = (walked as u32) + ray::from_at(s.size);
    let owed = node_cache::owed_at(count::EYE);
    // The pixels the frame before left owed, the cast's holes before the first.
    let mut owed_before: BTreeSet<u32> = marked.iter().copied().collect();
    // Each pixel's words once a walk finished it.
    let mut finished: BTreeMap<u32, [u32; 4]> = BTreeMap::new();
    let mut into = 0;
    let mut frames = 0u32;
    loop {
        // As the host sets each frame the eye's queue runs: what it takes, the frame's
        // work, its owed word and its stops none, and, after the first, a frame of the node
        // cache's own.
        let from = if frames == 0 {
            ray::FROM_WALKED
        } else {
            ray::FROM_OWED + into
        };
        gpu.queue
            .write_buffer(&beams, word(from_at), bytes(&[from]));
        into = ray::owed_into(from);
        let none = vec![0u32; (owed + 1 - node_cache::VISITED) as usize];
        gpu.queue
            .write_buffer(&table_buf, word(node_cache::VISITED), bytes(&none));
        gpu.queue.write_buffer(
            &table_buf,
            word(node_cache::HITS_ARGS),
            bytes(&[0u32, 1, 1, 0]),
        );
        if frames > 0 {
            let f = gpu.read(&table_buf)[node_cache::FRAME as usize] + 1;
            gpu.queue
                .write_buffer(&table_buf, word(node_cache::FRAME), bytes(&[f]));
        }
        let pool = (walked as u32) + ray::pool_at(s.size, into);
        gpu.holes_within(
            &binds,
            &table_buf,
            true,
            Some((&beams, pool + ray::POOL_CNT)),
            caps,
        );
        frames += 1;
        if frames == 1 {
            first_stops = gpu.read(&table_buf)[node_cache::HITS_CNT as usize];
        }
        // What it left owed: the pixels of its owed bits and of the rays parked in its pool.
        let now = gpu.read(&beams);
        let mut listed: Vec<u32> = Vec::new();
        let owed_bits = walked + ray::owed_bits_at(s.size) as usize;
        for y in 0..s.size[1] {
            for x in 0..s.size[0] {
                let (w, bit) = ray::hole_bit([x, y], s.size);
                if now[owed_bits + w as usize] & 1 << bit != 0 {
                    listed.push(y * s.size[0] + x);
                }
            }
        }
        let cnt = now[(pool + ray::POOL_CNT) as usize].min(ray::PARKED_ROOM);
        for k in 0..cnt {
            let at = (pool + ray::POOL_STATES + k * ray::PARK_WORDS) as usize;
            if now[at + 4] & ray::PARKED != 0 {
                let h = now[at + ray::PARK_PX as usize];
                listed.push((h >> 16) * s.size[0] + (h & 0xffff));
            }
        }
        let left = listed.len();
        let owed_now: BTreeSet<u32> = listed.iter().copied().collect();
        if owed_now.len() != listed.len() {
            failed.push(format!("frame {frames}: a pixel owed twice"));
        }
        if !owed_now.is_subset(&owed_before) {
            failed.push(format!(
                "frame {frames}: a pixel owed that the frame before did not owe"
            ));
        }
        // A pixel finished keeps its words; one finished this frame is kept.
        let out = read_texture(&gpu.device, &gpu.queue, &rays);
        let at = |p: u32| [0, 1, 2, 3].map(|l| out[l * texels + p as usize]);
        for (&p, was) in &finished {
            if at(p) != *was {
                failed.push(format!(
                    "frame {frames}: pixel {p} finished before was written again"
                ));
                break;
            }
        }
        for &p in owed_before.difference(&owed_now) {
            finished.insert(p, at(p));
        }
        owed_before = owed_now;
        if left == 0 || frames >= FRAMES_TO_REST || !failed.is_empty() {
            break;
        }
    }
    eprintln!("{name}: the eye owed nothing after {frames} frames");
    if !owed_before.is_empty() {
        failed.push(format!(
            "{name}: {} pixels still owed after {frames} frames",
            owed_before.len()
        ));
    }
    let out = read_texture(&gpu.device, &gpu.queue, &rays);
    let (same, differ, stepped) = marked_answers(w, &obs, s, origin, &marked, &out, &name, kinds);
    eprintln!("{name}: {same} alike, {differ} differ, {stepped} a step over");
    if differ > 0 {
        failed.push(format!("{name}: {differ} pixels differ"));
    }
    (frames, first_stops)
}

/// The eye's holes' queue run alone over more pixels than a frame's stops hold
/// (`ray::STOPS_A_FRAME`), its budget of visits and asks past all they take: the first frame
/// keeps a frame's stops and leaves the rest parked and owed, none dropped, and the frames after
/// end them, every marked pixel's words then as `cast::walk` from the root answers its ray
/// (`queue_alone`).
#[test]
fn eye_stops_past_a_frame_are_owed() {
    let Some(gpu) = device() else { return };
    let built = real_world::build(EYE);
    let b = world_bufs(&gpu, &built);
    let w = built.world();
    let c = EyeCase {
        name: "the eye, over the ground, near",
        origin: q8([
            EYE[0] + 90,
            EYE[1] + 90,
            built.top_at([EYE[0] + 90, EYE[1] + 90]) + 30,
        ]),
        look: [0.5, 0.4, -0.77],
        tan: [0.3, 0.2],
        lattice: 1,
        far: 600.0,
        range: [8, FINEST],
        cuts: [Cut::NONE; 2],
    };
    let s = Screen {
        name: "more pixels marked than a frame's stops",
        size: [512, 320],
        jitter: [0.25, -0.375],
        marked_in: 1,
    };
    assert!(
        s.size[0] * s.size[1] > ray::STOPS_A_FRAME,
        "the screen's holes pass a frame's stops"
    );
    let mut kinds = Kinds::new();
    let mut failed = Vec::new();
    let (frames, first) = queue_alone(
        &gpu,
        &b,
        w,
        (&c, &s),
        [1 << 30, 1 << 30],
        &mut kinds,
        &mut failed,
    );
    eprintln!(
        "{}: {first} stops counted in the first of {frames} frames",
        c.name
    );
    if first <= ray::STOPS_A_FRAME {
        failed.push(format!(
            "{}: {first} stops in the first frame, not past {}",
            c.name,
            ray::STOPS_A_FRAME
        ));
    }
    if frames < 2 {
        failed.push(format!("{}: every stop kept in one frame", c.name));
    }
    assert!(failed.is_empty(), "{}", failed.join("\n"));
}

/// A word the boxes' writing leaves standing where it writes none.
const UNWRITTEN: u32 = 0x5eed_0b0c;

/// The samples' words as the samples' run leaves them over `table`, each sample's words laid
/// first as a cast before left them in `prior`, else `UNTOUCHED`.
fn stopped(
    gpu: &Gpu,
    b: &WorldBufs,
    obs: &Observed,
    words: &[u32],
    prior: Option<&[u32]>,
    table: &[u32],
) -> Vec<u32> {
    let eye_buf = gpu.buffer(&obs.eye, wgpu::BufferUsages::UNIFORM);
    let head_buf = gpu.buffer(&obs.head, wgpu::BufferUsages::UNIFORM);
    let mut laid = words.to_vec();
    for j in 0..obs.lattice[1] {
        for i in 0..obs.lattice[0] {
            let at = obs.sample_at([i, j]) as usize;
            let to = at + beam::SAMPLE_WORDS as usize;
            match prior {
                Some(p) => laid[at..to].copy_from_slice(&p[at..to]),
                None => laid[at..to].fill(UNTOUCHED),
            }
        }
    }
    let beams = gpu.buffer(&laid, store_of());
    let table = gpu.buffer(table, store_of());
    let mut binds = world_binds(b, &eye_buf, &beams, &table);
    if obs.apart {
        binds.push((11, head_buf.as_entire_binding()));
    }
    gpu.samples(obs.apart, &binds, &table);
    gpu.read(&beams)
}

/// A box as the raster forms it off its draw word and its entry (pxl-engine `raster`), as the
/// test lays it: its planes, lowest then highest, f32 bits (`BOX_LO`, `BOX_HI`), its voxel's
/// number (`BOX_ID`, `raster::id_of`) and its faces with `raster::EYED` (`BOX_FACES`).
type BoxWords = [u32; BOX_WORDS as usize];
const BOX_LO: u32 = 0;
const BOX_HI: u32 = 3;
const BOX_ID: u32 = 6;
const BOX_FACES: u32 = 7;
const BOX_WORDS: u32 = 8;

/// A voxel's key as a sample's words hold it: its node, its zoom, corner and roots from where
/// its ray starts.
type Key = [u32; beam::NODE_WORDS as usize];

/// The key of the sample at word `at` of `words`.
fn sample_key(words: &[u32], at: usize) -> Key {
    key_words(words, at + beam::SAMPLE_NODE as usize)
}

/// The node words at word `k` of `words` (`beam::NODE_WORDS`).
fn key_words(words: &[u32], k: usize) -> Key {
    core::array::from_fn(|i| words[k + i])
}

/// The node a key's words name, as a walk from where its ray starts holds it.
fn node_of(key: &Key) -> Node {
    Node {
        zoom: key[beam::NODE_ZOOM as usize],
        corner: [0, 1, 2].map(|a| key[beam::NODE_CORNER as usize + a]),
        roots: [0, 1, 2].map(|a| key[beam::NODE_ROOTS as usize + a] as i32),
    }
}

/// The node's words as a sample's hold it (`node_of`'s inverse).
fn node_words(n: &Node) -> Key {
    let mut w = [0u32; beam::NODE_WORDS as usize];
    w[beam::NODE_ZOOM as usize] = n.zoom;
    for a in 0..3 {
        w[beam::NODE_CORNER as usize + a] = n.corner[a];
        w[beam::NODE_ROOTS as usize + a] = n.roots[a] as u32;
    }
    w
}

/// Each voxel the samples' words `words` draw from `origin`, by its key (`sample_key`): a
/// node a sample that stopped stopped on, its box the cell as the cast stops on it
/// (`cast::asset_box`), `raster::EYED` where `tan` is the screen's and it lies about the eye
/// (`raster::is_eyed`), its faces drawn each axis a sample of it entered across
/// (`raster::faces`), and its id but the voxel's number, which the boxes' order gives
/// (`BOX_ID` left 0).
fn boxes_want(
    obs: &Observed,
    words: &[u32],
    origin: [i32; 3],
    tan: Option<[f32; 2]>,
    kinds: &mut Kinds,
) -> BTreeMap<Key, BoxWords> {
    let mut want: BTreeMap<Key, BoxWords> = BTreeMap::new();
    let mut add = |k: &'static str, yes: bool| kinds.add(k, yes);
    for s in obs.taken() {
        let k = obs.sample_at(s) as usize;
        let node = k + beam::SAMPLE_NODE as usize;
        let zoom = words[node + beam::NODE_ZOOM as usize];
        add("samples", true);
        if zoom == beam::SAMPLE_NONE {
            add("past its reach, no box", true);
            continue;
        }
        let axis = words[k + beam::SAMPLE_AXIS as usize];
        // The side its ray entered through (`ray::face`).
        let face = ray::face(obs.aim(s).1.dir, axis);
        let key = sample_key(words, k);
        if let Some(w) = want.get_mut(&key) {
            add("a voxel another sample drew", true);
            add(
                "a voxel another sample drew, through another side",
                w[BOX_FACES as usize] >> face & 1 == 0,
            );
            w[BOX_FACES as usize] |= raster::faces([face]);
            continue;
        }
        let cell = Node {
            zoom,
            corner: [0, 1, 2].map(|a| words[node + beam::NODE_CORNER as usize + a]),
            roots: [0, 1, 2].map(|a| words[node + beam::NODE_ROOTS as usize + a] as i32),
        };
        let asset = words[k + beam::SAMPLE_ASSET as usize];
        let (lo, hi) = cast::asset_box(origin, &cell, asset);
        // boxes.wgsl insets a cell as `lo + (hi - lo) · by`, which a device may fuse; a fused
        // sum rounds as the Rust's product and sum apart wherever the product is exact, as a
        // node's side, a power of two, times `by` is. Each inset's product held exact, so the
        // planes are compared word for word.
        let eighths = answer::fill_eighths(asset);
        if eighths != answer::EIGHTHS {
            let whole = asset & !(((1 << answer::FILL_BITS) - 1) << answer::FILL_SHIFT);
            let (node_lo, node_hi) = cast::asset_box(origin, &cell, whole);
            let by = 0.5 * (1.0 - answer::side(eighths));
            for a in 0..3 {
                let side = node_hi[a] - node_lo[a];
                add(
                    "boxes, an inset's product rounded",
                    f64::from(side * by) != f64::from(side) * f64::from(by),
                );
            }
        }
        let lacks = words[k + beam::SAMPLE_LACKS as usize] != 0;
        let eyed = tan.is_some_and(|t| raster::is_eyed(lo, hi, t));
        let mut w = [0u32; BOX_WORDS as usize];
        for a in 0..3 {
            w[(BOX_LO as usize) + a] = lo[a].to_bits();
            w[(BOX_HI as usize) + a] = hi[a].to_bits();
        }
        w[BOX_ID as usize] = if lacks { raster::LACK } else { 0 };
        w[BOX_FACES as usize] = raster::faces([face]) | if eyed { raster::EYED } else { 0 };
        add("boxes", true);
        add("boxes, eyed", eyed);
        add("boxes, eyed, a cell", eyed && !lacks);
        add(
            "boxes, eyed, past the eye",
            eyed && lo.iter().any(|&v| v > 0.0),
        );
        // Eyed, its nearest point past what the screen's corner adds to the reach alone, and past
        // half the reach.
        let d2 = |a: usize| {
            let v = if lo[a] > 0.0 {
                lo[a]
            } else if hi[a] < 0.0 {
                hi[a]
            } else {
                0.0
            };
            v * v
        };
        let near = d2(0) + d2(1) + d2(2);
        let corner = tan.map_or(0.0, |t| t[0] * t[0] + t[1] * t[1]);
        let reach = 4.0 * raster::NEAR * raster::NEAR;
        add(
            "boxes, eyed, past the corner's share and half the reach",
            eyed && near >= reach * corner && near >= reach / 4.0 * (1.0 + corner),
        );
        add("boxes, lacks", lacks);
        add(
            "boxes, short of its cube",
            answer::fill_eighths(asset) < answer::EIGHTHS,
        );
        add("boxes, in another root", cell.roots != [0; 3]);
        want.insert(key, w);
    }
    want
}

/// The light's samples' stops by hand over `words`, in frame `frame`, into a node cache that
/// keeps each node a sample stopped on as its walk would (`lay_kept`), against pxl-engine
/// `record`: each such entry's state the faces its samples entered across and whether some
/// ray of them stopped on it coarser than its band (`record::light_whole`), marked in the
/// frame's number, as it draws from then; and no other entry drawn. Fails into `failed`.
#[expect(
    clippy::too_many_arguments,
    reason = "the device, the pass, its observer, words, groups, frame, what to tell and what failed"
)]
fn light_record_case(
    gpu: &Gpu,
    pass: &wgpu::ComputePipeline,
    obs: &Observed,
    words: &[u32],
    groups: [u32; 2],
    frame: u32,
    kinds: &mut Kinds,
    failed: &mut Vec<String>,
) {
    let name = &obs.name;
    let mut laid = frame_table();
    laid[node_cache::FRAME as usize] = frame;
    // Each stopped sample's node's slot, as the walk that stopped it keeps it, and the state
    // its samples set.
    let mut want: BTreeMap<u32, u32> = BTreeMap::new();
    for s in obs.taken() {
        let k = obs.sample_at(s) as usize;
        let node = k + beam::SAMPLE_NODE as usize;
        let zoom = words[node + beam::NODE_ZOOM as usize];
        if zoom == beam::SAMPLE_NONE {
            continue;
        }
        let n = Node {
            zoom,
            corner: [0, 1, 2].map(|a| words[node + beam::NODE_CORNER as usize + a]),
            roots: [0, 1, 2].map(|a| words[node + beam::NODE_ROOTS as usize + a] as i32),
        };
        let o = obs.aim(s).0;
        let lacks = words[k + beam::SAMPLE_LACKS as usize] != 0;
        let asset = [
            words[k + beam::SAMPLE_ASSET as usize],
            words[k + beam::SAMPLE_ASSET as usize + 1],
        ];
        let kind = if lacks { Kind::Lacks } else { Kind::Cell };
        let idx = n.idx(o);
        let first = node_cache::bucket_of(zoom, idx);
        let slot = lay_kept(&mut laid, o, &n, (kind, asset)).or_else(|| {
            (first..first + node_cache::BUCKET).find(|&slot| {
                let e = node_cache::entry_at(slot) as usize;
                let w = e + node_cache::W_NODE as usize;
                entry_held(&laid, e)
                    && laid[w] == zoom
                    && [1, 2, 3].map(|a| laid[w + a] as i32) == idx
            })
        });
        let Some(slot) = slot else {
            kinds.add("voxels, their bucket full", true);
            continue;
        };
        let state = raster::faces([ray::face(
            obs.aim(s).1.dir,
            words[k + beam::SAMPLE_AXIS as usize],
        )]) | if record::light_whole(zoom, obs.band(s)) {
            record::WHOLE
        } else {
            0
        };
        *want.entry(slot).or_default() |= state;
        kinds.add("voxels", true);
        kinds.add("voxels, lacks", lacks);
        kinds.add("voxels, in another root", n.roots != [0; 3]);
        kinds.add(
            "voxels, short of its cube",
            answer::fill_eighths(asset[0]) < answer::EIGHTHS,
        );
    }
    let mut args = vec![0u32; raster::ARGS_WORDS as usize];
    args[raster::ARGS_VERTICES as usize] = raster::VERTICES;
    let eye_buf = gpu.buffer(&obs.eye, wgpu::BufferUsages::UNIFORM);
    let head_buf = gpu.buffer(&obs.head, wgpu::BufferUsages::UNIFORM);
    let beams = gpu.buffer(words, store_of());
    let table = gpu.buffer(&laid, store_of());
    let args = gpu.buffer(&args, store_of());
    gpu.dispatch(
        pass,
        &[
            (0, eye_buf.as_entire_binding()),
            (8, beams.as_entire_binding()),
            (9, table.as_entire_binding()),
            (11, head_buf.as_entire_binding()),
            (16, args.as_entire_binding()),
        ],
        [groups[0], groups[1], 1],
    );
    let got = gpu.read(&table);
    let mut differ = 0u64;
    for (&slot, &state) in &want {
        let e = node_cache::entry_at(slot) as usize;
        let alike = got[e + node_cache::W_STATE as usize] == state
            && got[e + node_cache::W_SEEN as usize] == frame;
        differ += u64::from(!alike);
    }
    let stray = (0..node_cache::ENTRIES)
        .filter(|slot| {
            let e = node_cache::entry_at(*slot) as usize;
            got[e + node_cache::W_STATE as usize] != 0 && !want.contains_key(slot)
        })
        .count();
    eprintln!(
        "{name}: {} voxels, {differ} differ, {stray} drawn that no sample stopped on",
        want.len()
    );
    if differ != 0 || stray != 0 {
        failed.push(format!(
            "{name}: {differ} voxels differ, {stray} drawn that no sample stopped on"
        ));
    }
    if want.is_empty() {
        failed.push(format!("{name}: no voxel to compare"));
    }
    light_raster_drawn(gpu, obs, &got, kinds, failed);
}

/// The light's record's draw over the node cache `cache` (record_draw.wgsl
/// `light_record_draw_cs`), then the raster's draw of its draws from the light, as the renderer
/// lays the light's view (gpu/ray/raster.rs `light_prepare`), onto the map of `obs`: every
/// texel that keeps a voxel keeps one whose box, as the cast stops on its node
/// (`cast::asset_box`), the texel's own ray, from where it starts on the near plane along the
/// light, enters within its reach, but where the device's rounding may decide it otherwise (a
/// box grown by its rounding's room); and some texel keeps one.
fn light_raster_drawn(
    gpu: &Gpu,
    obs: &Observed,
    cache: &[u32],
    kinds: &mut Kinds,
    failed: &mut Vec<String>,
) {
    use pxl_engine::param::raster::GROWTH;
    use pxl_render::frame::shader::record::SHADER_RECORD_DRAW;
    use pxl_render::frame::shader::word::{BOX_VERTICES, RASTER_FLOATS};
    let Rays::Light {
        light: l,
        starts,
        size,
        ..
    } = &obs.rays
    else {
        unreachable!()
    };
    let size = *size;
    // The draw: a word a box, as many as its args count.
    let mut args = vec![0u32; raster::ARGS_WORDS as usize];
    args[raster::ARGS_VERTICES as usize] = raster::VERTICES;
    let args = gpu.buffer(&args, store_of());
    let draws = gpu.buffer(&vec![0u32; record::LIGHT_DRAWS as usize], store_of());
    let table = gpu.buffer(cache, store_of());
    let eye_buf = gpu.buffer(&obs.eye, wgpu::BufferUsages::UNIFORM);
    let head_buf = gpu.buffer(&obs.head, wgpu::BufferUsages::UNIFORM);
    let draw = gpu.pipeline(src(SHADER_RECORD_DRAW), "light_record_draw_cs");
    gpu.dispatch(
        &draw,
        &[
            (0, eye_buf.as_entire_binding()),
            (9, table.as_entire_binding()),
            (11, head_buf.as_entire_binding()),
            (14, draws.as_entire_binding()),
            (16, args.as_entire_binding()),
        ],
        [node_cache::ENTRIES / record::DRAW_LANES, 1, 1],
    );
    let cnt = gpu.read(&args)[raster::ARGS_INSTANCES as usize].min(record::LIGHT_DRAWS);
    let cache = gpu.read(&table);
    // The light's view, as the renderer lays it.
    let length = |a: [f32; 3]| (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt();
    let unit = |a: [f32; 3]| {
        let n = length(a);
        [a[0] / n, a[1] / n, a[2] / n]
    };
    let (r, d) = (unit(l.across), unit(l.down));
    let up = [-d[0], -d[1], -d[2]];
    let turn = BOX_VERTICES * u32::from(raster::is_flipped(r, up, l.dir));
    let origin = starts.start(l.corner);
    let n = size as f32;
    let view: [u32; RASTER_FLOATS] = [
        r[0].to_bits(),
        r[1].to_bits(),
        r[2].to_bits(),
        length(l.across).to_bits(),
        d[0].to_bits(),
        d[1].to_bits(),
        d[2].to_bits(),
        length(l.down).to_bits(),
        l.dir[0].to_bits(),
        l.dir[1].to_bits(),
        l.dir[2].to_bits(),
        0,
        turn,
        1,
        0,
        0,
        (GROWTH * length(l.across)).to_bits(),
        n.to_bits(),
        n.to_bits(),
        l.t_max.to_bits(),
        0,
        0,
        0,
        0,
        origin[0] as u32,
        origin[1] as u32,
        origin[2] as u32,
        0,
    ];
    let got = render_raster(
        gpu,
        &view,
        (&gpu.read(&draws)[..cnt as usize], &cache),
        [size; 2],
    );
    let (mut kept, mut entered, mut edges, mut wrong) = (0u64, 0u64, 0u64, 0u64);
    for j in 0..size {
        for i in 0..size {
            let id = got[(j * size + i) as usize];
            if id == 0 {
                continue;
            }
            kept += 1;
            let e = node_cache::entry_at(raster::voxel_of(id)) as usize;
            let node = entry_node(&cache, e, origin);
            let asset = cache[e + node_cache::W_ASSET as usize];
            let (lo, hi) = cast::asset_box(origin, &node, asset);
            let o = [0, 1, 2].map(|a| i as f32 * l.across[a] + j as f32 * l.down[a]);
            let enters = |grow: f32| {
                let (mut t0, mut t1) = (0.0f32, l.t_max);
                for a in 0..3 {
                    let room = grow * (hi[a] - lo[a]).abs().max(o[a].abs()).max(1.0);
                    let (lo, hi) = (lo[a] - room - o[a], hi[a] + room - o[a]);
                    let v = l.dir[a];
                    if v == 0.0 {
                        if !(lo <= 0.0 && 0.0 < hi) {
                            return false;
                        }
                        continue;
                    }
                    let (a0, a1) = if v > 0.0 {
                        (lo / v, hi / v)
                    } else {
                        (hi / v, lo / v)
                    };
                    t0 = t0.max(a0);
                    t1 = t1.min(a1);
                }
                t1 > t0
            };
            if enters(0.0) {
                entered += 1;
            } else if enters(1e-4) {
                edges += 1;
            } else {
                wrong += 1;
                if wrong <= 3 {
                    eprintln!(
                        "DIFFERS {}: texel {i}, {j} keeps slot {}, which its ray does not enter",
                        obs.name,
                        raster::voxel_of(id)
                    );
                }
            }
        }
    }
    kinds.count("the light's raster, texels a box's", entered);
    if kept == 0 || wrong != 0 || !edges_within(edges, kept) {
        failed.push(format!(
            "{}: the light's raster's {kept} texels kept, {wrong} by a box their ray does not \
             enter, {edges} at an edge",
            obs.name
        ));
    }
}

/// The eye's samples' stops by hand over `words` from `origin`, into the node cache `was` left
/// with each node a sample stopped on kept as its walk would keep it (`lay_kept`), in frame
/// `frame`, then its draw, against `boxes_want` and pxl-engine `record`: each such entry's
/// state whole where its ray stopped coarser than it asked, its faces, entered and asked,
/// those `was` held and those its samples entered across, marked drawn in the frame where
/// drawn; each box written a drawable entry's, word for word, `STALE` where it is not fresh
/// (`record::fresh`); every voxel in view (`record::in_view`) drawn; and the frame the draw ran
/// named (`node_cache::DRAWN`). The node cache after. Fails into `failed`.
#[expect(
    clippy::too_many_arguments,
    reason = "the device, the passes, its observer, words, origin and grid, what stood, what to tell and what failed"
)]
fn record_case(
    gpu: &Gpu,
    [pass, draw]: [&wgpu::ComputePipeline; 2],
    obs: &Observed,
    words: &[u32],
    (origin, grid): ([i32; 3], &Grid),
    (was, frame): (Option<&[u32]>, u32),
    kinds: &mut Kinds,
    failed: &mut Vec<String>,
) -> Vec<u32> {
    use pxl_render::frame::shader::word::SQUARE_SIDE;
    let name = &obs.name;
    let want = boxes_want(obs, words, origin, Some(grid.tan), kinds);
    let asked = raster::ASKED_AT as usize;
    let mut laid_args = vec![0u32; asked + raster::ARGS_WORDS as usize];
    laid_args[raster::ARGS_VERTICES as usize] = raster::VERTICES;
    laid_args[asked + raster::ARGS_FACES as usize + 1] = 1;
    laid_args[asked + raster::ARGS_FACES as usize + 2] = 1;
    laid_args[asked + raster::ARGS_ASKS as usize] = grid.asks_at;
    let mut rec = was.map_or_else(frame_table, <[u32]>::to_vec);
    rec[node_cache::FRAME as usize] = frame;
    // Each node a taken sample stopped on kept, as the walk that stopped it keeps it.
    for s in obs.taken() {
        let k = obs.sample_at(s) as usize;
        let node = k + beam::SAMPLE_NODE as usize;
        if words[node + beam::NODE_ZOOM as usize] == beam::SAMPLE_NONE {
            continue;
        }
        let n = node_of(&key_words(words, node));
        let lacks = words[k + beam::SAMPLE_LACKS as usize] != 0;
        let asset = [
            words[k + beam::SAMPLE_ASSET as usize],
            words[k + beam::SAMPLE_ASSET as usize + 1],
        ];
        let kind = if lacks { Kind::Lacks } else { Kind::Cell };
        lay_kept(&mut rec, origin, &n, (kind, asset));
    }
    let eye_buf = gpu.buffer(&obs.eye, wgpu::BufferUsages::UNIFORM);
    let beams = gpu.buffer(words, store_of());
    let args = gpu.buffer(&laid_args, store_of());
    let rec_buf = gpu.buffer(&rec, store_of());
    let boxes = gpu.buffer(&vec![UNWRITTEN; record::EYE_DRAWS as usize], store_of());
    let groups = grid.samples.map(|n| n.div_ceil(SQUARE_SIDE));
    gpu.dispatch(
        pass,
        &[
            (0, eye_buf.as_entire_binding()),
            (8, beams.as_entire_binding()),
            (9, rec_buf.as_entire_binding()),
            (16, args.as_entire_binding()),
        ],
        [groups[0], groups[1], 1],
    );
    // The light's head saying no receiver stands this frame, over its list, args and stood
    // list: the draw stands none, so none of them is written (tests/light.rs holds what it
    // stands where they do).
    let size = shadow::SIZE;
    let mut light_head = vec![0u32; light::HEAD_WORDS];
    for (k, v) in light::head_layout(size) {
        light_head[k] = v;
    }
    light_head[light::HEAD_STANDS_WORLD] = 0;
    let light_list: Vec<u32> = (0..light::list_words(size)).map(|k| k ^ 0x5a5a).collect();
    let light_args: Vec<u32> = (0..light::args_words(size)).map(|k| k ^ 0xa5a5).collect();
    let stood: Vec<u32> = (0..light::STOOD_WORDS).map(|k| k ^ 0x3c3c).collect();
    let light_head_buf = gpu.buffer(&light_head, wgpu::BufferUsages::UNIFORM);
    let light_list_buf = gpu.buffer(&light_list, store_of());
    let light_args_buf = gpu.buffer(&light_args, store_of());
    let stood_buf = gpu.buffer(&stood, store_of());
    gpu.dispatch(
        draw,
        &[
            (0, eye_buf.as_entire_binding()),
            (9, rec_buf.as_entire_binding()),
            (11, light_head_buf.as_entire_binding()),
            (12, light_list_buf.as_entire_binding()),
            (14, boxes.as_entire_binding()),
            (16, args.as_entire_binding()),
            (21, light_args_buf.as_entire_binding()),
            (22, stood_buf.as_entire_binding()),
        ],
        [node_cache::ENTRIES / record::DRAW_LANES, 1, 1],
    );
    let got = gpu.read(&rec_buf);
    let got_boxes = gpu.read(&boxes);
    let got_args = gpu.read(&args);
    let mut fail = |what: String| failed.push(format!("{name}: {what}"));
    if gpu.read(&light_list_buf) != light_list
        || gpu.read(&light_args_buf) != light_args
        || gpu.read(&stood_buf) != stood
    {
        fail("the light's words written where no receiver stands".into());
    }
    if got[node_cache::DRAWN as usize] != frame {
        fail("the draw's frame not named".into());
    }

    let f = |k: usize| f32::from_bits(obs.eye[k]);
    let v3 = |k: usize| [f(k), f(k + 1), f(k + 2)];
    let axes = [v3(eye::RIGHT), v3(eye::UP), v3(eye::FORWARD)];
    let tan = [f(eye::TAN), f(eye::TAN + 1)];
    let range = [obs.eye[eye::RANGE], obs.eye[eye::RANGE + 1]];
    let far = f(eye::FAR);
    // Each drawable slot by its key, as `raster::key` has it from this origin.
    let key_of = |e: usize| -> Key { node_words(&entry_node(&got, e, origin)) };
    let mut held: BTreeMap<Key, Vec<u32>> = BTreeMap::new();
    for slot in 0..node_cache::ENTRIES {
        let e = node_cache::entry_at(slot) as usize;
        if entry_held(&got, e) && got[e + node_cache::W_STATE as usize] != 0 {
            held.entry(key_of(e)).or_default().push(slot);
        }
    }
    let (mut twice, mut entries_differ, mut none) = (0u64, 0u64, 0u64);
    for (key, w) in &want {
        let Some(slots) = held.get(key) else {
            none += 1;
            continue;
        };
        twice += slots.len() as u64 - 1;
        // One slot a key.
        let e = node_cache::entry_at(slots[0]) as usize;
        let w_at = |a: u32| got[e + a as usize];
        let node = node_of(key);
        let whole = record::stopped_whole(origin, &node, range);
        // What the slot held before, where it held this node.
        let state_was = was
            .filter(|r| (0..node_cache::W_STATE as usize).all(|a| r[e + a] == got[e + a]))
            .map_or(0, |r| r[e + node_cache::W_STATE as usize]);
        // The eye's samples ask about every face they enter (`record::ASKED`).
        let faces = w[BOX_FACES as usize] & record::FACES;
        let state = state_was
            | faces
            | faces << record::ASKED_SHIFT
            | if whole { record::WHOLE } else { 0 };
        kinds.add("voxels, stopped whole", whole);
        let alike = w_at(node_cache::W_STATE) == state
            && node_cache::bucket_of(node.zoom, node.idx(origin))
                == slots[0] & !(node_cache::BUCKET - 1);
        if !alike {
            entries_differ += 1;
            if entries_differ <= 3 {
                eprintln!(
                    "DIFFERS {name}: entry {} state {:x}, want state {state:x}",
                    slots[0],
                    w_at(node_cache::W_STATE),
                );
            }
        }
    }
    // Each box a held voxel's, word for word.
    let cnt = got_args[raster::ARGS_INSTANCES as usize];
    let (mut same, mut differ, mut stale) = (0u64, 0u64, 0u64);
    let mut drawn = BTreeSet::new();
    for (place, &b) in got_boxes.iter().enumerate().take(cnt as usize) {
        let slot = raster::voxel_of(b);
        let e = node_cache::entry_at(slot) as usize;
        let key = key_of(e);
        let Some(w) = want.get(&key) else {
            differ += 1;
            continue;
        };
        drawn.insert(key);
        if got[e + node_cache::W_SEEN as usize] != frame {
            differ += 1;
            eprintln!("DIFFERS {name}: box {place}'s entry not marked drawn");
        }
        let state = got[e + node_cache::W_STATE as usize];
        let fresh = record::fresh(origin, &node_of(&key), state & record::WHOLE != 0, range);
        stale += u64::from(!fresh);
        let want_word = raster::draw_word(
            slot,
            w[BOX_ID as usize] & raster::LACK != 0,
            w[BOX_FACES as usize] & raster::EYED != 0,
            fresh,
        );
        if b == want_word {
            same += 1;
        } else {
            differ += 1;
            if differ <= 3 {
                eprintln!("DIFFERS {name}: box {place} {b:x}, want {want_word:x}");
            }
        }
    }
    // Every voxel in view drawn, but a stale one about the eye; one at the edge of the view as
    // the device rounds it told.
    let fresh_of = |key: &Key| {
        let state = held.get(key).map_or(0, |slots| {
            got[(node_cache::entry_at(slots[0]) + node_cache::W_STATE) as usize]
        });
        record::fresh(origin, &node_of(key), state & record::WHOLE != 0, range)
    };
    let mut missed = 0u64;
    for (key, w) in &want {
        if drawn.contains(key) || !held.contains_key(key) {
            continue;
        }
        let b = |k: u32| f32::from_bits(w[k as usize]);
        let lo = [0, 1, 2].map(|a| b(BOX_LO + a));
        let hi = [0, 1, 2].map(|a| b(BOX_HI + a));
        let grown = [tan[0] * 1.001, tan[1] * 1.001];
        let eyed = w[BOX_FACES as usize] & raster::EYED != 0;
        if eyed && !fresh_of(key) {
            kinds.add("boxes, eyed, stale, not drawn", true);
            continue;
        }
        if eyed || record::in_view((lo, hi), axes, tan, far) {
            missed += 1;
        } else {
            kinds.add(
                "voxels out of view",
                !record::in_view((lo, hi), axes, grown, far),
            );
        }
    }
    eprintln!(
        "{name}: {} voxels, {none} not held, {twice} held twice, {entries_differ} entries \
         differ; {cnt} boxes, {same} alike, {differ} differ, {stale} stale, {missed} in view \
         not drawn",
        want.len()
    );
    kinds.count("boxes, stale", stale);
    if none != 0 || twice != 0 || entries_differ != 0 || differ != 0 || missed != 0 {
        fail(format!(
            "{none} voxels not held, {twice} held twice, {entries_differ} entries differ, \
             {differ} boxes differ, {missed} in view not drawn"
        ));
    }
    if same == 0 {
        fail("no box to compare".to_string());
    }
    let cnt = cnt.min(record::EYE_DRAWS);
    raster_drawn(
        gpu,
        obs,
        origin,
        (&got, &got_boxes[..cnt as usize]),
        kinds,
        failed,
    );
    got
}

/// The raster's draw (raster.wgsl) of `draws` over the node cache `cache` in the view `view`
/// (`word::RASTER_FLOATS`), onto a target of `size` cleared to none, the nearest kept: each
/// texel's voxel's number (`raster::id_of`), 0 where it kept none.
fn render_raster(
    gpu: &Gpu,
    view: &[u32],
    (draws, cache): (&[u32], &[u32]),
    size: [u32; 2],
) -> Vec<u32> {
    use pxl_render::frame::shader::record::SHADER_RASTER;
    let device = &gpu.device;
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("raster"),
        source: wgpu::ShaderSource::Wgsl(src(SHADER_RASTER).into()),
    });
    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("raster"),
        layout: None,
        vertex: wgpu::VertexState {
            module: &module,
            entry_point: Some("vs"),
            compilation_options: Default::default(),
            buffers: &[],
        },
        primitive: Default::default(),
        depth_stencil: Some(wgpu::DepthStencilState {
            format: wgpu::TextureFormat::Depth32Float,
            depth_write_enabled: Some(true),
            depth_compare: Some(wgpu::CompareFunction::Greater),
            stencil: Default::default(),
            bias: Default::default(),
        }),
        multisample: Default::default(),
        fragment: Some(wgpu::FragmentState {
            module: &module,
            entry_point: Some("fs"),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::TextureFormat::R32Uint.into())],
        }),
        multiview_mask: None,
        cache: None,
    });
    let view_buf = gpu.buffer(view, wgpu::BufferUsages::UNIFORM);
    let draws_buf = gpu.buffer(draws, wgpu::BufferUsages::STORAGE);
    let cache_buf = gpu.buffer(cache, wgpu::BufferUsages::STORAGE);
    let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout: &pipeline.get_bind_group_layout(0),
        entries: &[
            wgpu::BindGroupEntry {
                binding: 1,
                resource: view_buf.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: draws_buf.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 9,
                resource: cache_buf.as_entire_binding(),
            },
        ],
    });
    let target = texture(
        device,
        &gpu.queue,
        size,
        1,
        wgpu::TextureUsages::RENDER_ATTACHMENT,
        &vec![0; (size[0] * size[1]) as usize],
    );
    let depth = device.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size: target.size(),
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Depth32Float,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let (target_view, depth_view) = (
        target.create_view(&Default::default()),
        depth.create_view(&Default::default()),
    );
    let mut e = device.create_command_encoder(&Default::default());
    {
        let mut pass = e.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: None,
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &target_view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &depth_view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(0.0),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            ..Default::default()
        });
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &group, &[]);
        pass.draw(0..raster::VERTICES, 0..draws.len() as u32);
    }
    gpu.queue.submit([e.finish()]);
    read_texture(device, &gpu.queue, &target)
}

/// The raster's draw over `draws`, the record's words a box, and the node cache `cache` it reads
/// each box's entry off (raster.wgsl), from the eye of `obs` at `origin`, onto a screen of its
/// own: every pixel that keeps a voxel keeps one whose box, as the cast stops on its node
/// (`cast::entry`), the pixel's own ray enters, but where the device's rounding may decide it
/// otherwise (`at_edge`); and some pixel keeps one.
fn raster_drawn(
    gpu: &Gpu,
    obs: &Observed,
    origin: [i32; 3],
    (cache, draws): (&[u32], &[u32]),
    kinds: &mut Kinds,
    failed: &mut Vec<String>,
) {
    use pxl_engine::param::raster::GROWTH;
    use pxl_render::frame::shader::word::{BOX_VERTICES, RASTER_FLOATS};
    let screen = Screen {
        name: "the raster's",
        size: [96, 64],
        jitter: [0.0; 2],
        marked_in: 0,
    };
    let f = |k: usize| f32::from_bits(obs.eye[k]);
    let v3 = |k: usize| [f(k), f(k + 1), f(k + 2)];
    let (r, u, fw) = (v3(eye::RIGHT), v3(eye::UP), v3(eye::FORWARD));
    let tan = [f(eye::TAN), f(eye::TAN + 1)];
    let [w, h] = screen.size.map(|v| v as f32);
    let px = (2.0 * tan[0] / w).max(2.0 * tan[1] / h);
    let turn = BOX_VERTICES * u32::from(raster::is_flipped(r, u, fw));
    let view: [u32; RASTER_FLOATS] = [
        r[0].to_bits(),
        r[1].to_bits(),
        r[2].to_bits(),
        tan[0].to_bits(),
        u[0].to_bits(),
        u[1].to_bits(),
        u[2].to_bits(),
        tan[1].to_bits(),
        fw[0].to_bits(),
        fw[1].to_bits(),
        fw[2].to_bits(),
        raster::NEAR.to_bits(),
        turn,
        0,
        0,
        0,
        (GROWTH * px).to_bits(),
        w.to_bits(),
        h.to_bits(),
        obs.eye[eye::FAR],
        0,
        0,
        0,
        0,
        origin[0] as u32,
        origin[1] as u32,
        origin[2] as u32,
        0,
    ];
    let size = screen.size;
    let got = render_raster(gpu, &view, (draws, cache), size);
    let (mut kept, mut entered, mut edges, mut wrong) = (0u64, 0u64, 0u64, 0u64);
    for y in 0..size[1] {
        for x in 0..size[0] {
            let id = got[(y * size[0] + x) as usize];
            if id == 0 {
                continue;
            }
            kept += 1;
            let e = node_cache::entry_at(raster::voxel_of(id)) as usize;
            let node = entry_node(cache, e, origin);
            let asset = cache[e + node_cache::W_ASSET as usize];
            let (ray, _) = px_aim(obs, &screen, [x, y]);
            let at = cast::entry_with(origin, &ray, ray.inverse(), &node, asset);
            if at.enters {
                entered += 1;
            } else if at_edge(&at, ray.dir) {
                edges += 1;
            } else {
                wrong += 1;
                if wrong <= 3 {
                    eprintln!(
                        "DIFFERS {}: pixel {x}, {y} keeps slot {}, which its ray does not enter",
                        obs.name,
                        raster::voxel_of(id)
                    );
                }
            }
        }
    }
    kinds.count("raster, pixels a box's", entered);
    if kept == 0 || wrong != 0 || !edges_within(edges, kept) {
        failed.push(format!(
            "{}: the raster's {kept} pixels kept, {wrong} by a box their ray does not enter, \
             {edges} at an edge",
            obs.name
        ));
    }
}

/// Each sample alike the one across before it, where its node's corner lies on the grid a zoom
/// coarser, its node taken a zoom coarser at the same corner, one row in eight: the words a pixel
/// reads alike but the zoom. How many.
fn coarser(obs: &Observed, words: &mut [u32]) -> u64 {
    let read = beam::SAMPLE_WORDS as usize;
    let mut cut = 0;
    for j in (0..obs.lattice[1]).step_by(8) {
        for i in 1..obs.lattice[0] {
            let (k, b) = (
                obs.sample_at([i, j]) as usize,
                obs.sample_at([i - 1, j]) as usize,
            );
            let z = k + (beam::SAMPLE_NODE + beam::NODE_ZOOM) as usize;
            let zoom = words[z];
            if zoom == beam::SAMPLE_NONE || zoom == 0 || words[k..k + read] != words[b..b + read] {
                continue;
            }
            let keep = !(u32::MAX >> (zoom - 1));
            let c = k + (beam::SAMPLE_NODE + beam::NODE_CORNER) as usize;
            if (0..3).all(|a| words[c + a] & !keep == 0) {
                words[z] = zoom - 1;
                cut += 1;
            }
        }
    }
    cut
}

/// The table a frame's beam passes start over.
fn frame_table() -> Vec<u32> {
    let mut t = vec![0u32; node_cache::WORDS as usize];
    t[node_cache::FRAME as usize] = FRAME;
    t
}

#[test]
fn hand_eye_record_answers_as_raster() {
    use pxl_render::frame::shader::record::{SHADER_RECORD_DRAW, SHADER_SAMPLE_BOX};
    let Some(gpu) = device() else { return };
    let built = real_world::build(EYE);
    let b = world_bufs(&gpu, &built);
    let at = spots(&built);
    let none = [Cut::NONE; 2];
    let over = |g: [i32; 2], up: i32| q8([g[0], g[1], built.top_at(g) + up]);
    let case = |name, origin, look, tan, lattice, far, range| EyeCase {
        name,
        origin,
        look,
        tan,
        lattice,
        far,
        range,
        cuts: none,
    };
    let mut cases = vec![
        case(
            "the eye, over the ground",
            over([EYE[0] + 90, EYE[1] + 90], 60),
            [0.5, 0.4, -0.77],
            [0.3, 0.2],
            1,
            2000.0,
            [8, FINEST],
        ),
        case(
            "the eye, lattice 2, over the seam",
            over([EYE[0] + 280, EYE[1] + 90], 40),
            [0.8, 0.2, -0.55],
            [0.4, 0.3],
            2,
            2000.0,
            [8, FINEST],
        ),
        case(
            "the eye, on a bridge",
            q8([at.bridge[0], at.bridge[1], at.bridge[2] + 12]),
            [0.5, -0.3, -0.8],
            [0.3, 0.3],
            1,
            600.0,
            [LEAF_CELL, FINEST],
        ),
        case(
            "the eye, down a bore, under the ground",
            q8([at.bore[0], at.bore[1], at.bore[2]]),
            [0.6, 0.2, -0.3],
            [0.3, 0.3],
            1,
            300.0,
            [LEAF_CELL, FINEST],
        ),
        case(
            "the eye, where no clip level holds the ground",
            q8([1 << 15, 1 << 15, 40]),
            [0.3, 0.2, -0.93],
            [0.4, 0.3],
            1,
            800.0,
            [8, FINEST],
        ),
        case(
            "the eye, high over the clip levels",
            over([EYE[0] + 2000, EYE[1] + 2000], 2500),
            [0.3, -0.2, -0.9],
            [0.3, 0.25],
            1,
            20000.0,
            [4, FINEST],
        ),
    ];
    cases.push(EyeCase {
        name: "the eye, on what is placed, cut and hid",
        cuts: [
            Cut {
                zoom: 22,
                at: [at.leaf_c[0] >> 3, at.leaf_c[1] >> 3, at.leaf_c[2] >> 3],
            },
            Cut {
                zoom: at.leaf_zoom,
                at: at.leaf_c,
            },
        ],
        ..case(
            "",
            over(at.on_placed, 20),
            [0.02, 0.03, -1.0],
            [0.3, 0.4],
            1,
            400.0,
            [LEAF_CELL, FINEST],
        )
    });
    let pass = gpu.pipeline(src(SHADER_SAMPLE_BOX), "sample_box_cs");
    let draw = gpu.pipeline(src(SHADER_RECORD_DRAW), "record_draw_cs");
    let mut kinds = Kinds::new();
    let mut failed = Vec::new();
    // `coarse`: some samples' nodes taken a zoom coarser (`coarser`).
    let mut run = |b: &WorldBufs, c: &EyeCase, coarse: bool, kinds: &mut Kinds| {
        let mut obs = eye_of(c, [0; 3]);
        let table = gpu.buffer(&frame_table(), store_of());
        let lines = eye_beams(&gpu, b, &obs, &table);
        let table = gpu.read(&table);
        let mut words = stopped(&gpu, b, &obs, &lines, None, &table);
        let Rays::Eye { origin, grid, .. } = &obs.rays else {
            unreachable!()
        };
        let (origin, grid) = (*origin, *grid);
        if coarse {
            kinds.count("alike but the zoom", coarser(&obs, &mut words));
            obs.name += ", a sample's node coarser at its corner";
        }
        // A frame into an empty record, and the next over what it kept: the same voxels
        // found, none written again.
        let passes = [&pass, &draw];
        let kept = record_case(
            &gpu,
            passes,
            &obs,
            &words,
            (origin, &grid),
            (None, FRAME),
            kinds,
            &mut failed,
        );
        obs.name += ", the next frame";
        record_case(
            &gpu,
            passes,
            &obs,
            &words,
            (origin, &grid),
            (Some(&kept), FRAME + 1),
            kinds,
            &mut failed,
        );
        // Two Q3 steps over the middle of the top of the nearest cell it drew, and under the
        // middle of its bottom: within `raster::is_eyed`'s reach at the screen's corner, and
        // past it straight ahead.
        let boxes = boxes_want(&obs, &words, origin, None, &mut Kinds::new());
        let q = (1 << FRAC_W) as f32;
        boxes
            .values()
            .filter(|w| w[BOX_ID as usize] & raster::LACK == 0)
            .map(|w| {
                let f = |k: u32| f32::from_bits(w[k as usize]);
                let mid = |a: u32| (f(BOX_LO + a) + f(BOX_HI + a)) / 2.0;
                let top = f(BOX_HI + 2);
                let near = mid(0) * mid(0) + mid(1) * mid(1) + top * top;
                let step = |v: f32| (v * q).round_ties_even() as i32;
                let at = |z: f32, by: i32| {
                    [
                        origin[0].wrapping_add(step(mid(0))),
                        origin[1] + step(mid(1)),
                        origin[2] + step(z) + by,
                    ]
                };
                (near, [at(f(BOX_HI + 2), 2), at(f(BOX_LO + 2), -2)])
            })
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|m| m.1)
    };
    for c in &cases {
        run(&b, c, false, &mut kinds);
    }
    run(&b, &cases[0], true, &mut kinds);
    // The eye by a cell of the bridge it saw, looking at it: its box about the eye.
    let [over_top, under] = run(&b, &cases[2], false, &mut Kinds::new()).expect("a cell drawn");
    for (name, origin, look) in [
        (
            "the eye, two steps over a cell's top",
            over_top,
            [0.1, 0.05, -1.0],
        ),
        (
            "the eye, two steps under a cell's bottom",
            under,
            [0.1, 0.05, 1.0],
        ),
    ] {
        let c = case(
            name,
            origin,
            look,
            [0.5, 0.4],
            1,
            300.0,
            [LEAF_CELL, FINEST],
        );
        run(&b, &c, false, &mut kinds);
    }
    // Over X = 0, where its roots part: boxes in the next root.
    let zero = real_world::build(EYE_ZERO);
    let bz = world_bufs(&gpu, &zero);
    let c = case(
        "the eye, looking over X = 0",
        q8([
            150,
            EYE_ZERO[1] + 90,
            zero.top_at([150, EYE_ZERO[1] + 90]) + 100,
        ]),
        [-0.9, 0.1, -0.4],
        [0.3, 0.2],
        1,
        2000.0,
        [8, FINEST],
    );
    run(&bz, &c, false, &mut kinds);
    eprintln!("  KINDS eye boxes {kinds:?}");
    for what in [
        "boxes",
        "boxes, eyed",
        "boxes, eyed, a cell",
        "boxes, eyed, past the eye",
        "boxes, eyed, past the corner's share and half the reach",
        "alike but the zoom",
        "boxes, lacks",
        "boxes, short of its cube",
        "boxes, in another root",
        "a voxel another sample drew",
        "a voxel another sample drew, through another side",
        "past its reach, no box",
    ] {
        if kinds.get(what) == 0 {
            failed.push(format!("none of {what} to say"));
        }
    }
    if kinds.get("boxes, an inset's product rounded") != 0 {
        failed
            .push("an inset's product rounded, which a fused sum may round otherwise".to_string());
    }
    assert!(failed.is_empty(), "{}", failed.join("\n"));
}

/// A voxel the record holds, its fill cube about an eye that has since come into it: the
/// record a cast from over the ground wrote, then the draw alone from the middle of each
/// coarse voxel's box, asking finer there than the voxel, so it is stale (`record::fresh`) and
/// about the eye (`raster::is_eyed`). The draw writes no box for it: a stale box about the
/// eye would be drawn over the whole screen, before every other stale box, and every pixel no
/// fresh box covers would take it (pxl-engine `record`, Fresh or stale).
#[test]
fn hand_eye_stale_box_about_the_eye_undrawn() {
    use pxl_render::frame::shader::record::{SHADER_RECORD_DRAW, SHADER_SAMPLE_BOX};
    let Some(gpu) = device() else { return };
    let built = real_world::build(EYE);
    let b = world_bufs(&gpu, &built);
    let over = |g: [i32; 2], up: i32| q8([g[0], g[1], built.top_at(g) + up]);
    let case = |name, origin| EyeCase {
        name,
        origin,
        look: [0.5, 0.4, -0.77],
        tan: [0.3, 0.2],
        lattice: 1,
        far: 2000.0,
        range: [8, FINEST],
        cuts: [Cut::NONE; 2],
    };
    let pass = gpu.pipeline(src(SHADER_SAMPLE_BOX), "sample_box_cs");
    let draw = gpu.pipeline(src(SHADER_RECORD_DRAW), "record_draw_cs");
    let mut kinds = Kinds::new();
    let mut failed = Vec::new();
    let far_eye = case(
        "the eye, over the ground",
        over([EYE[0] + 90, EYE[1] + 90], 60),
    );
    let obs = eye_of(&far_eye, [0; 3]);
    let table = gpu.buffer(&frame_table(), store_of());
    let lines = eye_beams(&gpu, &b, &obs, &table);
    let table = gpu.read(&table);
    let words = stopped(&gpu, &b, &obs, &lines, None, &table);
    let Rays::Eye { origin, grid, .. } = &obs.rays else {
        unreachable!()
    };
    let kept = record_case(
        &gpu,
        [&pass, &draw],
        &obs,
        &words,
        (*origin, grid),
        (None, FRAME),
        &mut kinds,
        &mut failed,
    );
    assert!(failed.is_empty(), "{}", failed.join("\n"));

    // The coarsest voxels that keep a share of their cube, each an eye come into its box.
    let q = (1 << FRAC_W) as f32;
    let mut inside: Vec<(u32, [i32; 3])> = (0..node_cache::ENTRIES)
        .filter_map(|slot| {
            let e = node_cache::entry_at(slot) as usize;
            let state = kept[e + node_cache::W_STATE as usize];
            let kind = kept[e + node_cache::W_KIND as usize] & node_cache::KIND_MASK;
            if !entry_held(&kept, e)
                || state == 0
                || kind != Kind::Cell as u32
                || state & record::WHOLE != 0
            {
                return None;
            }
            let node = entry_node(&kept, e, *origin);
            if node.zoom + 2 > FINEST {
                return None;
            }
            let asset = kept[e + node_cache::W_ASSET as usize];
            let (lo, hi) = cast::asset_box(*origin, &node, asset);
            let mid = |a: usize| ((lo[a] + hi[a]) / 2.0 * q).round_ties_even() as i32;
            Some((
                node.zoom,
                [
                    origin[0].wrapping_add(mid(0)),
                    origin[1] + mid(1),
                    origin[2] + mid(2),
                ],
            ))
            .map(|(z, at)| (z * node_cache::ENTRIES + slot, at))
        })
        .collect();
    inside.sort_unstable();
    let mut tried = 0u32;
    for &(k, at) in inside.iter().take(8) {
        let slot = k % node_cache::ENTRIES;
        let near = eye_of(&case("the eye, come into a voxel's box", at), [0; 3]);
        let e = node_cache::entry_at(slot) as usize;
        let node = entry_node(&kept, e, at);
        let asset = kept[e + node_cache::W_ASSET as usize];
        let (lo, hi) = cast::asset_box(at, &node, asset);
        if record::fresh(at, &node, false, far_eye.range) || !raster::is_eyed(lo, hi, far_eye.tan) {
            continue;
        }
        tried += 1;
        let mut rec = kept.clone();
        rec[node_cache::FRAME as usize] = FRAME + 1;
        let asked = raster::ASKED_AT as usize;
        let mut laid_args = vec![0u32; asked + raster::ARGS_WORDS as usize];
        laid_args[raster::ARGS_VERTICES as usize] = raster::VERTICES;
        let eye_buf = gpu.buffer(&near.eye, wgpu::BufferUsages::UNIFORM);
        let args = gpu.buffer(&laid_args, store_of());
        let rec_buf = gpu.buffer(&rec, store_of());
        let boxes = gpu.buffer(&vec![UNWRITTEN; record::EYE_DRAWS as usize], store_of());
        let size = shadow::SIZE;
        let mut light_head = vec![0u32; light::HEAD_WORDS];
        for (k, v) in light::head_layout(size) {
            light_head[k] = v;
        }
        light_head[light::HEAD_STANDS_WORLD] = 0;
        let light_head_buf = gpu.buffer(&light_head, wgpu::BufferUsages::UNIFORM);
        let light_list_buf = gpu.buffer(&vec![0u32; light::list_words(size) as usize], store_of());
        let light_args_buf = gpu.buffer(&vec![0u32; light::args_words(size) as usize], store_of());
        let stood_buf = gpu.buffer(&vec![0u32; light::STOOD_WORDS as usize], store_of());
        gpu.dispatch(
            &draw,
            &[
                (0, eye_buf.as_entire_binding()),
                (11, light_head_buf.as_entire_binding()),
                (12, light_list_buf.as_entire_binding()),
                (14, boxes.as_entire_binding()),
                (16, args.as_entire_binding()),
                (21, light_args_buf.as_entire_binding()),
                (22, stood_buf.as_entire_binding()),
                (9, rec_buf.as_entire_binding()),
            ],
            [node_cache::ENTRIES / record::DRAW_LANES, 1, 1],
        );
        let got_args = gpu.read(&args);
        let got = gpu.read(&boxes);
        let cnt = got_args[raster::ARGS_CNT as usize].min(record::EYE_DRAWS) as usize;
        let id = raster::id_of(slot, false);
        if let Some(b) = (0..cnt).find(|&b| got[b] & raster::ID_MASK & !raster::LACK == id) {
            let word = got[b];
            failed.push(format!(
                "slot {slot}, zoom {}: a stale box about the eye drawn, its word {word:#x}",
                node.zoom
            ));
        }
    }
    eprintln!("stale voxels about the eye tried: {tried}");
    assert!(tried > 0, "no voxel's box held the eye stale");
    assert!(failed.is_empty(), "{}", failed.join("\n"));
}

#[test]
fn hand_light_record_answers_as_raster() {
    use pxl_render::frame::shader::record::SHADER_LIGHT_SAMPLE_BOX;
    let Some(gpu) = device() else { return };
    let built = real_world::build(EYE);
    let b = world_bufs(&gpu, &built);
    let none = [Cut::NONE; 2];
    let over = |g: [i32; 2], up: i32| q8([g[0], g[1], built.top_at(g) + up]);
    let about = LightCase {
        name: "the light, about the eye",
        origin: over([EYE[0] + 90, EYE[1] + 90], 40),
        dir: [0.3, 0.2, -0.93],
        side: 1.0,
        back: 60.0,
        t_max: 400.0,
        range: [8, FINEST],
        corner: [1304, -896],
        cuts: none,
        size: SIZE,
        listed: None,
    };
    let cases = [
        LightCase { ..about },
        LightCase {
            name: "the light, its tiles listed",
            corner: [1304, -896],
            listed: Some(3),
            ..about
        },
        LightCase {
            name: "the light, every tile listed, the list's row full",
            corner: [1304, -896],
            listed: Some(1),
            ..about
        },
        LightCase {
            name: "the light, its tiles listed, the shipped map's size",
            side: 0.5,
            corner: [1304, -896],
            size: shadow::SIZE,
            listed: Some(23),
            ..about
        },
        LightCase {
            name: "the light, where no clip level holds the ground",
            origin: q8([1 << 15, 1 << 15, 40]),
            side: 8.0,
            back: 100.0,
            t_max: 800.0,
            corner: [0, 0],
            ..about
        },
        LightCase {
            name: "the light, wide, over the clip levels",
            origin: over([EYE[0] + 3000, EYE[1] + 3000], 900),
            dir: [0.2, -0.3, -0.93],
            side: 32.0,
            back: 200.0,
            t_max: 4000.0,
            corner: [80, 8],
            ..about
        },
    ];
    let pass = gpu.pipeline(src(SHADER_LIGHT_SAMPLE_BOX), "sample_box_cs");
    let mut kinds = Kinds::new();
    let mut failed = Vec::new();
    let mut run = |b: &WorldBufs, c: &LightCase, kinds: &mut Kinds| {
        let mut obs = light_of(c, [0, 0]);
        let table = gpu.buffer(&frame_table(), store_of());
        let lines = light_beams(&gpu, b, &mut obs, &table, c.listed);
        let table = gpu.read(&table);
        // Listed, its samples as the whole cast of the same box left them: a sample not taken
        // keeps what it held.
        let prior = c.listed.map(|_| {
            let whole = LightCase { listed: None, ..*c };
            let mut o = light_of(&whole, [0, 0]);
            let t = gpu.buffer(&frame_table(), store_of());
            let lines = light_beams(&gpu, b, &mut o, &t, None);
            let t = gpu.read(&t);
            stopped(&gpu, b, &o, &lines, None, &t)
        });
        let words = stopped(&gpu, b, &obs, &lines, prior.as_deref(), &table);
        // The rows the list's entries open, as the sample pass's.
        let groups = obs.groups;
        kinds.add("listed", c.listed.is_some());
        // The light's asks none: its slots read no faces' words.
        light_record_case(&gpu, &pass, &obs, &words, groups, FRAME, kinds, &mut failed);
    };
    for c in &cases {
        run(&b, c, &mut kinds);
    }
    let zero = real_world::build(EYE_ZERO);
    let bz = world_bufs(&gpu, &zero);
    let c = LightCase {
        name: "the light, over X = 0",
        origin: q8([
            10,
            EYE_ZERO[1] + 90,
            zero.top_at([10, EYE_ZERO[1] + 90]) + 40,
        ]),
        range: [LEAF_CELL, LEAF_CELL],
        ..about
    };
    run(&bz, &c, &mut kinds);
    eprintln!("  KINDS light record {kinds:?}");
    for what in [
        "voxels",
        "voxels, lacks",
        "voxels, short of its cube",
        "voxels, in another root",
        "listed",
    ] {
        if kinds.get(what) == 0 {
            failed.push(format!("none of {what} to say"));
        }
    }
    if kinds.get("boxes, an inset's product rounded") != 0 {
        failed
            .push("an inset's product rounded, which a fused sum may round otherwise".to_string());
    }
    assert!(failed.is_empty(), "{}", failed.join("\n"));
}

/// The beam rays' run (corners.wgsl over beam.eye.wgsl) over a node cache that holds part of
/// what the beam rays meet: a beam ray its walks over the node cache park goes on from the state
/// it was parked in, and every corner stops where `cast::walk` from the root stops the same ray,
/// at the same node and the same distance handed on (`Grid::hand_dist`). The first frame over
/// an empty cache, the next over that frame's with every other entry finer than `KEPT_ZOOM`
/// dropped.
#[test]
fn hand_beam_rays_answer_as_cast_walk() {
    const KEPT_ZOOM: Zoom = 12;
    let Some(gpu) = device() else { return };
    let built = real_world::build(EYE);
    let b = world_bufs(&gpu, &built);
    let w = built.world();
    let at = spots(&built);
    let none = [Cut::NONE; 2];
    let over = |g: [i32; 2], up: i32| q8([g[0], g[1], built.top_at(g) + up]);
    let cases = [
        EyeCase {
            name: "the eye, over the ground",
            origin: over([EYE[0] + 90, EYE[1] + 90], 60),
            look: [0.5, 0.4, -0.77],
            tan: [0.3, 0.2],
            lattice: 1,
            far: 2000.0,
            range: [8, FINEST],
            cuts: none,
        },
        EyeCase {
            name: "the eye, lattice 2, over the seam",
            origin: over([EYE[0] + 280, EYE[1] + 90], 40),
            look: [0.8, 0.2, -0.55],
            tan: [0.4, 0.3],
            lattice: 2,
            far: 2000.0,
            range: [8, FINEST],
            cuts: none,
        },
        EyeCase {
            name: "the eye, on a bridge",
            origin: q8([at.bridge[0], at.bridge[1], at.bridge[2] + 12]),
            look: [0.5, -0.3, -0.8],
            tan: [0.3, 0.3],
            lattice: 1,
            far: 600.0,
            range: [LEAF_CELL, FINEST],
            cuts: none,
        },
    ];
    let mut failed = Vec::new();
    for c in &cases {
        let obs = eye_of(c, [0; 3]);
        let Rays::Eye {
            origin,
            axes,
            far,
            range,
            grid,
        } = &obs.rays
        else {
            unreachable!()
        };
        let detail = Detail {
            range: *range,
            spacing: Spacing {
                angle: grid.s,
                across: 0.0,
            },
        };
        let [across, down] = grid.corners;
        let want: Vec<Trace<Inheritance>> = (0..across * down)
            .map(|k| {
                let t = grid.corner([k % across, k / across]);
                let ray = Ray::new(cast::through(axes[0], axes[1], axes[2], t[0], t[1]), *far);
                let mut q = Asking { w, cuts: c.cuts };
                cast::walk_from_root(&mut q, *origin, &ray, detail, Until::Beam, &mut ())
            })
            .collect();
        let table = gpu.buffer(&frame_table(), store_of());
        let first = eye_beams(&gpu, &b, &obs, &table);
        // Every other entry finer than `KEPT_ZOOM` dropped: the next frame's walk over the node
        // cache leaves its rays part of the way down.
        let mut held = gpu.read(&table);
        held[node_cache::FRAME as usize] = FRAME + 1;
        for e in (0..node_cache::ENTRIES).step_by(2) {
            let w = (node_cache::ENTRIES_AT + e * node_cache::ENTRY_WORDS) as usize;
            if held[w + node_cache::W_NODE as usize] >= KEPT_ZOOM {
                held[w] = 0;
            }
        }
        let table = gpu.buffer(&held, store_of());
        let next = eye_beams(&gpu, &b, &obs, &table);
        for (what, words) in [("first frame", &first), ("next frame", &next)] {
            let mut differ = 0u32;
            let mut first_differ = None;
            for k in 0..across * down {
                let n = (k * beam::LINE_WORDS + beam::LINE_NODE) as usize;
                let got_node = Node {
                    zoom: words[n + beam::NODE_ZOOM as usize],
                    corner: [0, 1, 2].map(|i| words[n + beam::NODE_CORNER as usize + i]),
                    roots: [0, 1, 2].map(|i| words[n + beam::NODE_ROOTS as usize + i] as i32),
                };
                let got_t = f32::from_bits(words[grid.hand_at(k) as usize]);
                let m = &want[k as usize];
                let want_t = grid.hand_dist(m.hit.t);
                if got_node != m.hit.node || gpu::ulps(got_t, want_t) > 4 {
                    differ += 1;
                    first_differ.get_or_insert((k, got_node, got_t, m.hit.node, want_t));
                }
            }
            eprintln!(
                "{} ({what}): {differ} of {} corners differ",
                c.name,
                across * down
            );
            if differ != 0 {
                failed.push(format!(
                    "{} ({what}): {differ} corners differ, first {first_differ:?}",
                    c.name
                ));
            }
        }
    }
    assert!(failed.is_empty(), "{}", failed.join("\n"));
}

/// What a beam ray's walk handed each node from the root to the one it stopped at (`Keep`).
struct Handed(Vec<Inheritance>);

impl Keep<Inheritance> for Handed {
    fn keep(&mut self, met: &Trace<Inheritance>, stack: &[Inheritance; DEPTH]) {
        let z = met.hit.node.zoom as usize;
        self.0 = stack[..z].iter().copied().chain([met.hit.at]).collect();
    }
}

/// The beam rays' run for the light's map (corners.wgsl over beam.light.wgsl) over a node cache that
/// holds part of what its beam rays meet: every beam ray stops where `cast::walk` from the root
/// stops the same ray from the origin the pass laid, at the same node, hands on the same
/// distance (`light::margin`, under a rounding a device may take, `margins`), and holds in its
/// line what that walk handed the node and every node over it. The first frame over an empty
/// cache, the next over that frame's with every other entry finer than `KEPT_ZOOM` dropped.
#[test]
fn hand_light_beam_rays_answer_as_cast_walk() {
    const KEPT_ZOOM: Zoom = 12;
    let Some(gpu) = device() else { return };
    let built = real_world::build(EYE);
    let b = world_bufs(&gpu, &built);
    let w = built.world();
    let none = [Cut::NONE; 2];
    let over = |g: [i32; 2], up: i32| q8([g[0], g[1], built.top_at(g) + up]);
    let cases = [
        LightCase {
            name: "the light, about the eye",
            origin: over([EYE[0] + 90, EYE[1] + 90], 40),
            dir: [0.3, 0.2, -0.93],
            side: 1.0,
            back: 60.0,
            t_max: 400.0,
            range: [8, FINEST],
            corner: [1304, -896],
            cuts: none,
            size: SIZE,
            listed: None,
        },
        LightCase {
            name: "the light, narrow and slanting",
            origin: over([EYE[0] + 40, EYE[1] - 20], 25),
            dir: [0.6, -0.3, -0.74],
            side: 0.37,
            back: 120.0,
            t_max: 500.0,
            range: [LEAF_CELL, FINEST],
            corner: [-16, 32],
            cuts: none,
            size: SIZE,
            listed: None,
        },
    ];
    let mut failed = Vec::new();
    for c in &cases {
        let mut obs = light_of(c, [0, 0]);
        let table = gpu.buffer(&frame_table(), store_of());
        let first = light_beams(&gpu, &b, &mut obs, &table, None);
        let mut held = gpu.read(&table);
        held[node_cache::FRAME as usize] = FRAME + 1;
        for e in (0..node_cache::ENTRIES).step_by(2) {
            let at = (node_cache::ENTRIES_AT + e * node_cache::ENTRY_WORDS) as usize;
            if held[at + node_cache::W_NODE as usize] >= KEPT_ZOOM {
                held[at] = 0;
            }
        }
        let table = gpu.buffer(&held, store_of());
        let next = light_beams(&gpu, &b, &mut obs, &table, None);
        let Rays::Light {
            light: l,
            range,
            size,
            ..
        } = &obs.rays
        else {
            unreachable!()
        };
        let across = light::beam_across(*size);
        let margins = margins(l.spacing().across);
        for (what, words) in [("first frame", &first), ("next frame", &next)] {
            let mut differ = 0u32;
            let mut first_differ = None;
            for k in 0..across * across {
                let line = light::line(*size, k);
                let at = |a: u32| words[(line + a) as usize];
                let o = [0, 1, 2].map(|a| at(light::LINE_ORIGIN + a) as i32);
                let tile = light::corner_at([k % across, k / across])
                    .map(|v| v.div_euclid(light::TILE as i32));
                let band = l.band(*size, *range, tile);
                let detail = Detail {
                    range: [band, band],
                    spacing: l.spacing(),
                };
                let mut q = Asking { w, cuts: c.cuts };
                let mut handed = Handed(Vec::new());
                let ray = Ray::new(l.dir, l.t_max);
                let m = cast::walk_from_root(&mut q, o, &ray, detail, Until::Beam, &mut handed);
                let n = beam::LINE_NODE;
                let got_node = Node {
                    zoom: at(n + beam::NODE_ZOOM),
                    corner: [0, 1, 2].map(|i| at(n + beam::NODE_CORNER + i)),
                    roots: [0, 1, 2].map(|i| at(n + beam::NODE_ROOTS + i) as i32),
                };
                let got_t = f32::from_bits(words[light::hand_at(*size, k) as usize]);
                let hands = margins.map(|(_, margin)| (m.hit.t - margin).max(0.0));
                let hand_alike = hands.iter().any(|h| h.to_bits() == got_t.to_bits());
                let line_alike = got_node == m.hit.node
                    && handed.0.iter().enumerate().all(|(z, h)| {
                        let g = [0, 1, 2, 3]
                            .map(|i| at(beam::LINE_INHERITANCE + beam::LINE_STEP * z as u32 + i));
                        g == h.words()
                    });
                if !hand_alike || !line_alike {
                    differ += 1;
                    first_differ.get_or_insert((k, got_node, got_t, m.hit.node, hands[0]));
                }
            }
            eprintln!(
                "{} ({what}): {differ} of {} beam rays differ",
                c.name,
                across * across
            );
            if differ != 0 {
                failed.push(format!(
                    "{} ({what}): {differ} beam rays differ, first {first_differ:?}",
                    c.name
                ));
            }
        }
    }
    assert!(failed.is_empty(), "{}", failed.join("\n"));
}

/// A sample whose walk over the node cache meets a node the cache holds only at other zooms is
/// left there (walk.wgsl, cached.wgsl): the samples' run goes on from where it was left, and every
/// sample stops as `cast::walk` from the root stops it. The cache the eye's first frame leaves
/// at coarse zooms, the next frame asking finer, every other entry finer than `KEPT_ZOOM` dropped.
#[test]
fn hand_samples_over_other_zooms_answer_as_cast_walk() {
    const KEPT_ZOOM: Zoom = 12;
    let Some(gpu) = device() else { return };
    let built = real_world::build(EYE);
    let b = world_bufs(&gpu, &built);
    let w = built.world();
    let over = |g: [i32; 2], up: i32| q8([g[0], g[1], built.top_at(g) + up]);
    let case = |range: [Zoom; 2]| EyeCase {
        name: "the eye, over the ground, asked coarse then fine",
        origin: over([EYE[0] + 90, EYE[1] + 90], 60),
        look: [0.5, 0.4, -0.77],
        tan: [0.3, 0.2],
        lattice: 1,
        far: 2000.0,
        range,
        cuts: [Cut::NONE; 2],
    };
    let mut kinds = Kinds::new();
    let mut failed = Vec::new();
    let coarse = eye_of(&case([8, 16]), [0; 3]);
    let table = gpu.buffer(&frame_table(), store_of());
    let words = eye_beams(&gpu, &b, &coarse, &table);
    let after_beam = gpu.read(&table);
    let mut held = run_frame(
        &gpu,
        &b,
        w,
        &coarse,
        &words,
        &after_beam,
        None,
        "coarse",
        &mut kinds,
        &mut failed,
    );
    held[node_cache::FRAME as usize] = FRAME + 1;
    for e in (0..node_cache::ENTRIES).step_by(2) {
        let at = (node_cache::ENTRIES_AT + e * node_cache::ENTRY_WORDS) as usize;
        if held[at + node_cache::W_NODE as usize] >= KEPT_ZOOM {
            held[at] = 0;
        }
    }
    let fine = eye_of(&case([8, FINEST]), [0; 3]);
    let table = gpu.buffer(&held, store_of());
    let words = eye_beams(&gpu, &b, &fine, &table);
    let after_beam = gpu.read(&table);
    run_frame(
        &gpu,
        &b,
        w,
        &fine,
        &words,
        &after_beam,
        None,
        "fine",
        &mut kinds,
        &mut failed,
    );
    assert!(failed.is_empty(), "{}", failed.join("\n"));
}
