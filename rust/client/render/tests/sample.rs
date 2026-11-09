//! Checks that the hand-written walks answer as pxl-engine's walk from the root answers the
//! same ray, over the real world of tests/real_world.
//!
//! The samples' run (queue_walk.wgsl, queue_ask.wgsl and queue_last.wgsl over sample.wgsl),
//! for the eye and for the light (sample.eye.wgsl, sample.light.wgsl), starts from what the
//! beam passes leave. Each sample must stop where the Rust stops it, in a first frame and in
//! the next frame over the node cache the first left. The beam rays themselves, the holes'
//! queue over the pixels the cast marked, the drawable entries the samples' stops set and the
//! draw draws, and the boxes the light's samples draw must answer alike.
//!
//! The tests are skipped where no adapter answers.

mod gpu;
mod real_world;

use pxl_spec::share::{EIGHTHS, FILL_BITS};
use std::collections::{BTreeMap, BTreeSet};

use gpu::{
    at_edge, cross, edge_axes, edges_within, entry_node, holds_node, lay_kept, list_every, margins,
    read_texture, texture, unit, Gpu, KindCount, LightBox,
};
use pxl_engine::answer::{self, Kind};
use pxl_engine::beam::{self, Grid};
use pxl_engine::cast::{self, Detail, End, Keep, Node, Ray, Spacing, Trace, Until, DEPTH};
use pxl_engine::light::{self, Light, StartGrid};
use pxl_engine::node_cache;
use pxl_engine::object::Cut;
use pxl_engine::param;
use pxl_engine::world::Inheritance;
use pxl_engine::{eye, raster, ray, record};
use pxl_render::frame::shader::record::{src, SHADER_CELL, SHADER_LIGHT_CELL};
use pxl_render::frame::shader::word::SHADOW_SIZE;
use pxl_spec::space::camera::grid::CELL;
use pxl_spec::space::coord::q3::FRAC_W;
use pxl_spec::space::coord::zoom::{Zoom, LEAF_CELL_ZOOM};
use pxl_spec::value;

use real_world::{q8, store_of, truth, Asking, Built, World, WorldBuffer, FINEST};

/// The leaf cells the worlds are built about. `EYE` lies 300 cells west of X's seam, on the
/// east side of X = 0, and `EYE_WEST` 300 cells east of the seam, on the west side of X = 0.
const EYE: [i32; 2] = [(1 << 23) - 300, (40 << 16) + 30];
const EYE_WEST: [i32; 2] = [-(1 << 23) + 300, (40 << 16) + 30];
/// This one lies by X = 0, where one root ends and the next begins in X.
const EYE_ZERO: [i32; 2] = [150, (40 << 16) + 30];
const SIZE: u32 = 256;
const FRAME: u32 = 9;
const CAST: u32 = 7;
const WORKGROUP: u32 = beam::SAMPLE_SIDE;
/// The most differing pixels that are tried again from a start one Q3 step over. The device
/// works out an eye's ray itself (walk_node.wgsl `eye_aim`), and its rounding may take the ray
/// across an edge, which a start a step over stands in for.
const STEPS_TRIED: u32 = 32;
/// The word every sample's words are laid with. A sample that no walk writes keeps it.
const UNTOUCHED: u32 = 0xdead_beef;

/// An observer's samples as the test lays them: the eye's words and the light's head, laid as
/// pxl-engine eye.rs and light.rs place them, the observer's buffer of words, and how its
/// passes run.
struct Observed {
    name: String,
    eye: Vec<u32>,
    header: Vec<u32>,
    words: Vec<u32>,
    lattice: [u32; 2],
    /// Whether its passes read the light's head, which the light's do and the eye's do not.
    apart: bool,
    /// The workgroups of the hit pass's dispatch, across and down.
    groups: [u32; 2],
    cuts: [Cut; 2],
    source: RaySource,
}

/// Where an observer's rays start and go: the eye's grid or the light's box.
enum RaySource {
    Eye {
        origin: [i32; 3],
        axes: [[f32; 3]; 3],
        far: f32,
        range: [Zoom; 2],
        grid: Grid,
    },
    Light {
        light: Light,
        starts: StartGrid,
        range: [Zoom; 2],
        reads: Vec<bool>,
        /// The map's side, in texels.
        size: u32,
        /// The samples the threads take, where the cast does not begin its epoch. `None` means
        /// every sample.
        taken: Option<Vec<[u32; 2]>>,
    },
}

impl Observed {
    fn aim(&self, s: [u32; 2]) -> ([i32; 3], Ray, Detail) {
        match &self.source {
            RaySource::Eye {
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
                            angle: CELL,
                            across: 0.0,
                        },
                    },
                )
            }
            RaySource::Light {
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

    /// The band that sample `s`'s ray asked. An eye's ray asks no one band, so it is 0, and no
    /// eye's stop counts as whole by it (`record::light_whole`).
    fn band(&self, s: [u32; 2]) -> u32 {
        match &self.source {
            RaySource::Eye { .. } => 0,
            RaySource::Light { .. } => self.aim(s).2.range[0],
        }
    }

    fn sample_at(&self, s: [u32; 2]) -> u32 {
        match &self.source {
            RaySource::Eye { grid, .. } => grid.sample_at(s),
            RaySource::Light { size, .. } => light::sample(*size, s[1] * self.lattice[0] + s[0]),
        }
    }

    /// The samples the hit pass's threads take, each once.
    fn taken(&self) -> Vec<[u32; 2]> {
        if let RaySource::Light {
            taken: Some(list), ..
        } = &self.source
        {
            return list.clone();
        }
        let [a, d] = self.lattice;
        (0..d).flat_map(|j| (0..a).map(move |i| [i, j])).collect()
    }

    /// Whether the cast needs sample `s`: whether the sample's tile or a tile beside it is read.
    /// An eye needs every sample.
    fn needed(&self, s: [u32; 2]) -> bool {
        let RaySource::Light {
            light: l,
            reads,
            size,
            ..
        } = &self.source
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

    /// Sample `s`'s walk from the root, which is the answer the device must give.
    fn walk(&self, w: World, s: [u32; 2]) -> Trace<Inheritance> {
        let (o, ray, detail) = self.aim(s);
        walk_from(w, self.cuts, o, &ray, detail)
    }
}

/// A ray's walk from `o`, from the root down (`cast::walk_from_root`), with each node asked of
/// the world.
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

/// An eye over a built world: where it stands, where it looks, and the zooms it asks.
struct EyeCase {
    name: &'static str,
    origin: [i32; 3],
    look: [f32; 3],
    tan: [f32; 2],
    far: f32,
    range: [Zoom; 2],
    cuts: [Cut; 2],
}

/// A light box over a built world.
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
    /// `None` casts the whole lattice, as the first cast of an epoch does. `Some(k)` casts only
    /// the tiles listed, about one in `k` (`light_beams`).
    one_in: Option<u32>,
}

/// The eye of `c`, its origin moved by `moved` Q3 steps along each axis.
fn eye_of(c: &EyeCase, moved: [i32; 3]) -> Observed {
    let forward = unit(c.look);
    let right = unit(cross(forward, [0.0, 0.0, 1.0]));
    let up = cross(right, forward);
    let origin = [
        c.origin[0].wrapping_add(moved[0]),
        c.origin[1].wrapping_add(moved[1]),
        c.origin[2].wrapping_add(moved[2]),
    ];
    let grid = Grid::of(c.tan);
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
        header: vec![0; light::HEADER_WORDS],
        words: vec![0; grid.size() as usize],
        lattice: grid.samples,
        apart: false,
        groups: [
            grid.samples[0].div_ceil(WORKGROUP),
            grid.samples[1].div_ceil(WORKGROUP),
        ],
        cuts: c.cuts,
        source: RaySource::Eye {
            origin,
            axes: [forward, right, up],
            far: c.far,
            range: c.range,
            grid,
        },
    }
}

/// Runs the eye's beam passes over `observed`: the beam rays' run, then the cells' starts.
fn eye_beams(
    gpu: &Gpu,
    b: &WorldBuffer,
    observed: &Observed,
    node_cache: &wgpu::Buffer,
) -> Vec<u32> {
    let RaySource::Eye { grid, .. } = &observed.source else {
        unreachable!()
    };
    let eye = gpu.buf(&observed.eye, wgpu::BufferUsages::UNIFORM);
    let header = gpu.buf(&observed.header, wgpu::BufferUsages::UNIFORM);
    let beams = gpu.buf(&observed.words, store_of());
    gpu.corners(false, &world_binds(b, &eye, &beams, node_cache), node_cache);
    gpu.dispatch(
        &gpu.pipeline(src(SHADER_CELL), "cell_cs"),
        &[(0, eye.as_entire_binding()), (8, beams.as_entire_binding())],
        [
            grid.span[0].div_ceil(WORKGROUP),
            grid.span[1].div_ceil(WORKGROUP),
            1,
        ],
    );
    let _ = header;
    gpu.read(&beams)
}

fn world_binds<'a>(
    b: &'a WorldBuffer,
    eye: &'a wgpu::Buffer,
    beams: &'a wgpu::Buffer,
    node_cache: &'a wgpu::Buffer,
) -> Vec<(u32, wgpu::BindingResource<'a>)> {
    vec![
        (0, eye.as_entire_binding()),
        (1, b.clip.as_entire_binding()),
        (2, b.ground.as_entire_binding()),
        (4, b.folds.as_entire_binding()),
        (5, b.shards.as_entire_binding()),
        (6, b.placed.as_entire_binding()),
        (7, b.pool.as_entire_binding()),
        (8, beams.as_entire_binding()),
        (9, node_cache.as_entire_binding()),
    ]
}

/// The light box of `c` and where its rays start, with its corner moved by `moved` world
/// texels.
fn box_of(c: &LightCase, moved: [i32; 2]) -> (Light, StartGrid, [i32; 2]) {
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

/// The light of `c`, its corner moved by `moved`, before its lines and cells are laid. Every
/// tile is read.
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
    let mut header = vec![0u32; light::HEADER_WORDS];
    header[light::HEADER_CAST] = CAST;
    header[light::HEADER_EPOCH] = CAST;
    for (k, v) in light::header_layout(c.size) {
        header[k] = v;
    }
    header[light::STARTS_AT * 4..(light::STARTS_AT + light::STARTS_VEC4S) * 4]
        .copy_from_slice(&starts.words());
    let n = c.size / param::light::SAMPLE;
    Observed {
        name: c.name.to_string(),
        eye: words,
        header,
        words: vec![0; light::list_words(c.size) as usize],
        lattice: [n, n],
        apart: true,
        groups: [n.div_ceil(WORKGROUP), n.div_ceil(WORKGROUP)],
        cuts: c.cuts,
        source: RaySource::Light {
            light: l,
            starts,
            range: c.range,
            reads: vec![true; (light::tiles(c.size) * light::tiles(c.size)) as usize],
            size: c.size,
            taken: None,
        },
    }
}

/// Runs the light's beam rays and cells over the whole box. Then, where `one_in` is given, it
/// cuts the cast down to the tiles listed as fed, about one in `one_in`, and the rest are not
/// read. Some samples are then not taken, and some are taken but not needed.
fn light_beams(
    gpu: &Gpu,
    b: &WorldBuffer,
    observed: &mut Observed,
    node_cache: &wgpu::Buffer,
    one_in: Option<u32>,
) -> Vec<u32> {
    let RaySource::Light { size, .. } = observed.source else {
        unreachable!()
    };
    let eye = gpu.buf(&observed.eye, wgpu::BufferUsages::UNIFORM);
    let header = gpu.buf(&observed.header, wgpu::BufferUsages::UNIFORM);
    let tiles = light::tiles(size);
    let mut list = observed.words.clone();
    let reads_at = light::reads_at(size) as usize;
    list[reads_at..reads_at + (tiles * tiles) as usize].fill(CAST);
    // Every tile is listed as fed, as a cast that claims them all lists them. The samples'
    // passes run over that list, each entry a square of the tile's samples (`list_every`).
    let fed = list_every(&mut list, size, light::LIST_SOURCES, 1);
    observed.groups = list_every(
        &mut list,
        size,
        light::LIST_SOURCES,
        light::TILE / param::light::SAMPLE,
    );
    let beams = gpu.buf(&list, store_of());
    let mut world = world_binds(b, &eye, &beams, node_cache);
    world.push((11, header.as_entire_binding()));
    gpu.light_beam(&world, &beams, node_cache, size);
    gpu.dispatch(
        &gpu.pipeline(src(SHADER_LIGHT_CELL), "light_cell_cs"),
        &[
            (0, eye.as_entire_binding()),
            (8, beams.as_entire_binding()),
            (11, header.as_entire_binding()),
        ],
        [fed[0], fed[1], 1],
    );
    let mut words = gpu.read(&beams);
    let RaySource::Light {
        light: l,
        reads,
        taken,
        ..
    } = &mut observed.source
    else {
        unreachable!()
    };
    if let Some(every) = one_in {
        // Some tiles are read, the box's corners among them. The tiles listed as fed are those
        // beside a read tile, as the propagation lists them (light_enlist.wgsl `enlist`),
        // but only about one in `every` and those on the box's first column and last row.
        observed.header[light::HEADER_EPOCH] = CAST - 3;
        let side = light::TILE / param::light::SAMPLE;
        let at = light::list_cnt_at(size, light::LIST_SOURCES) as usize;
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
        // The entries are listed in reverse, and the room past them is filled with a tile that
        // is not listed. A thread that read past the list, or one place over, would then walk
        // samples that no thread takes, and the test would see them.
        entries.reverse();
        words[at] = entries.len() as u32;
        words[at + 1..at + 1 + entries.len()].copy_from_slice(&entries);
        let cap = light::list_cap(size, light::LIST_SOURCES) as usize;
        if entries.len() < cap {
            let unlisted = (0..tiles * tiles)
                .map(|k| ((k / tiles) << 16) | (k % tiles))
                .find(|e| !entries.contains(e))
                .expect("a tile unlisted");
            words[at + 1 + entries.len()..at + 1 + cap].fill(unlisted);
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
        let rows = threads.div_ceil(light::LIST_ACROSS * light::LIST_LANES);
        // A workgroup holds `light::LIST_LANES` threads, eight a side, as the pass's own
        // dispatch takes them.
        observed.groups = [light::LIST_ACROSS, rows];
    }
    words
}

/// Runs one frame's samples over the node cache `node_cache0`. Each sample's stop must be the stop
/// the walk from the root gives its ray, and no sample may be written that no thread takes.
/// Where `last_cache` is given, the run's last walk, which asks the world, runs over it, while the
/// walks over the node cache alone and their asks read `node_cache0`. Returns the node cache the
/// passes left.
#[expect(
    clippy::too_many_arguments,
    reason = "a frame's device, world, observer, words, tables, what it tells and its passes"
)]
fn run_frame(
    gpu: &Gpu,
    b: &WorldBuffer,
    w: World,
    observed: &Observed,
    words: &[u32],
    node_cache0: &[u32],
    last_cache: Option<&[u32]>,
    what: &str,
    kind_cnt: &mut KindCount,
    failed: &mut Vec<String>,
) -> Vec<u32> {
    let name = format!("{} ({what})", observed.name);
    let eye_buf = gpu.buf(&observed.eye, wgpu::BufferUsages::UNIFORM);
    let header_buf = gpu.buf(&observed.header, wgpu::BufferUsages::UNIFORM);
    let taken = observed.taken();

    let mut laid = words.to_vec();
    for j in 0..observed.lattice[1] {
        for i in 0..observed.lattice[0] {
            let at = observed.sample_at([i, j]) as usize;
            laid[at..at + beam::SAMPLE_WORDS as usize].fill(UNTOUCHED);
        }
    }
    let beams = gpu.buf(&laid, store_of());
    let node_cache = gpu.buf(node_cache0, store_of());
    let last_cache = last_cache.map(|t| gpu.buf(t, store_of()));
    let mut binds = world_binds(b, &eye_buf, &beams, &node_cache);
    if observed.apart {
        binds.push((11, header_buf.as_entire_binding()));
    }
    gpu.samples_last_cache(observed.apart, &binds, &node_cache, last_cache.as_ref());
    let node_cache = last_cache.unwrap_or(node_cache);
    let out = gpu.read(&beams);

    // A needed sample's stop must be what its ray's walk answers. A sample not needed must
    // stop on nothing.
    let (mut same, mut differ, mut axis_over, mut edges) = (0u64, 0u64, 0u64, 0u64);
    let mut first = None;
    for &s in &taken {
        let at = observed.sample_at(s) as usize;
        let node = at + beam::SAMPLE_NODE as usize;
        let zoom_word = node + beam::NODE_ZOOM as usize;
        if !observed.needed(s) {
            if out[zoom_word] != beam::SAMPLE_NONE {
                differ += 1;
                first.get_or_insert((s, out[at..at + 12].to_vec()));
            }
            continue;
        }
        let trace = observed.walk(w, s);
        let want_zoom = if trace.end == End::Miss {
            beam::SAMPLE_NONE
        } else {
            trace.hit.node.zoom
        };
        let mut ok = out[zoom_word] == want_zoom;
        if trace.end != End::Miss {
            ok =
                ok && (0..3).all(|a| {
                    out[node + beam::NODE_CORNER as usize + a] == trace.hit.node.corner[a]
                        && out[node + beam::NODE_ROOTS as usize + a]
                            == trace.hit.node.roots[a] as u32
                }) && out[at + beam::SAMPLE_ASSET as usize] == trace.hit.asset[0]
                    && out[at + beam::SAMPLE_ASSET as usize + 1] == trace.hit.asset[1];
            if ok && out[at + beam::SAMPLE_AXIS as usize] != trace.hit.axis {
                // The ray enters at an edge, and a GPU's rounding of the planes made it enter
                // across another axis.
                axis_over += 1;
            }
        } else {
            ok = ok
                && out[at + beam::SAMPLE_ASSET as usize] == 0
                && out[at + beam::SAMPLE_ASSET as usize + 1] == 0
                && out[at + beam::SAMPLE_AXIS as usize] == 0;
        }
        ok = ok
            && out[at + beam::SAMPLE_LACKS as usize] & beam::SAMPLE_LACK
                == u32::from(trace.end == End::Lacks);
        // A ray that the device's rounding takes across an edge may stop on either side of it.
        // The device's stop is taken where the cell the walk stopped at, or the one the device
        // stopped at, is entered at an edge (`at_edge`, as tests/cast.rs takes a pixel).
        if !ok {
            let (o, ray, _) = observed.aim(s);
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
            if theirs || (trace.end == End::Hit && edge(&trace.hit.node, trace.hit.asset[0])) {
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
                    trace.end, trace.hit.node.zoom, trace.hit.node.corner, trace.hit.node.roots, trace.hit.t, trace.hit.axis, trace.hit.asset, &out[at..at + beam::SAMPLE_WORDS as usize]
                );
            }
            first.get_or_insert((s, out[at..at + 12].to_vec()));
        }
        let hits = trace.end == End::Hit;
        let a = trace.hit.asset[0];
        kind_cnt.add("hit", hits);
        kind_cnt.add("past its reach", trace.end == End::Miss);
        kind_cnt.add("lacks", trace.end == End::Lacks);
        kind_cnt.add("hit, placed", hits && answer::is_placed(a));
        kind_cnt.add(
            "hit, short of its cube",
            hits && answer::fill_eighths(a) < 8,
        );
        kind_cnt.add("hit, a way's", hits && value::transports(answer::val(a)));
        kind_cnt.add("ended in another root", trace.hit.node.roots != [0; 3]);
    }
    // No sample that no thread takes may be written.
    let wanted: BTreeSet<[u32; 2]> = taken.iter().copied().collect();
    let mut stray = 0u64;
    for j in 0..observed.lattice[1] {
        for i in 0..observed.lattice[0] {
            if !wanted.contains(&[i, j]) {
                let at = observed.sample_at([i, j]) as usize;
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
    gpu.read(&node_cache)
}

/// Runs an observer's two frames. The second observer stands a step moved, and its frame runs
/// over the node cache the first frame left.
#[expect(
    clippy::too_many_arguments,
    reason = "the device, the world, two frames' observers, their lines, what to tell and what failed"
)]
fn two_frames(
    gpu: &Gpu,
    b: &WorldBuffer,
    w: World,
    first: &mut Observed,
    next: &mut Observed,
    lines: &mut dyn FnMut(&mut Observed, &wgpu::Buffer) -> Vec<u32>,
    kind_cnt: &mut KindCount,
    failed: &mut Vec<String>,
) {
    let node_cache_buf = gpu.buf(&frame_node_cache(), store_of());
    let words = lines(first, &node_cache_buf);
    let after_beam = gpu.read(&node_cache_buf);
    let mut table1 = run_frame(
        gpu,
        b,
        w,
        first,
        &words,
        &after_beam,
        None,
        "first frame",
        kind_cnt,
        failed,
    );
    table1[node_cache::FRAME as usize] = FRAME + 1;
    let node_cache_buf = gpu.buf(&table1, store_of());
    let words1 = lines(next, &node_cache_buf);
    let after_beam1 = gpu.read(&node_cache_buf);
    run_frame(
        gpu,
        b,
        w,
        next,
        &words1,
        &after_beam1,
        None,
        "next frame",
        kind_cnt,
        failed,
    );
}

/// Fails where the cases' rays met no case of a kind in `what`, as the test would then say
/// nothing of that kind.
fn met_each(kind_cnt: &KindCount, what: &[&str], failed: &mut Vec<String>) {
    eprintln!("  KINDS {kind_cnt:?}");
    for k in what {
        if kind_cnt.get(k) == 0 {
            failed.push(format!("none of {k} to say"));
        }
    }
}

fn world_bufs(gpu: &Gpu, built: &Built) -> WorldBuffer {
    let store = store_of();
    let flat_u = |rows: &[[u32; 4]]| rows.iter().flatten().copied().collect::<Vec<u32>>();
    let clip: Vec<u32> = built.clipmap.iter().flatten().map(|&v| v as u32).collect();
    WorldBuffer {
        clip: gpu.buf(&clip, store),
        ground: gpu.buf(&built.ground, store),
        folds: gpu.buf(&built.folds, store),
        shards: gpu.buf(&flat_u(&built.shards), store),
        placed: gpu.buf(&built.objects, store),
        pool: gpu.buf(&built.pool, store),
    }
}

fn device() -> Option<Gpu> {
    // A loop in a hand-written shader can hang the wait on the device, so the run ends itself.
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

/// Spots in the world that the cases stand at: on a bridge, down a bore, and on what is
/// placed, with the zoom and node of the leaf placed there.
struct Fixture {
    bridge: [i32; 3],
    bore: [i32; 3],
    on_placed: [i32; 2],
    leaf_zoom: Zoom,
    leaf_c: [i32; 3],
}

fn spots(b: &Built) -> Fixture {
    let none = [Cut::NONE; 2];
    let way = |lines: [u32; 2]| {
        let of_way = |p: [i32; 3]| {
            let c = [real_world::wrap(p[0]), p[1], p[2]];
            let zooms = [LEAF_CELL_ZOOM, LEAF_CELL_ZOOM];
            value::transports(answer::val(
                truth(b.world(), none, LEAF_CELL_ZOOM, c, zooms).0.asset[0],
            ))
        };
        b.way_points
            .iter()
            .filter(|p| p.0 == LEAF_CELL_ZOOM && lines.contains(&p.1) && p.2[0] > 0)
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
    Fixture {
        bridge: way([0, 3]),
        bore: way([1, 4]),
        on_placed: [leaf_c[0] >> 1, leaf_c[1] >> 1],
        leaf_zoom,
        leaf_c,
    }
}

/// A first word that no walk writes, which a marked pixel holds until its walk ends it.
/// `ray::first_word` sets no bit past the fill share's, and a ray that met nothing writes 0.
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
            far: 2000.0,
            range: [8, FINEST],
            cuts: none,
        },
        EyeCase {
            name: "the eye, over the seam",
            origin: over([EYE[0] + 280, EYE[1] + 90], 40),
            look: [0.8, 0.2, -0.55],
            tan: [0.4, 0.3],
            far: 2000.0,
            range: [8, FINEST],
            cuts: none,
        },
        EyeCase {
            name: "the eye, on what is placed, cut and hid",
            origin: over(at.on_placed, 20),
            look: [0.02, 0.03, -1.0],
            tan: [0.3, 0.4],
            far: 400.0,
            range: [LEAF_CELL_ZOOM, FINEST],
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
            far: 600.0,
            range: [LEAF_CELL_ZOOM, FINEST],
            cuts: none,
        },
        EyeCase {
            name: "the eye, down a bore, under the ground",
            origin: q8([at.bore[0], at.bore[1], at.bore[2]]),
            look: [0.6, 0.2, -0.3],
            tan: [0.3, 0.3],
            far: 300.0,
            range: [LEAF_CELL_ZOOM, FINEST],
            cuts: none,
        },
        EyeCase {
            name: "the eye, where no clip level holds the ground",
            origin: q8([1 << 15, 1 << 15, 40]),
            look: [0.3, 0.2, -0.93],
            tan: [0.4, 0.3],
            far: 800.0,
            range: [8, FINEST],
            cuts: none,
        },
        EyeCase {
            name: "the eye, high over the clip levels",
            origin: over([EYE[0] + 2000, EYE[1] + 2000], 2500),
            look: [0.3, -0.2, -0.9],
            tan: [0.3, 0.25],
            far: 20000.0,
            range: [4, FINEST],
            cuts: none,
        },
    ];
    let mut kind_cnt = KindCount::new();
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
            &mut |o, node_cache| eye_beams(&gpu, &b, o, node_cache),
            &mut kind_cnt,
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
        &mut |o, node_cache| eye_beams(&gpu, &bw, o, node_cache),
        &mut kind_cnt,
        &mut failed,
    );
    // Over the seam, where the world's X wraps.
    let c = EyeCase {
        name: "the eye, looking over the seam",
        origin: over_w([EYE_WEST[0] - 280, EYE_WEST[1] + 90], 40),
        look: [-0.9, 0.1, -0.3],
        tan: [0.3, 0.2],
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
        &mut |o, node_cache| eye_beams(&gpu, &bw, o, node_cache),
        &mut kind_cnt,
        &mut failed,
    );
    // Over X = 0, where one root ends and the next begins in X, the eye's rays end in the next
    // root.
    let zero = real_world::build(EYE_ZERO);
    let bz = world_bufs(&gpu, &zero);
    let over_z = |g: [i32; 2], up: i32| q8([g[0], g[1], zero.top_at(g) + up]);
    let c = EyeCase {
        name: "the eye, looking over X = 0",
        origin: over_z([20, EYE_ZERO[1] + 90], 40),
        look: [-0.9, 0.1, -0.3],
        tan: [0.3, 0.2],
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
        &mut |o, node_cache| eye_beams(&gpu, &bz, o, node_cache),
        &mut kind_cnt,
        &mut failed,
    );
    met_each(
        &kind_cnt,
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

/// A walk that asks may read a slot while another walk writes it, and the slot's key puts no
/// order over its words (pxl-engine node_cache.rs). So an entry that holds words of two writes
/// must answer nothing. Every entry the first frame kept has one word of its answer changed,
/// with its key and its node left standing. The next frame's walks over the node cache alone
/// start from one that kept nothing, so every sample is left to its asking walk over the torn
/// entries, and each must still answer as the walk from the root does.
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
        far: 2000.0,
        range: [8, FINEST],
        cuts: [Cut::NONE; 2],
    };
    let (mut kind_cnt, mut failed) = (KindCount::new(), Vec::new());
    let observed = eye_of(&c, [0; 3]);
    let empty = frame_node_cache();
    let node_cache = gpu.buf(&empty, store_of());
    let words = eye_beams(&gpu, &b, &observed, &node_cache);
    let first = gpu.read(&node_cache);
    let mut torn = run_frame(
        &gpu,
        &b,
        w,
        &observed,
        &words,
        &first,
        None,
        "first frame",
        &mut kind_cnt,
        &mut failed,
    );
    torn[node_cache::FRAME as usize] = FRAME + 1;
    let answer_words = node_cache::W_NODE - node_cache::W_KIND;
    let mut changed = 0u32;
    for slot in 0..node_cache::ENTRIES {
        if torn[node_cache::key_at(slot) as usize] != 0 {
            let at = node_cache::entry_at(slot) + node_cache::W_KIND + slot % answer_words;
            torn[at as usize] ^= 1 << 8;
            changed += 1;
        }
    }
    assert!(changed > 1000, "{changed} entries kept to change");
    let mut next_empty = empty;
    next_empty[node_cache::FRAME as usize] = FRAME + 1;
    let node_cache = gpu.buf(&next_empty, store_of());
    let words = eye_beams(&gpu, &b, &observed, &node_cache);
    let next_first = gpu.read(&node_cache);
    run_frame(
        &gpu,
        &b,
        w,
        &observed,
        &words,
        &next_first,
        Some(&torn),
        "next frame",
        &mut kind_cnt,
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
            one_in: None,
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
            one_in: Some(3),
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
            one_in: Some(1),
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
            size: SHADOW_SIZE,
            one_in: Some(23),
        },
        LightCase {
            name: "the light, one band everywhere",
            origin: over([EYE[0] + 90, EYE[1] + 90], 40),
            dir: [0.3, 0.2, -0.93],
            side: 1.0,
            back: 60.0,
            t_max: 400.0,
            range: [LEAF_CELL_ZOOM, LEAF_CELL_ZOOM],
            corner: [1304, -896],
            cuts: none,
            size: SIZE,
            one_in: None,
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
            one_in: None,
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
            one_in: None,
        },
        LightCase {
            name: "the light, on what is placed, cut and hid",
            origin: over(at.on_placed, 30),
            dir: [0.1, 0.25, -0.96],
            side: 0.25,
            back: 10.0,
            t_max: 120.0,
            range: [LEAF_CELL_ZOOM, FINEST],
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
            one_in: None,
        },
        LightCase {
            name: "the light, on a bridge, along an axis",
            origin: q8([at.bridge[0], at.bridge[1], at.bridge[2] + 30]),
            dir: [0.0, 0.0, -1.0],
            side: 0.5,
            back: 10.0,
            t_max: 200.0,
            range: [LEAF_CELL_ZOOM, FINEST],
            corner: [-16, 32],
            cuts: none,
            size: SIZE,
            one_in: None,
        },
        LightCase {
            name: "the light, on a bridge, listed, the box wrapping",
            origin: q8([at.bridge[0], at.bridge[1], at.bridge[2] + 30]),
            dir: [0.3, -0.1, -0.95],
            side: 0.5,
            back: 10.0,
            t_max: 200.0,
            range: [LEAF_CELL_ZOOM, FINEST],
            corner: [-16, 32],
            cuts: none,
            size: SIZE,
            one_in: Some(3),
        },
    ];
    let mut kind_cnt = KindCount::new();
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
            &mut |o, node_cache| light_beams(&gpu, &b, o, node_cache, c.one_in),
            &mut kind_cnt,
            &mut failed,
        );
    }
    // A box over the seam, where the world's X wraps. It asks one band everywhere, so that
    // every cell resumes from its beam rays.
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
        range: [LEAF_CELL_ZOOM, LEAF_CELL_ZOOM],
        corner: [1304, -896],
        cuts: none,
        size: SIZE,
        one_in: None,
    };
    let mut first = light_of(&c, [0, 0]);
    let mut next = light_of(&c, [0, 0]);
    two_frames(
        &gpu,
        &bw,
        west.world(),
        &mut first,
        &mut next,
        &mut |o, node_cache| light_beams(&gpu, &bw, o, node_cache, c.one_in),
        &mut kind_cnt,
        &mut failed,
    );
    // A box over X = 0, where one root ends and the next begins in X. It asks one band
    // everywhere, so that every cell resumes from its beam rays, and a line's start and a
    // ray's start lie in different roots.
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
        range: [LEAF_CELL_ZOOM, LEAF_CELL_ZOOM],
        corner: [1304, -896],
        cuts: none,
        size: SIZE,
        one_in: None,
    };
    let mut first = light_of(&c, [0, 0]);
    let mut next = light_of(&c, [0, 0]);
    two_frames(
        &gpu,
        &bz,
        zero.world(),
        &mut first,
        &mut next,
        &mut |o, node_cache| light_beams(&gpu, &bz, o, node_cache, c.one_in),
        &mut kind_cnt,
        &mut failed,
    );
    // A narrow box slanting east across X = 0. A ray resumes from a line in the next root
    // over, so the node the line holds is taken from the ray's origin a root apart
    // (resume.wgsl `moved_roots`). In a wider box every such cell starts at 0, as a beam ray
    // stops at X = 0 within its margin, and nothing resumes across the roots.
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
        &mut |o, node_cache| light_beams(&gpu, &bz, o, node_cache, c.one_in),
        &mut kind_cnt,
        &mut failed,
    );
    met_each(
        &kind_cnt,
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

/// A screen whose pixels the cast marked as holes, for the walks to take again. `size` is its
/// pixels across and down, `jitter` is the eye's jitter, and one pixel in `marked_in` is marked
/// and listed (pxl-engine `ray::holes_at`).
struct Screen {
    name: &'static str,
    size: [u32; 2],
    jitter: [f32; 2],
    marked_in: u32,
}

/// The tangents across and up of pixel `p`'s ray through its jittered middle, with row 0 at
/// the top. Every pass that walks a drawn pixel takes its ray by this rule (grid.wgsl
/// `px_tan`).
fn px_tan(s: &Screen, tan: [f32; 2], p: [u32; 2]) -> [f32; 2] {
    let ndc_x = (p[0] as f32 + 0.5 + s.jitter[0]) / s.size[0] as f32 * 2.0 - 1.0;
    let ndc_y = 1.0 - (p[1] as f32 + 0.5 + s.jitter[1]) / s.size[1] as f32 * 2.0;
    [ndc_x * tan[0], ndc_y * tan[1]]
}

/// Lists the pixels `marked` of a target `size` in the eye's holes `holes`, which start at
/// their first word, as `cast_cs` lists them. Each pixel in `marked` is its row times the
/// width plus its column.
fn list_holes(holes: &mut [u32], size: [u32; 2], marked: &[u32]) {
    let at = ray::holes_at(size) as usize;
    holes[at] = marked.len() as u32;
    for (k, &p) in marked.iter().enumerate() {
        holes[at + 1 + k] = (p % size[0]) | ((p / size[0]) << 16);
    }
}

/// Pixel `p`'s ray and the zooms it asks.
fn px_aim(observed: &Observed, s: &Screen, p: [u32; 2]) -> (Ray, Detail) {
    let RaySource::Eye {
        axes,
        far,
        range,
        grid,
        ..
    } = &observed.source
    else {
        unreachable!()
    };
    let at = px_tan(s, grid.tan, p);
    let d = cast::through(axes[0], axes[1], axes[2], at[0], at[1]);
    let detail = Detail {
        range: *range,
        spacing: Spacing {
            angle: CELL,
            across: 0.0,
        },
    };
    (Ray::new(d, *far), detail)
}

/// The holes' queue over the eye's marked pixels (queue.wgsl, queue_walk.wgsl, queue_ask.wgsl,
/// queue_last.wgsl) must give every marked pixel the words the walk from the root gives its
/// ray, and must write no other pixel.
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
            far: 2000.0,
            range: [8, FINEST],
            cuts: none,
        },
        EyeCase {
            name: "the eye, over the seam",
            origin: over([EYE[0] + 280, EYE[1] + 90], 40),
            look: [0.8, 0.2, -0.55],
            tan: [0.4, 0.3],
            far: 2000.0,
            range: [8, FINEST],
            cuts: none,
        },
        EyeCase {
            name: "the eye, on a bridge",
            origin: q8([at.bridge[0], at.bridge[1], at.bridge[2] + 12]),
            look: [0.5, -0.3, -0.8],
            tan: [0.3, 0.3],
            far: 600.0,
            range: [LEAF_CELL_ZOOM, FINEST],
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
    let mut kind_cnt = KindCount::new();
    let mut failed = Vec::new();
    for c in &eyes {
        for s in &screens {
            holes_answer(&gpu, &b, w, (c, s), false, &mut kind_cnt, &mut failed);
        }
    }
    met_each(&kind_cnt, &["hit", "past its reach"], &mut failed);
    assert!(failed.is_empty(), "{}", failed.join("\n"));
}

/// Runs the holes' queue over the pixels of `s` that the cast marked, from the eye of case `c`.
/// Every marked pixel must hold the words the walk from the root gives its ray, and no other
/// pixel may be written. Returns the stops the run counted (`node_cache::HITS_CNT`).
fn holes_answer(
    gpu: &Gpu,
    b: &WorldBuffer,
    w: World,
    (c, s): (&EyeCase, &Screen),
    spent: bool,
    kind_cnt: &mut KindCount,
    failed: &mut Vec<String>,
) -> u32 {
    let name = format!("{}, {}", c.name, s.name);
    let mut observed = eye_of(c, [0; 3]);
    for k in 0..2 {
        observed.eye[eye::JITTER + k] = s.jitter[k].to_bits();
    }
    let mut node_cache0 = vec![0u32; node_cache::WORDS as usize];
    node_cache0[node_cache::FRAME as usize] = FRAME;
    let node_cache_buf = gpu.buf(&node_cache0, store_of());
    let mut words = eye_beams(gpu, b, &observed, &node_cache_buf);
    let RaySource::Eye { origin, grid, .. } = &observed.source else {
        unreachable!()
    };
    let (origin, grid) = (*origin, *grid);
    // The eye's holes lie past the grid's words.
    let walked = grid.walked_at() as usize;
    words.resize(walked + ray::holes_words(s.size) as usize, 0);

    // The pixels the cast marked are listed.
    let [wd, ht] = s.size;
    let texels = (wd * ht) as usize;
    let mark = |p: u32| (p.wrapping_mul(0x9e37_79b9) >> 7).is_multiple_of(s.marked_in);
    let marked: Vec<u32> = (0..wd * ht).filter(|&p| mark(p)).collect();
    list_holes(&mut words[walked..], s.size, &marked);
    let mut ray_words: Vec<u32> = (0..ray::WORDS as usize * texels)
        .map(|k| (k as u32).wrapping_mul(0x2c1b_3c6d) >> 2)
        .collect();
    for &p in &marked {
        ray_words[p as usize] = UNWALKED;
    }

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
    let eye_buf = gpu.buf(&observed.eye, wgpu::BufferUsages::UNIFORM);
    // The eye's head is all zero, as the cast binds it (ray/cast.rs `eye_header`).
    let header = gpu.buf(
        &vec![0u32; light::HEADER_WORDS],
        wgpu::BufferUsages::UNIFORM,
    );
    let beams = gpu.buf(&words, store_of());
    let mut binds = world_binds(b, &eye_buf, &beams, &node_cache_buf);
    binds.push((3, wgpu::BindingResource::TextureView(&view)));
    binds.push((11, header.as_entire_binding()));
    gpu.asks_spent.set(spent);
    gpu.holes(ray::QUEUE_EYE_HOLES, &binds, &node_cache_buf);
    gpu.asks_spent.set(false);
    let out = read_texture(&gpu.device, &gpu.queue, &rays);
    if spent {
        lacks_answer(w, &observed, s, origin, &marked, &out, kind_cnt, failed);
        return gpu.read(&node_cache_buf)[node_cache::HITS_CNT as usize];
    }

    let (same, differ, stepped) =
        marked_answers(w, &observed, s, origin, &marked, &out, &name, kind_cnt);
    // A pixel the cast did not mark must keep its words.
    let untouched = (0..wd * ht).filter(|&p| !mark(p)).all(|p| {
        (0..ray::WORDS as usize)
            .all(|l| out[l * texels + p as usize] == ray_words[l * texels + p as usize])
    });
    if !untouched {
        failed.push(format!("{name}: a pixel not marked was written"));
    }
    eprintln!("{name}: {same} alike, {differ} differ, {stepped} a step over");
    // Every ray ends within its frame. A ray that the run's pool has no room to park
    // (`ray::POOL_CAP`) ends where it stands, as a lack at the deepest node the node cache
    // holds, so it may differ from the walk. No more pixels than those may differ.
    let lacks = (marked.len() as u32).saturating_sub(ray::POOL_CAP);
    if differ > lacks {
        failed.push(format!(
            "{name}: {differ} pixels differ, {lacks} past the pool"
        ));
    }
    gpu.read(&node_cache_buf)[node_cache::HITS_CNT as usize]
}

/// Checks the `marked` pixels' words where the frame could ask nothing more of the world, so
/// that a ray goes only as deep as the node cache holds. A pixel that shows anything must show
/// what the world answers of the node at that zoom where its ray stands, asked as a cell of
/// its own zoom, which is the deepest node held (queue_last.wgsl `shows`).
#[expect(
    clippy::too_many_arguments,
    reason = "the world, the observer, its screen and origin, the pixels, their words and what to tell"
)]
fn lacks_answer(
    w: World,
    observed: &Observed,
    s: &Screen,
    origin: [i32; 3],
    marked: &[u32],
    out: &[u32],
    kind_cnt: &mut KindCount,
    failed: &mut Vec<String>,
) {
    let [wd, ht] = s.size;
    let texels = (wd * ht) as usize;
    let mut wrong = 0;
    for &p in marked {
        let (r, d) = px_aim(observed, s, [p % wd, p / wd]);
        let got = |l: u32| out[l as usize * texels + p as usize];
        if got(0) == 0 {
            kind_cnt.add("shows nothing", true);
            continue;
        }
        let zoom = got(0) >> ray::ZOOM_SHIFT & ((1 << ray::ZOOM_BITS) - 1);
        // A step past where the ray stands, so that the point lies in the node it entered.
        let t = f32::from_bits(got(1)) + 2.0 / (1 << FRAC_W) as f32;
        let (point, roots) = cast::point_at(origin, &r, t);
        let keep = if zoom == 0 { 0 } else { !0u32 << (32 - zoom) };
        let node = Node {
            corner: point.map(|c| c & keep),
            zoom,
            roots,
        };
        let (a, _) = truth(w, observed.cuts, zoom, node.idx(origin), [zoom, zoom]);
        let shown = a.kind == Kind::Cell
            && (0..6).any(|face| got(0) == ray::first_word(a.asset[0], face, zoom))
            && got(2) == a.asset[1];
        if !shown {
            wrong += 1;
        }
        let trace = walk_from(w, observed.cuts, origin, &r, d);
        kind_cnt.add(
            "a coarser cell than the walk's",
            trace.end == End::Hit && zoom < trace.hit.node.zoom,
        );
    }
    if wrong != 0 {
        failed.push(format!("{wrong} pixels show what no node holds"));
    }
}

/// Compares each of the `marked` pixels' words in the rays `out` with the walk from the root
/// along its ray from `origin`. Where they differ, starts within one Q3 step, as a GPU may
/// round the start, are tried for at most `STEPS_TRIED` pixels. Returns how many pixels are
/// alike, how many differ, and how many matched from a start a step over, and counts what each
/// ray trace in `kind_cnt`.
#[expect(
    clippy::too_many_arguments,
    reason = "the world, the observer, its screen and origin, the pixels, their words and what to tell"
)]
fn marked_answers(
    w: World,
    observed: &Observed,
    s: &Screen,
    origin: [i32; 3],
    marked: &[u32],
    out: &[u32],
    name: &str,
    kind_cnt: &mut KindCount,
) -> (u32, u32, u32) {
    let [wd, ht] = s.size;
    let texels = (wd * ht) as usize;
    let (mut same, mut differ, mut stepped) = (0u32, 0u32, 0u32);
    for &p in marked {
        let px = [p % wd, p / wd];
        let (r, d) = px_aim(observed, s, px);
        let trace = walk_from(w, observed.cuts, origin, &r, d);
        let got = |l: u32| out[l as usize * texels + p as usize];
        let face = 2 * trace.hit.axis + u32::from(r.dir[trace.hit.axis as usize] > 0.0);
        let mut ok = got(0) != UNWALKED;
        if trace.end == End::Hit {
            // The pixel must name the face its ray enters across. At an edge between two axes'
            // planes it may name the other axis's face, as the device's rounding of the ray may
            // take it there (`edge_axes`, as tests/cast.rs takes a pixel at an edge).
            let e = cast::entry_with(origin, &r, r.inverse(), &trace.hit.node, trace.hit.asset[0]);
            let axes = edge_axes(&e, r.dir);
            let entered = |axis: u32| {
                got(0)
                    == ray::first_word(
                        trace.hit.asset[0],
                        ray::face(r.dir, axis),
                        trace.hit.node.zoom,
                    )
            };
            let other = !entered(trace.hit.axis) && axes.iter().any(|&a| entered(a));
            kind_cnt.add("at an edge", axes.len() > 1);
            kind_cnt.add("at an edge, the other axis's face", other);
            ok = ok && (entered(trace.hit.axis) || other) && got(2) == trace.hit.asset[1];
            let t = f32::from_bits(got(1));
            ok = ok && (t - trace.hit.t).abs() <= 1e-4 * trace.hit.t.max(1.0);
        } else {
            ok = ok && got(0) == 0 && got(1) == 0;
        }
        if !ok && stepped <= STEPS_TRIED {
            // Tries every start within one Q3 step on each axis, as a GPU may round the start.
            let o = origin;
            let again = (0..27).any(|m| {
                let from = [o[0] + m % 3 - 1, o[1] + (m / 3) % 3 - 1, o[2] + m / 9 - 1];
                let m = walk_from(w, observed.cuts, from, &r, d);
                m.end == trace.end
                    && (m.end != End::Hit
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
                    trace.end,
                    trace.hit.node.zoom,
                    trace.hit.node.corner,
                    trace.hit.t,
                    trace.hit.axis,
                    trace.hit.asset,
                    [got(0), got(1), got(2), got(3)]
                );
            }
            assert!(
                std::env::var_os("MUT_WGSL").is_none(),
                "{name}: {px:?} differs"
            );
        }
        kind_cnt.add("hit", trace.end == End::Hit);
        kind_cnt.add("past its reach", trace.end == End::Miss);
    }
    (same, differ, stepped)
}

/// `hand_missed_pixels_answer_as_cast_walk` over every pixel of a screen. A ray at an edge
/// between two axes' planes may enter across either face, as the device's rounding of the ray
/// takes it (`edge_axes`), and the test must meet at least one such ray.
#[test]
fn hand_missed_pixels_at_an_edge_answer_as_cast_walk() {
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
        far: 150.0,
        range: [8, FINEST],
        cuts: [Cut::NONE; 2],
    };
    // Looking up over the hills, one ray meets the edge of two faces of a cell, with its two
    // planes' t a little apart, and the device's rounding of its direction and reciprocal puts
    // it across the other face (pixel [15, 39]).
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
    let mut kind_cnt = KindCount::new();
    let mut failed = Vec::new();
    for c in &cases {
        holes_answer(&gpu, &b, w, (c, &s), false, &mut kind_cnt, &mut failed);
    }
    met_each(
        &kind_cnt,
        &["hit", "past its reach", "at an edge"],
        &mut failed,
    );
    assert!(failed.is_empty(), "{}", failed.join("\n"));
}

/// The holes' queue over more pixels than a frame's stops hold (`ray::HITS_A_FRAME`). A ray
/// past them keeps no stop, and every marked pixel must still hold the words the walk from the
/// root gives its ray (`holes_answer`).
#[test]
fn eye_rays_past_a_frames_hits_answer_as_cast_walk() {
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
        far: 600.0,
        range: [8, FINEST],
        cuts: [Cut::NONE; 2],
    };
    let s = Screen {
        name: "more pixels marked than a frame's hits",
        size: [768, 432],
        jitter: [0.25, -0.375],
        marked_in: 1,
    };
    let mut kind_cnt = KindCount::new();
    let mut failed = Vec::new();
    let hits = holes_answer(&gpu, &b, w, (&c, &s), false, &mut kind_cnt, &mut failed);
    if hits <= ray::HITS_A_FRAME {
        failed.push(format!(
            "{}: {hits} hits, not past {}",
            c.name,
            ray::HITS_A_FRAME
        ));
    }
    assert!(failed.is_empty(), "{}", failed.join("\n"));
}

/// With the frame's asks spent, a ray goes only as deep as the node cache holds, and a pixel
/// shows the deepest node held, not nothing.
#[test]
fn hand_a_lack_shows_the_deepest_node_held() {
    let Some(gpu) = Gpu::new() else {
        eprintln!("no adapter: skipped");
        return;
    };
    let built = real_world::build(EYE);
    let b = world_bufs(&gpu, &built);
    let w = built.world();
    let c = EyeCase {
        name: "the eye, over the ground, its asks spent",
        origin: q8([
            EYE[0] + 90,
            EYE[1] + 90,
            built.top_at([EYE[0] + 90, EYE[1] + 90]) + 30,
        ]),
        look: [0.5, 0.4, -0.77],
        tan: [0.3, 0.2],
        far: 600.0,
        range: [8, FINEST],
        cuts: [Cut::NONE; 2],
    };
    let s = Screen {
        name: "every pixel marked and listed",
        size: [131, 77],
        jitter: [0.25, -0.375],
        marked_in: 1,
    };
    let mut kind_cnt = KindCount::new();
    let mut failed = Vec::new();
    holes_answer(&gpu, &b, w, (&c, &s), true, &mut kind_cnt, &mut failed);
    met_each(&kind_cnt, &["a coarser cell than the walk's"], &mut failed);
    assert!(failed.is_empty(), "{}", failed.join("\n"));
}

/// The word the boxes' buffer is laid with. A word the draw does not write keeps it.
const UNWRITTEN: u32 = 0x5eed_0b0c;

/// Runs the samples over `node_cache` and returns the words they leave. Each
/// sample's words are first laid as a cast before left them in `prior`, or with `UNTOUCHED`
/// where `prior` is `None`.
fn stopped(
    gpu: &Gpu,
    b: &WorldBuffer,
    observed: &Observed,
    words: &[u32],
    prior: Option<&[u32]>,
    node_cache: &[u32],
) -> Vec<u32> {
    let eye_buf = gpu.buf(&observed.eye, wgpu::BufferUsages::UNIFORM);
    let header_buf = gpu.buf(&observed.header, wgpu::BufferUsages::UNIFORM);
    let mut laid = words.to_vec();
    for j in 0..observed.lattice[1] {
        for i in 0..observed.lattice[0] {
            let at = observed.sample_at([i, j]) as usize;
            let to = at + beam::SAMPLE_WORDS as usize;
            match prior {
                Some(p) => laid[at..to].copy_from_slice(&p[at..to]),
                None => laid[at..to].fill(UNTOUCHED),
            }
        }
    }
    let beams = gpu.buf(&laid, store_of());
    let node_cache = gpu.buf(node_cache, store_of());
    let mut binds = world_binds(b, &eye_buf, &beams, &node_cache);
    if observed.apart {
        binds.push((11, header_buf.as_entire_binding()));
    }
    gpu.samples(observed.apart, &binds, &node_cache);
    gpu.read(&beams)
}

/// A box as the raster forms it from its draw word and its entry (pxl-engine `raster`), in the
/// test's own layout: its lowest and highest planes as f32 bits (`BOX_LO`, `BOX_HI`), its
/// voxel's number (`BOX_ID`, `raster::id_of`), and its faces with `raster::EYED`
/// (`BOX_FACES`).
type RasterBox = [u32; BOX_WORDS as usize];
const BOX_LO: u32 = 0;
const BOX_HI: u32 = 3;
const BOX_ID: u32 = 6;
const BOX_FACES: u32 = 7;
const BOX_WORDS: u32 = 8;

/// A voxel's key as a sample's words hold it: its node's zoom, corner and roots, as seen from
/// where its ray starts.
type Key = [u32; beam::NODE_WORDS as usize];

fn sample_key(words: &[u32], at: usize) -> Key {
    key_words(words, at + beam::SAMPLE_NODE as usize)
}

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

/// The box each voxel of the samples' words `words` must draw from `origin`, keyed by the
/// voxel's key (`sample_key`). A voxel is a node that some sample stopped on. Its box is the
/// cell as the cast stops on it (`cast::asset_box`). It is `raster::EYED` where `tan` gives the
/// screen and the box lies about the eye (`raster::is_eyed`). It draws a face for each side
/// that some sample of it entered through (`raster::faces`). Its id holds `raster::LACK` alone,
/// since the voxel's number depends on the order the boxes are written in (`BOX_ID`).
fn boxes_want(
    observed: &Observed,
    words: &[u32],
    origin: [i32; 3],
    tan: Option<[f32; 2]>,
    kind_cnt: &mut KindCount,
) -> BTreeMap<Key, RasterBox> {
    let mut want: BTreeMap<Key, RasterBox> = BTreeMap::new();
    let mut add = |k: &'static str, yes: bool| kind_cnt.add(k, yes);
    for s in observed.taken() {
        let k = observed.sample_at(s) as usize;
        let node = k + beam::SAMPLE_NODE as usize;
        let zoom = words[node + beam::NODE_ZOOM as usize];
        add("samples", true);
        if zoom == beam::SAMPLE_NONE {
            add("past its reach, no box", true);
            continue;
        }
        let axis = words[k + beam::SAMPLE_AXIS as usize];
        // The side its ray entered through (`ray::face`).
        let face = ray::face(observed.aim(s).1.dir, axis);
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
        // record_box.wgsl insets a cell as `lo + (hi - lo) · by`, which a device may fuse into
        // one rounding. A fused sum rounds as the Rust's separate product and sum wherever the
        // product is exact, as a node's side, a power of two, times `by` is. The test fails
        // where an inset's product rounds, so that the planes can be compared word for word.
        let eighths = answer::fill_eighths(asset);
        if eighths != EIGHTHS {
            let whole = asset & !(((1 << FILL_BITS) - 1) << answer::FILL_SHIFT);
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
        let lacks = words[k + beam::SAMPLE_LACKS as usize] & beam::SAMPLE_LACK != 0;
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
        // Counts an eyed box whose nearest point lies past what the screen's corner alone adds
        // to the reach, and past half the reach.
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
            answer::fill_eighths(asset) < EIGHTHS,
        );
        add("boxes, in another root", cell.roots != [0; 3]);
        want.insert(key, w);
    }
    want
}

/// Runs the light's hand-written record of its samples' stops over `words`, in frame `frame`,
/// into a node cache that keeps each node a sample stopped on as its walk would (`lay_kept`),
/// and checks it against pxl-engine `record`. Each such entry's state must hold the faces its
/// samples entered across, and whether some ray stopped on it coarser than its band
/// (`record::light_whole`). Each must be marked drawn in the frame's number, as it draws from
/// then on, and no other entry may be drawn. Failures go into `failed`.
#[expect(
    clippy::too_many_arguments,
    reason = "the device, the pass, its observer, words, groups, frame, what to tell and what failed"
)]
fn light_record_case(
    gpu: &Gpu,
    pass: &wgpu::ComputePipeline,
    observed: &Observed,
    words: &[u32],
    groups: [u32; 2],
    frame: u32,
    kind_cnt: &mut KindCount,
    failed: &mut Vec<String>,
) {
    let name = &observed.name;
    let mut laid = frame_node_cache();
    laid[node_cache::FRAME as usize] = frame;
    // The slot of each stopped sample's node, kept as the walk that stopped it keeps it, and
    // the state its samples set.
    let mut want: BTreeMap<u32, u32> = BTreeMap::new();
    for s in observed.taken() {
        let k = observed.sample_at(s) as usize;
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
        let o = observed.aim(s).0;
        let lacks = words[k + beam::SAMPLE_LACKS as usize] & beam::SAMPLE_LACK != 0;
        let asset = [
            words[k + beam::SAMPLE_ASSET as usize],
            words[k + beam::SAMPLE_ASSET as usize + 1],
        ];
        let kind = if lacks { Kind::Lacks } else { Kind::Cell };
        let idx = n.idx(o);
        let first = node_cache::bucket_of(zoom, idx);
        let slot = lay_kept(&mut laid, o, &n, (kind, asset)).or_else(|| {
            (first..first + node_cache::BUCKET).find(|&slot| {
                let w = (node_cache::entry_at(slot) + node_cache::W_NODE) as usize;
                holds_node(&laid, slot)
                    && laid[w] == zoom
                    && [1, 2, 3].map(|a| laid[w + a] as i32) == idx
            })
        });
        let Some(slot) = slot else {
            kind_cnt.add("voxels, their bucket full", true);
            continue;
        };
        let state = raster::faces([ray::face(
            observed.aim(s).1.dir,
            words[k + beam::SAMPLE_AXIS as usize],
        )]) | if record::light_whole(zoom, observed.band(s)) {
            record::WHOLE
        } else {
            0
        };
        *want.entry(slot).or_default() |= state;
        kind_cnt.add("voxels", true);
        kind_cnt.add("voxels, lacks", lacks);
        kind_cnt.add("voxels, in another root", n.roots != [0; 3]);
        kind_cnt.add(
            "voxels, short of its cube",
            answer::fill_eighths(asset[0]) < EIGHTHS,
        );
    }
    let mut args = vec![0u32; raster::ARGS_WORDS as usize];
    args[raster::ARGS_VERTICES as usize] = raster::VERTICES;
    let eye_buf = gpu.buf(&observed.eye, wgpu::BufferUsages::UNIFORM);
    let header_buf = gpu.buf(&observed.header, wgpu::BufferUsages::UNIFORM);
    let beams = gpu.buf(words, store_of());
    let node_cache = gpu.buf(&laid, store_of());
    let args = gpu.buf(&args, store_of());
    gpu.dispatch(
        pass,
        &[
            (0, eye_buf.as_entire_binding()),
            (8, beams.as_entire_binding()),
            (9, node_cache.as_entire_binding()),
            (11, header_buf.as_entire_binding()),
            (16, args.as_entire_binding()),
        ],
        [groups[0], groups[1], 1],
    );
    let got = gpu.read(&node_cache);
    let mut differ = 0u64;
    for (&slot, &state) in &want {
        let alike = got[node_cache::state_at(slot) as usize] == state
            && got[node_cache::drawn_at(slot) as usize] == frame;
        differ += u64::from(!alike);
    }
    let stray = (0..node_cache::ENTRIES)
        .filter(|slot| got[node_cache::state_at(*slot) as usize] != 0 && !want.contains_key(slot))
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
    light_raster_drawn(gpu, observed, &got, kind_cnt, failed);
}

/// Runs the light's record draw over the node cache `cache` (record_draw.wgsl
/// `light_record_draw_cs`), then the raster's draw of those boxes from the light onto the map
/// of `observed`, with the light's view laid as the renderer lays it (ray/raster.rs
/// `light_prepare`). Every texel that keeps a voxel must keep one whose box
/// (`cast::asset_box`) the texel's own ray enters within its reach. That ray starts on the near
/// plane and runs along the light. Where the device's rounding may decide the entry otherwise,
/// the box is grown by that rounding's room. Some texel must keep a voxel.
fn light_raster_drawn(
    gpu: &Gpu,
    observed: &Observed,
    cache: &[u32],
    kind_cnt: &mut KindCount,
    failed: &mut Vec<String>,
) {
    use pxl_engine::param::raster::GROWTH;
    use pxl_render::frame::shader::record::SHADER_RECORD_DRAW;
    use pxl_render::frame::shader::word::{BOX_VERTICES, RASTER_FLOATS};
    let RaySource::Light {
        light: l,
        starts,
        size,
        ..
    } = &observed.source
    else {
        unreachable!()
    };
    let size = *size;
    // The draw writes a word for each box, as many as its args count.
    let mut args = vec![0u32; raster::ARGS_WORDS as usize];
    args[raster::ARGS_VERTICES as usize] = raster::VERTICES;
    let args = gpu.buf(&args, store_of());
    let draws = gpu.buf(&vec![0u32; record::LIGHT_DRAWS as usize], store_of());
    let node_cache = gpu.buf(cache, store_of());
    let eye_buf = gpu.buf(&observed.eye, wgpu::BufferUsages::UNIFORM);
    let header_buf = gpu.buf(&observed.header, wgpu::BufferUsages::UNIFORM);
    let draw = gpu.pipeline(src(SHADER_RECORD_DRAW), "light_record_draw_cs");
    gpu.dispatch(
        &draw,
        &[
            (0, eye_buf.as_entire_binding()),
            (9, node_cache.as_entire_binding()),
            (11, header_buf.as_entire_binding()),
            (14, draws.as_entire_binding()),
            (16, args.as_entire_binding()),
        ],
        [node_cache::ENTRIES / record::DRAW_LANES, 1, 1],
    );
    let cnt = gpu.read(&args)[raster::ARGS_INSTANCES as usize].min(record::LIGHT_DRAWS);
    let cache = gpu.read(&node_cache);
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
                        observed.name,
                        raster::voxel_of(id)
                    );
                }
            }
        }
    }
    kind_cnt.count("the light's raster, texels a box's", entered);
    if kept == 0 || wrong != 0 || !edges_within(edges, kept) {
        failed.push(format!(
            "{}: the light's raster's {kept} texels kept, {wrong} by a box their ray does not \
             enter, {edges} at an edge",
            observed.name
        ));
    }
}

/// Runs the eye's hand-written record of its samples' stops over `words` from `origin`, in
/// frame `frame`, and then its draw. The record runs into the node cache `was` left, with each
/// node a sample stopped on kept as its walk would keep it (`lay_kept`). The result is checked
/// against `boxes_want` and pxl-engine `record`. Each such entry's state must be whole where
/// its ray stopped coarser than it asked. Its faces, entered and asked, must be those `was`
/// held and those its samples entered across, and it must be marked drawn in the frame where
/// it is drawn. Each box written must be a fresh drawable entry's, word for word
/// (`record::fresh`). Every fresh voxel in view (`record::in_view`) must be drawn, and the
/// draw must name the frame it ran in (`node_cache::DRAWN`). Returns the node cache after the
/// draw, and puts failures into `failed`.
#[expect(
    clippy::too_many_arguments,
    reason = "the device, the passes, its observer, words, origin and grid, what stood, what to tell and what failed"
)]
fn record_case(
    gpu: &Gpu,
    [pass, draw]: [&wgpu::ComputePipeline; 2],
    observed: &Observed,
    words: &[u32],
    (origin, grid): ([i32; 3], &Grid),
    (was, frame): (Option<&[u32]>, u32),
    kind_cnt: &mut KindCount,
    failed: &mut Vec<String>,
) -> Vec<u32> {
    use pxl_render::frame::shader::word::SQUARE_SIDE;
    let name = &observed.name;
    let want = boxes_want(observed, words, origin, Some(grid.tan), kind_cnt);
    let asked = raster::ASKED_AT as usize;
    let mut laid_args = vec![0u32; asked + raster::ARGS_WORDS as usize];
    laid_args[raster::ARGS_VERTICES as usize] = raster::VERTICES;
    laid_args[asked + raster::ARGS_FACES as usize + 1] = 1;
    laid_args[asked + raster::ARGS_FACES as usize + 2] = 1;
    laid_args[asked + raster::ARGS_ASKS as usize] = grid.asks_at;
    let mut rec = was.map_or_else(frame_node_cache, <[u32]>::to_vec);
    rec[node_cache::FRAME as usize] = frame;
    // Each node a taken sample stopped on is kept, as the walk that stopped it keeps it.
    for s in observed.taken() {
        let k = observed.sample_at(s) as usize;
        let node = k + beam::SAMPLE_NODE as usize;
        if words[node + beam::NODE_ZOOM as usize] == beam::SAMPLE_NONE {
            continue;
        }
        let n = node_of(&key_words(words, node));
        let lacks = words[k + beam::SAMPLE_LACKS as usize] & beam::SAMPLE_LACK != 0;
        let asset = [
            words[k + beam::SAMPLE_ASSET as usize],
            words[k + beam::SAMPLE_ASSET as usize + 1],
        ];
        let kind = if lacks { Kind::Lacks } else { Kind::Cell };
        lay_kept(&mut rec, origin, &n, (kind, asset));
    }
    let eye_buf = gpu.buf(&observed.eye, wgpu::BufferUsages::UNIFORM);
    let beams = gpu.buf(words, store_of());
    let args = gpu.buf(&laid_args, store_of());
    let rec_buf = gpu.buf(&rec, store_of());
    let boxes = gpu.buf(&vec![UNWRITTEN; record::EYE_DRAWS as usize], store_of());
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
    // The light's head says that no receiver stands this frame, so the draw must stand none
    // and must write none of the light's list, args and stood list. tests/light.rs checks what
    // it stands where receivers do stand.
    let size = SHADOW_SIZE;
    let mut light_header = vec![0u32; light::HEADER_WORDS];
    for (k, v) in light::header_layout(size) {
        light_header[k] = v;
    }
    light_header[light::HEADER_WORLD_RECEIVERS] = 0;
    let light_list: Vec<u32> = (0..light::list_words(size)).map(|k| k ^ 0x5a5a).collect();
    let light_args: Vec<u32> = (0..light::args_words(size)).map(|k| k ^ 0xa5a5).collect();
    let stood: Vec<u32> = (0..light::RECEIVER_TILES_WORDS)
        .map(|k| k ^ 0x3c3c)
        .collect();
    let light_header_buf = gpu.buf(&light_header, wgpu::BufferUsages::UNIFORM);
    let light_list_buf = gpu.buf(&light_list, store_of());
    let light_args_buf = gpu.buf(&light_args, store_of());
    let receiver_tiles_buf = gpu.buf(&stood, store_of());
    gpu.dispatch(
        draw,
        &[
            (0, eye_buf.as_entire_binding()),
            (9, rec_buf.as_entire_binding()),
            (11, light_header_buf.as_entire_binding()),
            (12, light_list_buf.as_entire_binding()),
            (14, boxes.as_entire_binding()),
            (16, args.as_entire_binding()),
            (21, light_args_buf.as_entire_binding()),
            (22, receiver_tiles_buf.as_entire_binding()),
        ],
        [node_cache::ENTRIES / record::DRAW_LANES, 1, 1],
    );
    let got = gpu.read(&rec_buf);
    let got_boxes = gpu.read(&boxes);
    let got_args = gpu.read(&args);
    let mut fail = |what: String| failed.push(format!("{name}: {what}"));
    if gpu.read(&light_list_buf) != light_list
        || gpu.read(&light_args_buf) != light_args
        || gpu.read(&receiver_tiles_buf) != stood
    {
        fail("the light's words written where no receiver stands".into());
    }
    if got[node_cache::DRAWN as usize] != frame {
        fail("the draw's frame not named".into());
    }

    let f = |k: usize| f32::from_bits(observed.eye[k]);
    let v3 = |k: usize| [f(k), f(k + 1), f(k + 2)];
    let axes = [v3(eye::RIGHT), v3(eye::UP), v3(eye::FORWARD)];
    let tan = [f(eye::TAN), f(eye::TAN + 1)];
    let range = [observed.eye[eye::RANGE], observed.eye[eye::RANGE + 1]];
    let far = f(eye::FAR);
    // Each drawable slot, keyed by its node as seen from this origin.
    let key_of = |e: usize| -> Key { node_words(&entry_node(&got, e, origin)) };
    let mut slots_of: BTreeMap<Key, Vec<u32>> = BTreeMap::new();
    for slot in 0..node_cache::ENTRIES {
        if holds_node(&got, slot) && got[node_cache::state_at(slot) as usize] != 0 {
            slots_of
                .entry(key_of(node_cache::entry_at(slot) as usize))
                .or_default()
                .push(slot);
        }
    }
    let (mut twice, mut entries_differ, mut none) = (0u64, 0u64, 0u64);
    for (key, w) in &want {
        let Some(slots) = slots_of.get(key) else {
            none += 1;
            continue;
        };
        twice += slots.len() as u64 - 1;
        // A key must be held in one slot alone, and the first is checked.
        let e = node_cache::entry_at(slots[0]) as usize;
        let key_word = node_cache::key_at(slots[0]) as usize;
        let state_word = node_cache::state_at(slots[0]) as usize;
        let node = node_of(key);
        let whole = record::ray_whole(origin, &node, range);
        // The state the slot held before, where it already held this entry.
        let state_was = was
            .filter(|r| {
                r[key_word] == got[key_word]
                    && (0..node_cache::ENTRY_WORDS as usize).all(|a| r[e + a] == got[e + a])
            })
            .map_or(0, |r| r[state_word]);
        // The eye's samples ask about every face they enter (`record::ASKED`).
        let faces = w[BOX_FACES as usize] & record::FACES;
        let state = state_was
            | faces
            | faces << record::ASKED_SHIFT
            | if whole { record::WHOLE } else { 0 };
        kind_cnt.add("voxels, stopped whole", whole);
        let alike = got[state_word] == state
            && node_cache::bucket_of(node.zoom, node.idx(origin))
                == slots[0] & !(node_cache::BUCKET - 1);
        if !alike {
            entries_differ += 1;
            if entries_differ <= 3 {
                eprintln!(
                    "DIFFERS {name}: entry {} state {:x}, want state {state:x}",
                    slots[0], got[state_word],
                );
            }
        }
    }
    // Each box must be a held voxel's, word for word.
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
        if got[node_cache::drawn_at(slot) as usize] != frame {
            differ += 1;
            eprintln!("DIFFERS {name}: box {place}'s entry not marked drawn");
        }
        let state = got[node_cache::state_at(slot) as usize];
        stale += u64::from(!record::fresh(
            origin,
            &node_of(&key),
            state & record::WHOLE != 0,
            range,
        ));
        let want_word = raster::draw_word(
            slot,
            w[BOX_ID as usize] & raster::LACK != 0,
            w[BOX_FACES as usize] & raster::EYED != 0,
            true,
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
    // Every fresh voxel in view must be drawn. A voxel not drawn counts as out of view only
    // where it lies outside the view grown by the device's rounding.
    let fresh_of = |key: &Key| {
        let state = slots_of
            .get(key)
            .map_or(0, |slots| got[node_cache::state_at(slots[0]) as usize]);
        record::fresh(origin, &node_of(key), state & record::WHOLE != 0, range)
    };
    let mut missed = 0u64;
    for (key, w) in &want {
        if drawn.contains(key) || !slots_of.contains_key(key) {
            continue;
        }
        if !fresh_of(key) {
            kind_cnt.add("voxels, stale, not drawn", true);
            continue;
        }
        let b = |k: u32| f32::from_bits(w[k as usize]);
        let lo = [0, 1, 2].map(|a| b(BOX_LO + a));
        let hi = [0, 1, 2].map(|a| b(BOX_HI + a));
        let grown = [tan[0] * 1.001, tan[1] * 1.001];
        let eyed = w[BOX_FACES as usize] & raster::EYED != 0;
        if eyed || record::in_view((lo, hi), axes, tan, far) {
            missed += 1;
        } else {
            kind_cnt.add(
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
    if none != 0 || twice != 0 || entries_differ != 0 || differ != 0 || stale != 0 || missed != 0 {
        fail(format!(
            "{none} voxels not held, {twice} held twice, {entries_differ} entries differ, \
             {differ} boxes differ, {stale} stale boxes, {missed} in view not drawn"
        ));
    }
    if same == 0 {
        fail("no box to compare".to_string());
    }
    let cnt = cnt.min(record::EYE_DRAWS);
    raster_drawn(
        gpu,
        observed,
        origin,
        (&got, &got_boxes[..cnt as usize]),
        kind_cnt,
        failed,
    );
    got
}

/// Runs the raster's draw (raster.wgsl) of `draws` over the node cache `cache`, in the view
/// `view` (`word::RASTER_FLOATS`), onto a target of `size` cleared to none, keeping the
/// nearest. Returns each texel's voxel's number (`raster::id_of`), 0 where it kept none.
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
    let view_buf = gpu.buf(view, wgpu::BufferUsages::UNIFORM);
    let draws_buf = gpu.buf(draws, wgpu::BufferUsages::STORAGE);
    let cache_buf = gpu.buf(cache, wgpu::BufferUsages::STORAGE);
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

/// Runs the raster's draw (raster.wgsl) of `draws`, the record's word for each box, over the
/// node cache `cache` it reads each box's entry from. It draws from the eye of `observed` at
/// `origin` onto a screen of its own. Every pixel that keeps a voxel must keep one whose box,
/// as the cast stops on its node (`cast::entry`), the pixel's own ray enters, but for a pixel
/// the device's rounding may decide otherwise (`at_edge`). Some pixel must keep a voxel.
fn raster_drawn(
    gpu: &Gpu,
    observed: &Observed,
    origin: [i32; 3],
    (cache, draws): (&[u32], &[u32]),
    kind_cnt: &mut KindCount,
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
    let f = |k: usize| f32::from_bits(observed.eye[k]);
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
        observed.eye[eye::FAR],
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
            let (ray, _) = px_aim(observed, &screen, [x, y]);
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
                        observed.name,
                        raster::voxel_of(id)
                    );
                }
            }
        }
    }
    kind_cnt.count("raster, pixels a box's", entered);
    if kept == 0 || wrong != 0 || !edges_within(edges, kept) {
        failed.push(format!(
            "{}: the raster's {kept} pixels kept, {wrong} by a box their ray does not enter, \
             {edges} at an edge",
            observed.name
        ));
    }
}

/// In one row of samples in eight, takes the node a zoom coarser at the same corner for each
/// sample whose words match the sample before it across, where the node's corner lies on the
/// coarser zoom's grid. The words a pixel reads then match but for the zoom. Returns how many
/// samples it changed.
fn coarser(observed: &Observed, words: &mut [u32]) -> u64 {
    let read = beam::SAMPLE_WORDS as usize;
    let mut cut = 0;
    for j in (0..observed.lattice[1]).step_by(8) {
        for i in 1..observed.lattice[0] {
            let (k, b) = (
                observed.sample_at([i, j]) as usize,
                observed.sample_at([i - 1, j]) as usize,
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

/// The node cache a frame's beam passes start over: empty but for the frame's number.
fn frame_node_cache() -> Vec<u32> {
    let mut t = vec![0u32; node_cache::WORDS as usize];
    t[node_cache::FRAME as usize] = FRAME;
    t
}

#[test]
fn hand_eye_record_answers_as_raster() {
    use pxl_render::frame::shader::record::{SHADER_RECORD_DRAW, SHADER_SAMPLE_HIT};
    let Some(gpu) = device() else { return };
    let built = real_world::build(EYE);
    let b = world_bufs(&gpu, &built);
    let at = spots(&built);
    let none = [Cut::NONE; 2];
    let over = |g: [i32; 2], up: i32| q8([g[0], g[1], built.top_at(g) + up]);
    let case = |name, origin, look, tan, far, range| EyeCase {
        name,
        origin,
        look,
        tan,
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
            2000.0,
            [8, FINEST],
        ),
        case(
            "the eye, over the seam",
            over([EYE[0] + 280, EYE[1] + 90], 40),
            [0.8, 0.2, -0.55],
            [0.4, 0.3],
            2000.0,
            [8, FINEST],
        ),
        case(
            "the eye, on a bridge",
            q8([at.bridge[0], at.bridge[1], at.bridge[2] + 12]),
            [0.5, -0.3, -0.8],
            [0.3, 0.3],
            600.0,
            [LEAF_CELL_ZOOM, FINEST],
        ),
        case(
            "the eye, down a bore, under the ground",
            q8([at.bore[0], at.bore[1], at.bore[2]]),
            [0.6, 0.2, -0.3],
            [0.3, 0.3],
            300.0,
            [LEAF_CELL_ZOOM, FINEST],
        ),
        case(
            "the eye, where no clip level holds the ground",
            q8([1 << 15, 1 << 15, 40]),
            [0.3, 0.2, -0.93],
            [0.4, 0.3],
            800.0,
            [8, FINEST],
        ),
        case(
            "the eye, high over the clip levels",
            over([EYE[0] + 2000, EYE[1] + 2000], 2500),
            [0.3, -0.2, -0.9],
            [0.3, 0.25],
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
            400.0,
            [LEAF_CELL_ZOOM, FINEST],
        )
    });
    let pass = gpu.pipeline(src(SHADER_SAMPLE_HIT), "sample_hit_cs");
    let draw = gpu.pipeline(src(SHADER_RECORD_DRAW), "record_draw_cs");
    let mut kind_cnt = KindCount::new();
    let mut failed = Vec::new();
    // With `coarse`, some samples' nodes are taken a zoom coarser (`coarser`).
    let mut run = |b: &WorldBuffer, c: &EyeCase, coarse: bool, kind_cnt: &mut KindCount| {
        let mut observed = eye_of(c, [0; 3]);
        let node_cache = gpu.buf(&frame_node_cache(), store_of());
        let lines = eye_beams(&gpu, b, &observed, &node_cache);
        let node_cache = gpu.read(&node_cache);
        let mut words = stopped(&gpu, b, &observed, &lines, None, &node_cache);
        let RaySource::Eye { origin, grid, .. } = &observed.source else {
            unreachable!()
        };
        let (origin, grid) = (*origin, *grid);
        if coarse {
            kind_cnt.count("alike but the zoom", coarser(&observed, &mut words));
            observed.name += ", a sample's node coarser at its corner";
        }
        // A frame records into an empty node cache, and the next frame records over what it
        // kept. The next frame must find the same voxels and write none of them again.
        let passes = [&pass, &draw];
        let first_frame = record_case(
            &gpu,
            passes,
            &observed,
            &words,
            (origin, &grid),
            (None, FRAME),
            kind_cnt,
            &mut failed,
        );
        observed.name += ", the next frame";
        record_case(
            &gpu,
            passes,
            &observed,
            &words,
            (origin, &grid),
            (Some(&first_frame), FRAME + 1),
            kind_cnt,
            &mut failed,
        );
        // Returns two points: two Q3 steps over the middle of the top of the nearest cell
        // drawn, and two under the middle of its bottom. Each lies within `raster::is_eyed`'s
        // reach at the screen's corner, and past that reach straight ahead.
        let boxes = boxes_want(&observed, &words, origin, None, &mut KindCount::new());
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
        run(&b, c, false, &mut kind_cnt);
    }
    run(&b, &cases[0], true, &mut kind_cnt);
    // An eye by a cell of the bridge it saw, looking at it, so that the cell's box lies about
    // the eye.
    let [over_top, under] = run(&b, &cases[2], false, &mut KindCount::new()).expect("a cell drawn");
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
            300.0,
            [LEAF_CELL_ZOOM, FINEST],
        );
        run(&b, &c, false, &mut kind_cnt);
    }
    // Over X = 0, where one root ends and the next begins in X, some boxes lie in the next
    // root.
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
        2000.0,
        [8, FINEST],
    );
    run(&bz, &c, false, &mut kind_cnt);
    eprintln!("  KINDS eye boxes {kind_cnt:?}");
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
        if kind_cnt.get(what) == 0 {
            failed.push(format!("none of {what} to say"));
        }
    }
    if kind_cnt.get("boxes, an inset's product rounded") != 0 {
        failed
            .push("an inset's product rounded, which a fused sum may round otherwise".to_string());
    }
    assert!(failed.is_empty(), "{}", failed.join("\n"));
}

/// The draw must write no box for a stale voxel (pxl-engine `record`, "Fresh or stale"), even
/// one that lies about the eye, whose box would be drawn over the whole screen. The record is
/// the one a cast from over the ground wrote. The draw then runs alone from the middle of each
/// coarse voxel's box, where the eye asks finer than the voxel, so the voxel is stale
/// (`record::fresh`) and its box lies about the eye (`raster::is_eyed`).
#[test]
fn hand_eye_stale_box_about_the_eye_undrawn() {
    use pxl_render::frame::shader::record::{SHADER_RECORD_DRAW, SHADER_SAMPLE_HIT};
    let Some(gpu) = device() else { return };
    let built = real_world::build(EYE);
    let b = world_bufs(&gpu, &built);
    let over = |g: [i32; 2], up: i32| q8([g[0], g[1], built.top_at(g) + up]);
    let case = |name, origin| EyeCase {
        name,
        origin,
        look: [0.5, 0.4, -0.77],
        tan: [0.3, 0.2],
        far: 2000.0,
        range: [8, FINEST],
        cuts: [Cut::NONE; 2],
    };
    let pass = gpu.pipeline(src(SHADER_SAMPLE_HIT), "sample_hit_cs");
    let draw = gpu.pipeline(src(SHADER_RECORD_DRAW), "record_draw_cs");
    let mut kind_cnt = KindCount::new();
    let mut failed = Vec::new();
    let far_eye = case(
        "the eye, over the ground",
        over([EYE[0] + 90, EYE[1] + 90], 60),
    );
    let observed = eye_of(&far_eye, [0; 3]);
    let node_cache = gpu.buf(&frame_node_cache(), store_of());
    let lines = eye_beams(&gpu, &b, &observed, &node_cache);
    let node_cache = gpu.read(&node_cache);
    let words = stopped(&gpu, &b, &observed, &lines, None, &node_cache);
    let RaySource::Eye { origin, grid, .. } = &observed.source else {
        unreachable!()
    };
    let first_frame = record_case(
        &gpu,
        [&pass, &draw],
        &observed,
        &words,
        (*origin, grid),
        (None, FRAME),
        &mut kind_cnt,
        &mut failed,
    );
    assert!(failed.is_empty(), "{}", failed.join("\n"));

    // The coarsest drawn cells that are not whole, each with the middle of its box for an eye
    // to stand at.
    let q = (1 << FRAC_W) as f32;
    let mut inside: Vec<(u32, [i32; 3])> = (0..node_cache::ENTRIES)
        .filter_map(|slot| {
            let e = node_cache::entry_at(slot) as usize;
            let state = first_frame[node_cache::state_at(slot) as usize];
            let kind = first_frame[e + node_cache::W_KIND as usize] & node_cache::KIND_MASK;
            if !holds_node(&first_frame, slot)
                || state == 0
                || kind != Kind::Cell as u32
                || state & record::WHOLE != 0
            {
                return None;
            }
            let node = entry_node(&first_frame, e, *origin);
            if node.zoom + 2 > FINEST {
                return None;
            }
            let asset = first_frame[e + node_cache::W_ASSET as usize];
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
        let node = entry_node(&first_frame, e, at);
        let asset = first_frame[e + node_cache::W_ASSET as usize];
        let (lo, hi) = cast::asset_box(at, &node, asset);
        if record::fresh(at, &node, false, far_eye.range) || !raster::is_eyed(lo, hi, far_eye.tan) {
            continue;
        }
        tried += 1;
        let mut rec = first_frame.clone();
        rec[node_cache::FRAME as usize] = FRAME + 1;
        let asked = raster::ASKED_AT as usize;
        let mut laid_args = vec![0u32; asked + raster::ARGS_WORDS as usize];
        laid_args[raster::ARGS_VERTICES as usize] = raster::VERTICES;
        let eye_buf = gpu.buf(&near.eye, wgpu::BufferUsages::UNIFORM);
        let args = gpu.buf(&laid_args, store_of());
        let rec_buf = gpu.buf(&rec, store_of());
        let boxes = gpu.buf(&vec![UNWRITTEN; record::EYE_DRAWS as usize], store_of());
        let size = SHADOW_SIZE;
        let mut light_header = vec![0u32; light::HEADER_WORDS];
        for (k, v) in light::header_layout(size) {
            light_header[k] = v;
        }
        light_header[light::HEADER_WORLD_RECEIVERS] = 0;
        let light_header_buf = gpu.buf(&light_header, wgpu::BufferUsages::UNIFORM);
        let light_list_buf = gpu.buf(&vec![0u32; light::list_words(size) as usize], store_of());
        let light_args_buf = gpu.buf(&vec![0u32; light::args_words(size) as usize], store_of());
        let receiver_tiles_buf = gpu.buf(
            &vec![0u32; light::RECEIVER_TILES_WORDS as usize],
            store_of(),
        );
        gpu.dispatch(
            &draw,
            &[
                (0, eye_buf.as_entire_binding()),
                (11, light_header_buf.as_entire_binding()),
                (12, light_list_buf.as_entire_binding()),
                (14, boxes.as_entire_binding()),
                (16, args.as_entire_binding()),
                (21, light_args_buf.as_entire_binding()),
                (22, receiver_tiles_buf.as_entire_binding()),
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
    use pxl_render::frame::shader::record::SHADER_LIGHT_SAMPLE_HIT;
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
        one_in: None,
    };
    let cases = [
        LightCase { ..about },
        LightCase {
            name: "the light, its tiles listed",
            corner: [1304, -896],
            one_in: Some(3),
            ..about
        },
        LightCase {
            name: "the light, every tile listed, the list's row full",
            corner: [1304, -896],
            one_in: Some(1),
            ..about
        },
        LightCase {
            name: "the light, its tiles listed, the shipped map's size",
            side: 0.5,
            corner: [1304, -896],
            size: SHADOW_SIZE,
            one_in: Some(23),
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
    let pass = gpu.pipeline(src(SHADER_LIGHT_SAMPLE_HIT), "sample_hit_cs");
    let mut kind_cnt = KindCount::new();
    let mut failed = Vec::new();
    let mut run = |b: &WorldBuffer, c: &LightCase, kind_cnt: &mut KindCount| {
        let mut observed = light_of(c, [0, 0]);
        let node_cache = gpu.buf(&frame_node_cache(), store_of());
        let lines = light_beams(&gpu, b, &mut observed, &node_cache, c.one_in);
        let node_cache = gpu.read(&node_cache);
        // Where the cast lists its tiles, each sample is first laid as the whole cast of the
        // same box left it, and a sample not taken must keep what it held.
        let prior = c.one_in.map(|_| {
            let whole = LightCase { one_in: None, ..*c };
            let mut o = light_of(&whole, [0, 0]);
            let t = gpu.buf(&frame_node_cache(), store_of());
            let lines = light_beams(&gpu, b, &mut o, &t, None);
            let t = gpu.read(&t);
            stopped(&gpu, b, &o, &lines, None, &t)
        });
        let words = stopped(&gpu, b, &observed, &lines, prior.as_deref(), &node_cache);
        // The record pass dispatches over the rows the list's entries open, as the sample pass
        // does.
        let groups = observed.groups;
        kind_cnt.add("listed", c.one_in.is_some());
        // The light asks nothing, since its slots read no faces' words.
        light_record_case(
            &gpu,
            &pass,
            &observed,
            &words,
            groups,
            FRAME,
            kind_cnt,
            &mut failed,
        );
    };
    for c in &cases {
        run(&b, c, &mut kind_cnt);
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
        range: [LEAF_CELL_ZOOM, LEAF_CELL_ZOOM],
        ..about
    };
    run(&bz, &c, &mut kind_cnt);
    eprintln!("  KINDS light record {kind_cnt:?}");
    for what in [
        "voxels",
        "voxels, lacks",
        "voxels, short of its cube",
        "voxels, in another root",
        "listed",
    ] {
        if kind_cnt.get(what) == 0 {
            failed.push(format!("none of {what} to say"));
        }
    }
    if kind_cnt.get("boxes, an inset's product rounded") != 0 {
        failed
            .push("an inset's product rounded, which a fused sum may round otherwise".to_string());
    }
    assert!(failed.is_empty(), "{}", failed.join("\n"));
}

/// The beam rays' run (corner.wgsl over beam.eye.wgsl) over a node cache that holds part of
/// what the beam rays meet. A beam ray that its walks over the node cache park must go on from
/// the state it was parked in. Every corner must stop where the walk from the root stops the
/// same ray, at the same node, and must hand on the same distance (`Grid::hand_dist`). The
/// first frame runs over an empty node cache, and the next over the first frame's with every
/// other entry finer than `DROP_ZOOM` dropped.
#[test]
fn hand_beam_rays_answer_as_cast_walk() {
    const DROP_ZOOM: Zoom = 12;
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
            far: 2000.0,
            range: [8, FINEST],
            cuts: none,
        },
        EyeCase {
            name: "the eye, over the seam",
            origin: over([EYE[0] + 280, EYE[1] + 90], 40),
            look: [0.8, 0.2, -0.55],
            tan: [0.4, 0.3],
            far: 2000.0,
            range: [8, FINEST],
            cuts: none,
        },
        EyeCase {
            name: "the eye, on a bridge",
            origin: q8([at.bridge[0], at.bridge[1], at.bridge[2] + 12]),
            look: [0.5, -0.3, -0.8],
            tan: [0.3, 0.3],
            far: 600.0,
            range: [LEAF_CELL_ZOOM, FINEST],
            cuts: none,
        },
    ];
    let mut failed = Vec::new();
    for c in &cases {
        let observed = eye_of(c, [0; 3]);
        let RaySource::Eye {
            origin,
            axes,
            far,
            range,
            grid,
        } = &observed.source
        else {
            unreachable!()
        };
        let detail = Detail {
            range: *range,
            spacing: Spacing {
                angle: CELL,
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
        let node_cache = gpu.buf(&frame_node_cache(), store_of());
        let first = eye_beams(&gpu, &b, &observed, &node_cache);
        // Every other entry finer than `DROP_ZOOM` is dropped, so the next frame's walk over the
        // node cache leaves its rays part of the way down.
        let mut cache = gpu.read(&node_cache);
        cache[node_cache::FRAME as usize] = FRAME + 1;
        for e in (0..node_cache::ENTRIES).step_by(2) {
            let w = node_cache::entry_at(e) as usize;
            if cache[w + node_cache::W_NODE as usize] >= DROP_ZOOM {
                cache[node_cache::key_at(e) as usize] = 0;
            }
        }
        let node_cache = gpu.buf(&cache, store_of());
        let next = eye_beams(&gpu, &b, &observed, &node_cache);
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
    fn keep(&mut self, trace: &Trace<Inheritance>, stack: &[Inheritance; DEPTH]) {
        let z = trace.hit.node.zoom as usize;
        self.0 = stack[..z].iter().copied().chain([trace.hit.at]).collect();
    }
}

/// The beam rays' run for the light's map (corner.wgsl over beam.light.wgsl) over a node cache
/// that holds part of what its beam rays meet. Every beam ray must stop where the walk from the
/// root stops the same ray from the origin the pass laid, at the same node. It must hand on the
/// same distance (`light::margin`) under either rounding a device may take (`margins`), and its
/// line must hold what that walk handed the node and every node above it. The first frame runs
/// over an empty node cache, and the next over the first frame's with every other entry finer
/// than `DROP_ZOOM` dropped.
#[test]
fn hand_light_beam_rays_answer_as_cast_walk() {
    const DROP_ZOOM: Zoom = 12;
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
            one_in: None,
        },
        LightCase {
            name: "the light, narrow and slanting",
            origin: over([EYE[0] + 40, EYE[1] - 20], 25),
            dir: [0.6, -0.3, -0.74],
            side: 0.37,
            back: 120.0,
            t_max: 500.0,
            range: [LEAF_CELL_ZOOM, FINEST],
            corner: [-16, 32],
            cuts: none,
            size: SIZE,
            one_in: None,
        },
    ];
    let mut failed = Vec::new();
    for c in &cases {
        let mut observed = light_of(c, [0, 0]);
        let node_cache = gpu.buf(&frame_node_cache(), store_of());
        let first = light_beams(&gpu, &b, &mut observed, &node_cache, None);
        let mut cache = gpu.read(&node_cache);
        cache[node_cache::FRAME as usize] = FRAME + 1;
        for e in (0..node_cache::ENTRIES).step_by(2) {
            let at = node_cache::entry_at(e) as usize;
            if cache[at + node_cache::W_NODE as usize] >= DROP_ZOOM {
                cache[node_cache::key_at(e) as usize] = 0;
            }
        }
        let node_cache = gpu.buf(&cache, store_of());
        let next = light_beams(&gpu, &b, &mut observed, &node_cache, None);
        let RaySource::Light {
            light: l,
            range,
            size,
            ..
        } = &observed.source
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
                let mut inheritance = Handed(Vec::new());
                let ray = Ray::new(l.dir, l.t_max);
                let m =
                    cast::walk_from_root(&mut q, o, &ray, detail, Until::Beam, &mut inheritance);
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
                    && inheritance.0.iter().enumerate().all(|(z, h)| {
                        let g = [0, 1, 2]
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
/// left there (walk.wgsl, cached.wgsl). The samples' run must go on from where it was left, and
/// every sample must stop where the walk from the root stops it. The eye's first frame asks
/// coarse zooms. The next frame asks finer, over the node cache the first left, with every
/// other entry finer than `DROP_ZOOM` dropped.
#[test]
fn hand_samples_over_other_zooms_answer_as_cast_walk() {
    const DROP_ZOOM: Zoom = 12;
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
        far: 2000.0,
        range,
        cuts: [Cut::NONE; 2],
    };
    let mut kind_cnt = KindCount::new();
    let mut failed = Vec::new();
    let coarse = eye_of(&case([8, 16]), [0; 3]);
    let node_cache = gpu.buf(&frame_node_cache(), store_of());
    let words = eye_beams(&gpu, &b, &coarse, &node_cache);
    let after_beam = gpu.read(&node_cache);
    let mut cache = run_frame(
        &gpu,
        &b,
        w,
        &coarse,
        &words,
        &after_beam,
        None,
        "coarse",
        &mut kind_cnt,
        &mut failed,
    );
    cache[node_cache::FRAME as usize] = FRAME + 1;
    for e in (0..node_cache::ENTRIES).step_by(2) {
        let at = node_cache::entry_at(e) as usize;
        if cache[at + node_cache::W_NODE as usize] >= DROP_ZOOM {
            cache[node_cache::key_at(e) as usize] = 0;
        }
    }
    let fine = eye_of(&case([8, FINEST]), [0; 3]);
    let node_cache = gpu.buf(&cache, store_of());
    let words = eye_beams(&gpu, &b, &fine, &node_cache);
    let after_beam = gpu.read(&node_cache);
    run_frame(
        &gpu,
        &b,
        w,
        &fine,
        &words,
        &after_beam,
        None,
        "fine",
        &mut kind_cnt,
        &mut failed,
    );
    assert!(failed.is_empty(), "{}", failed.join("\n"));
}
