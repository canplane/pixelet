//! `sample_hit_cs` and `sample_cs` by hand (frame/shader/sample_hit.wgsl, sample.wgsl), the eye's
//! and the light's (sample_eye.wgsl, sample_light.wgsl), answer as `cast::walk` from the root
//! answers the same ray: over the real world of tests/real_world, off what the beam's passes
//! leave there, each sample's stop as the Rust's, and the next frame off the node cache the first
//! left. The missed pixels' walks and the boxes the samples draw answer alike. Skipped where no
//! adapter answers.

mod gpu;
mod real_world;

use std::collections::{BTreeMap, BTreeSet};

use gpu::{cross, margins, read_texture, texture, unit, Gpu, Kinds, LightBox};
use pxl_engine::answer;
use pxl_engine::area::Shard;
use pxl_engine::beam::{self, Grid};
use pxl_engine::cast::{self, Detail, End, Keep, Node, Ray, Spacing, Trace, Until, DEPTH};
use pxl_engine::clipmap::Lookup;
use pxl_engine::light::{self, Light, Starts};
use pxl_engine::node_cache;
use pxl_engine::overwrite::{Cut, NodeSlot};
use pxl_engine::param;
use pxl_engine::world::Inheritance;
use pxl_engine::{eye, raster, ray, value, Zoom, FRAC_W, LEAF_CELL};
use pxl_render::frame::shader::record::{
    src, SHADER_BEAM, SHADER_BEAM_HIT, SHADER_CELL, SHADER_LIGHT_CELL, SHADER_LIGHT_SAMPLE,
    SHADER_LIGHT_SAMPLE_HIT, SHADER_SAMPLE, SHADER_SAMPLE_HIT,
};
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
    lefts_at: u32,
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
        lefts_at: grid.lefts_at,
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

/// The eye's beam passes over `obs`: the hit, then what it left, then the cells' starts.
fn eye_beams(gpu: &Gpu, b: &WorldBufs, obs: &Observed, table: &wgpu::Buffer) -> Vec<u32> {
    let Rays::Eye { grid, .. } = &obs.rays else {
        unreachable!()
    };
    let eye = gpu.buffer(&obs.eye, wgpu::BufferUsages::UNIFORM);
    let head = gpu.buffer(&obs.head, wgpu::BufferUsages::UNIFORM);
    let beams = gpu.buffer(&obs.words, store_of());
    let left = gpu.buffer(&[0, 1, 1, 0], store_of());
    gpu.dispatch(
        &gpu.pipeline(src(SHADER_BEAM_HIT), "beam_hit_cs"),
        &[
            (0, eye.as_entire_binding()),
            (8, beams.as_entire_binding()),
            (9, table.as_entire_binding()),
            (10, left.as_entire_binding()),
        ],
        [
            grid.corners[0].div_ceil(WORKGROUP),
            grid.corners[1].div_ceil(WORKGROUP),
            1,
        ],
    );
    let args = gpu.read(&left);
    let mut words = gpu.read(&beams);
    words[grid.corners_at as usize] = args[3];
    let beams = gpu.buffer(&words, store_of());
    let world = world_binds(b, &eye, &beams, table);
    gpu.dispatch(
        &gpu.pipeline(src(SHADER_BEAM), "beam_cs"),
        &world,
        [args[0].max(1), 1, 1],
    );
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
        lefts_at: light::lefts_at(c.size),
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
    // And their squares, which a beam ray's corner reads where it feeds many tiles.
    let coarse_at = light::coarse_at(size) as usize;
    let squares = light::coarse(size) * light::coarse(size);
    list[coarse_at..coarse_at + squares as usize].fill(CAST);
    let beams = gpu.buffer(&list, store_of());
    let mut world = world_binds(b, &eye, &beams, table);
    world.push((11, head.as_entire_binding()));
    gpu.light_beam(&world, &beams, size);
    let cells = light::cells_across(size);
    gpu.dispatch(
        &gpu.pipeline(src(SHADER_LIGHT_CELL), "light_cell_cs"),
        &[
            (0, eye.as_entire_binding()),
            (8, beams.as_entire_binding()),
            (11, head.as_entire_binding()),
        ],
        [cells.div_ceil(WORKGROUP), cells.div_ceil(WORKGROUP), 1],
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
        // Tiles feeding one in `every` of the box, a tile in nine of them read, the box's edges
        // among them.
        obs.head[light::HEAD_EPOCH] = CAST - 3;
        let side = light::TILE / param::light::SAMPLE;
        let at = light::listed_at(size, light::LISTED_FED) as usize;
        let mut entries = Vec::new();
        for j in 0..tiles {
            for i in 0..tiles {
                if (i + 2 * j).is_multiple_of(every) || i == 0 || j == tiles - 1 {
                    entries.push(j << 16 | i);
                }
                let read = (i % 5 == 1 && j % 4 == 2)
                    || (i == 0 && (j == 0 || j == 13))
                    || (i == tiles - 1 && j == tiles - 1);
                reads[(j * tiles + i) as usize] = read;
                words[reads_at + (j * tiles + i) as usize] = if read { CAST } else { CAST - 1 };
            }
        }
        // In reverse, and the room past them an unlisted tile's: a read past the list, or a
        // place over, walks what none takes.
        entries.reverse();
        words[at] = entries.len() as u32;
        words[at + 1..at + 1 + entries.len()].copy_from_slice(&entries);
        let room = light::listed_room(size, light::LISTED_FED) as usize;
        if entries.len() < room {
            let unlisted = 1;
            assert!(!entries.contains(&unlisted));
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

/// One frame over `table0`: the hit pass, then the walk of the samples it left, each sample's
/// stop against `cast::walk`'s for its ray, and no sample put that no thread takes. The table
/// the passes left.
#[expect(
    clippy::too_many_arguments,
    reason = "a frame's device, world, observer, words, table, what it tells and its passes"
)]
fn run_frame(
    gpu: &Gpu,
    b: &WorldBufs,
    w: World,
    obs: &Observed,
    words: &[u32],
    table0: &[u32],
    what: &str,
    kinds: &mut Kinds,
    failed: &mut Vec<String>,
    hit: &wgpu::ComputePipeline,
    world: &wgpu::ComputePipeline,
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
    let left = gpu.buffer(&[0, 1, 1, 0], store_of());
    let mut binds = vec![
        (0, eye_buf.as_entire_binding()),
        (8, beams.as_entire_binding()),
        (9, table.as_entire_binding()),
        (10, left.as_entire_binding()),
    ];
    if obs.apart {
        binds.push((11, head_buf.as_entire_binding()));
    }
    gpu.dispatch(hit, &binds, [obs.groups[0], obs.groups[1], 1]);
    let args = gpu.read(&left);
    let mut after = gpu.read(&beams);
    after[obs.lefts_at as usize] = args[3];
    let beams = gpu.buffer(&after, store_of());
    let mut binds = world_binds(b, &eye_buf, &beams, &table);
    if obs.apart {
        binds.push((11, head_buf.as_entire_binding()));
    }
    if args[3] != 0 {
        gpu.dispatch(world, &binds, [args[0], 1, 1]);
    }
    let out = gpu.read(&beams);

    // Each sample's stop: a needed one's as its ray's walk answers, one not needed none.
    let (mut same, mut differ, mut axis_over) = (0u64, 0u64, 0u64);
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
        "{name}: {same} alike ({axis_over} an axis over), {differ} differ of {} samples, {stray} stray",
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
    let (hit, world) = if first.apart {
        (
            gpu.pipeline(src(SHADER_LIGHT_SAMPLE_HIT), "sample_hit_cs"),
            gpu.pipeline(src(SHADER_LIGHT_SAMPLE), "sample_cs"),
        )
    } else {
        (
            gpu.pipeline(src(SHADER_SAMPLE_HIT), "sample_hit_cs"),
            gpu.pipeline(src(SHADER_SAMPLE), "sample_cs"),
        )
    };
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
        "first frame",
        kinds,
        failed,
        &hit,
        &world,
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
        "next frame",
        kinds,
        failed,
        &hit,
        &world,
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
            corner: [1300, -900],
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
            corner: [1300, -900],
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
            corner: [77, 5],
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
        corner: [1300, -900],
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
        corner: [1300, -900],
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
/// across and down, the eye's jitter, one in so many marked, and how many of those the cast
/// listed before its list was full.
struct Screen {
    name: &'static str,
    size: [u32; 2],
    jitter: [f32; 2],
    marked_in: u32,
    listed: u32,
}

/// The tangents across and up of pixel `p`'s ray through its jittered middle, row 0 at the
/// top: the rule every pass that walks a drawn pixel takes it by (grid.wgsl `px_tan`).
fn px_tan(s: &Screen, tan: [f32; 2], p: [u32; 2]) -> [f32; 2] {
    let ndc_x = (p[0] as f32 + 0.5 + s.jitter[0]) / s.size[0] as f32 * 2.0 - 1.0;
    let ndc_y = 1.0 - (p[1] as f32 + 0.5 + s.jitter[1]) / s.size[1] as f32 * 2.0;
    [ndc_x * tan[0], ndc_y * tan[1]]
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

#[test]
fn hand_missed_pixels_answer_as_cast_walk() {
    use pxl_render::frame::shader::record::{SHADER_MISS, SHADER_MISS_HIT};
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
            listed: u32::MAX,
        },
        Screen {
            name: "one in three marked, jittered, the list full past a third",
            size: [131, 77],
            jitter: [0.25, -0.375],
            marked_in: 3,
            listed: 1000,
        },
    ];
    let hit_p = gpu.pipeline(src(SHADER_MISS_HIT), "miss_hit_cs");
    let list_p = gpu.pipeline(src(SHADER_MISS), "miss_list_cs");
    let whole_p = gpu.pipeline(src(SHADER_MISS), "miss_cs");
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
            let words = eye_beams(&gpu, &b, &obs, &table_buf);
            let mut table = gpu.read(&table_buf);
            let Rays::Eye { origin, grid, .. } = &obs.rays else {
                unreachable!()
            };
            let (origin, grid) = (*origin, *grid);

            // The pixels the cast marked, and those it listed, in an order of their own as its
            // atomics take them.
            let [wd, ht] = s.size;
            let texels = (wd * ht) as usize;
            let mark = |p: u32| (p.wrapping_mul(0x9e37_79b9) >> 7).is_multiple_of(s.marked_in);
            let mut marked: Vec<u32> = (0..wd * ht).filter(|&p| mark(p)).collect();
            marked.sort_by_key(|&p| p.wrapping_mul(0x85eb_ca77).rotate_left(13));
            let listed = marked.len().min(s.listed as usize);
            table[node_cache::MARKS_CNT as usize] = listed as u32;
            for (k, &p) in marked[..listed].iter().enumerate() {
                table[(node_cache::MARKS_AT + k as u32) as usize] = ((p / wd) << 16) | (p % wd);
            }
            let mut ray_words: Vec<u32> = (0..ray::WORDS as usize * texels)
                .map(|k| (k as u32).wrapping_mul(0x2c1b_3c6d) >> 2)
                .collect();
            for &p in &marked {
                ray_words[p as usize] = ray::MISS;
            }

            // The walk over the node cache alone, what it left listed, then the walks that ask.
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
            let beams = gpu.buffer(&words, store_of());
            let table_buf = gpu.buffer(&table, store_of());
            let left = gpu.buffer(&[ray::MISS_ACROSS, 0, 1, 0, 0, 1, 1, 0], store_of());
            gpu.dispatch(
                &hit_p,
                &[
                    (0, eye_buf.as_entire_binding()),
                    (3, wgpu::BindingResource::TextureView(&view)),
                    (8, beams.as_entire_binding()),
                    (9, table_buf.as_entire_binding()),
                    (10, left.as_entire_binding()),
                ],
                [
                    ray::MISS_ACROSS,
                    (listed as u32)
                        .div_ceil(ray::MISS_ACROSS * ray::MISS_LANES)
                        .max(1),
                    1,
                ],
            );
            let args = gpu.read(&left);
            let mut after = gpu.read(&beams);
            after[grid.misses_at as usize] = args[3];
            let mut table = gpu.read(&table_buf);
            table[node_cache::HITS_ARGS as usize..(node_cache::HITS_CNT + 1) as usize]
                .copy_from_slice(&args[4..8]);
            let beams = gpu.buffer(&after, store_of());
            let table_buf = gpu.buffer(&table, store_of());
            let mut binds = world_binds(&b, &eye_buf, &beams, &table_buf);
            binds.push((3, wgpu::BindingResource::TextureView(&view)));
            if args[1] != 0 {
                gpu.dispatch(&list_p, &binds, [args[0], args[1], 1]);
            }
            gpu.dispatch(&whole_p, &binds, [wd.div_ceil(8), ht.div_ceil(8), 1]);
            let out = read_texture(&gpu.device, &gpu.queue, &rays);

            // Each marked pixel's stop as `cast::walk` from the root answers its ray.
            let (mut same, mut differ, mut stepped) = (0u32, 0u32, 0u32);
            for &p in &marked {
                let px = [p % wd, p / wd];
                let (r, d) = px_aim(&obs, s, px);
                let met = walk_from(w, obs.cuts, origin, &r, d);
                let got = |l: u32| out[l as usize * texels + p as usize];
                let face = 2 * met.hit.axis + u32::from(r.dir[met.hit.axis as usize] > 0.0);
                let mut ok = got(0) != ray::MISS;
                if met.ends == End::Hit {
                    ok = ok
                        && got(0) == ray::first_word(met.hit.asset[0], face, met.hit.node.zoom)
                        && got(2) == met.hit.asset[1];
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
                            met.ends, met.hit.node.zoom, met.hit.node.corner, met.hit.t, met.hit.axis, met.hit.asset,
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

/// A word the boxes' writing leaves standing where it writes none.
const UNWRITTEN: u32 = 0x5eed_0b0c;

/// The samples' words as the hand's hit pass and the walk after it leave them over `table`,
/// each sample's words laid first as a cast before left them in `prior`, else `UNTOUCHED`.
#[expect(
    clippy::too_many_arguments,
    reason = "the device, the world, the observer, its words, what stood before, the table and the passes"
)]
fn stopped(
    gpu: &Gpu,
    b: &WorldBufs,
    obs: &Observed,
    words: &[u32],
    prior: Option<&[u32]>,
    table: &[u32],
    hit: &wgpu::ComputePipeline,
    world: &wgpu::ComputePipeline,
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
    let left = gpu.buffer(&[0, 1, 1, 0], store_of());
    let mut binds = vec![
        (0, eye_buf.as_entire_binding()),
        (8, beams.as_entire_binding()),
        (9, table.as_entire_binding()),
        (10, left.as_entire_binding()),
    ];
    if obs.apart {
        binds.push((11, head_buf.as_entire_binding()));
    }
    gpu.dispatch(hit, &binds, [obs.groups[0], obs.groups[1], 1]);
    let args = gpu.read(&left);
    let mut after = gpu.read(&beams);
    after[obs.lefts_at as usize] = args[3];
    let beams = gpu.buffer(&after, store_of());
    let mut binds = world_binds(b, &eye_buf, &beams, &table);
    if obs.apart {
        binds.push((11, head_buf.as_entire_binding()));
    }
    if args[3] != 0 {
        gpu.dispatch(world, &binds, [args[0], 1, 1]);
    }
    gpu.read(&beams)
}

/// A box as pxl-engine `raster` has it written (`raster::BOX_WORDS`).
type BoxWords = [u32; raster::BOX_WORDS as usize];

/// Each box the samples' words `words` draw from `origin`, by its sample's id
/// (`raster::id_of`): a sample that stopped, unless the one across or up before it that the
/// cast needs stopped alike (`raster::alike`), which one before is asked and in what order
/// this test's own reading of boxes.wgsl `sample_box`; the cell as the cast stops on it
/// (`cast::asset_box`), and `raster::EYED` where `tan` is the screen's and it lies about the
/// eye (`raster::is_eyed`).
fn boxes_want(
    obs: &Observed,
    words: &[u32],
    origin: [i32; 3],
    tan: Option<[f32; 2]>,
    kinds: &mut Kinds,
) -> BTreeMap<u32, BoxWords> {
    let mut want = BTreeMap::new();
    let n = obs.lattice;
    let alike = |a: usize, b: usize| raster::alike(words, a, b);
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
        // A sample before that the cast does not need is not compared, whatever it holds.
        for b in [[s[0].wrapping_sub(1), s[1]], [s[0], s[1].wrapping_sub(1)]] {
            if b[0] < n[0] && b[1] < n[1] && !obs.needed(b) {
                add(
                    "a sample before not needed, alike as it stood",
                    alike(k, obs.sample_at(b) as usize),
                );
            }
        }
        let across = s[0] > 0 && obs.needed([s[0] - 1, s[1]]);
        if across && alike(k, obs.sample_at([s[0] - 1, s[1]]) as usize) {
            add("alike the one across", true);
            continue;
        }
        let up = s[1] > 0 && obs.needed([s[0], s[1] - 1]);
        if up && alike(k, obs.sample_at([s[0], s[1] - 1]) as usize) {
            add("alike the one up", true);
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
        let id = raster::id_of(s[1] * n[0] + s[0], lacks);
        let mut w = [0u32; raster::BOX_WORDS as usize];
        for a in 0..3 {
            w[(raster::BOX_LO as usize) + a] = lo[a].to_bits();
            w[(raster::BOX_HI as usize) + a] = hi[a].to_bits();
        }
        w[raster::BOX_ID as usize] = id;
        let axis = words[k + beam::SAMPLE_AXIS as usize];
        w[raster::BOX_AXIS as usize] = axis | if eyed { raster::EYED } else { 0 };
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
        want.insert(id, w);
    }
    want
}

/// The boxes' writing by hand over `words` from `origin` against `boxes_want`: as many boxes,
/// each once, word for word. Fails into `failed`.
#[expect(
    clippy::too_many_arguments,
    reason = "the device, the pass, its observer, words, origin and screen, what to tell and what failed"
)]
fn boxes_case(
    gpu: &Gpu,
    pass: &wgpu::ComputePipeline,
    obs: &Observed,
    words: &[u32],
    origin: [i32; 3],
    grid: Option<&Grid>,
    groups: [u32; 2],
    kinds: &mut Kinds,
    failed: &mut Vec<String>,
) {
    let name = &obs.name;
    let want = boxes_want(obs, words, origin, grid.map(|g| g.tan), kinds);
    let samples = obs.lattice[0] * obs.lattice[1];
    let mut laid_args = vec![0u32; raster::ARGS_WORDS as usize];
    laid_args[raster::ARGS_VERTICES as usize] = raster::VERTICES;
    if let Some(g) = grid {
        laid_args[raster::ARGS_FACES as usize + 1] = 1;
        laid_args[raster::ARGS_FACES as usize + 2] = 1;
        laid_args[raster::ARGS_ASKS as usize] = g.asks_at;
    }
    let eye_buf = gpu.buffer(&obs.eye, wgpu::BufferUsages::UNIFORM);
    let head_buf = gpu.buffer(&obs.head, wgpu::BufferUsages::UNIFORM);
    let beams = gpu.buffer(words, store_of());
    let boxes = gpu.buffer(
        &vec![UNWRITTEN; raster::words(samples) as usize],
        store_of(),
    );
    let args = gpu.buffer(&laid_args, store_of());
    let mut binds = vec![
        (0, eye_buf.as_entire_binding()),
        (8, beams.as_entire_binding()),
        (14, boxes.as_entire_binding()),
        (16, args.as_entire_binding()),
    ];
    if obs.apart {
        binds.push((11, head_buf.as_entire_binding()));
    }
    gpu.dispatch(pass, &binds, [groups[0], groups[1], 1]);
    let got_args = gpu.read(&args);
    let got_boxes = gpu.read(&boxes);

    let mut fail = |what: String| failed.push(format!("{name}: {what}"));
    let cnt = got_args[raster::ARGS_INSTANCES as usize];
    if cnt != want.len() as u32 {
        fail(format!("{cnt} boxes drawn, want {}", want.len()));
    }
    // Each box once, word for word.
    let (mut same, mut differ) = (0u64, 0u64);
    let mut seen = BTreeSet::new();
    for place in 0..(samples as usize).min(cnt as usize) {
        let at = place * raster::BOX_WORDS as usize;
        let b = &got_boxes[at..at + raster::BOX_WORDS as usize];
        let id = b[raster::BOX_ID as usize];
        let Some(w) = want.get(&id).filter(|_| seen.insert(id)) else {
            differ += 1;
            if differ <= 3 {
                eprintln!("DIFFERS {name}: box {place} {b:x?}, no such sample's or twice");
            }
            continue;
        };
        if b != w {
            differ += 1;
            if differ <= 3 {
                eprintln!("DIFFERS {name}: box {place} {b:x?}, want {w:x?}");
            }
        } else {
            same += 1;
        }
    }
    eprintln!(
        "{name}: {same} boxes alike, {differ} differ of {} wanted, {cnt} written",
        want.len()
    );
    if differ != 0 || same != want.len() as u64 {
        fail(format!(
            "{differ} boxes differ, {same} alike of {}",
            want.len()
        ));
    }
    if want.is_empty() {
        fail("no box to compare".to_string());
    }
}

/// Each sample alike the one across before it, where its node's corner lies on the grid a zoom
/// coarser, its node taken a zoom coarser at the same corner, one row in eight: the words a pixel
/// reads alike but the zoom. How many.
fn coarser(obs: &Observed, words: &mut [u32]) -> u64 {
    let read = beam::SAMPLE_FACES as usize;
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
fn hand_eye_boxes_answer_as_raster() {
    use pxl_render::frame::shader::record::SHADER_SAMPLE_BOX;
    use pxl_render::frame::shader::word::SQUARE_SIDE;
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
    let hit = gpu.pipeline(src(SHADER_SAMPLE_HIT), "sample_hit_cs");
    let world = gpu.pipeline(src(SHADER_SAMPLE), "sample_cs");
    let pass = gpu.pipeline(src(SHADER_SAMPLE_BOX), "sample_box_cs");
    let mut kinds = Kinds::new();
    let mut failed = Vec::new();
    // `coarse`: some samples' nodes taken a zoom coarser (`coarser`).
    let mut run = |b: &WorldBufs, c: &EyeCase, coarse: bool, kinds: &mut Kinds| {
        let mut obs = eye_of(c, [0; 3]);
        let table = gpu.buffer(&frame_table(), store_of());
        let lines = eye_beams(&gpu, b, &obs, &table);
        let table = gpu.read(&table);
        let mut words = stopped(&gpu, b, &obs, &lines, None, &table, &hit, &world);
        let Rays::Eye { origin, grid, .. } = &obs.rays else {
            unreachable!()
        };
        let (origin, grid) = (*origin, *grid);
        if coarse {
            kinds.count("alike but the zoom", coarser(&obs, &mut words));
            obs.name += ", a sample's node coarser at its corner";
        }
        let groups = grid.samples.map(|n| n.div_ceil(SQUARE_SIDE));
        boxes_case(
            &gpu,
            &pass,
            &obs,
            &words,
            origin,
            Some(&grid),
            groups,
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
            .filter(|w| w[raster::BOX_ID as usize] & raster::LACK == 0)
            .map(|w| {
                let f = |k: u32| f32::from_bits(w[k as usize]);
                let mid = |a: u32| (f(raster::BOX_LO + a) + f(raster::BOX_HI + a)) / 2.0;
                let top = f(raster::BOX_HI + 2);
                let near = mid(0) * mid(0) + mid(1) * mid(1) + top * top;
                let step = |v: f32| (v * q).round_ties_even() as i32;
                let at = |z: f32, by: i32| {
                    [
                        origin[0].wrapping_add(step(mid(0))),
                        origin[1] + step(mid(1)),
                        origin[2] + step(z) + by,
                    ]
                };
                (
                    near,
                    [at(f(raster::BOX_HI + 2), 2), at(f(raster::BOX_LO + 2), -2)],
                )
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
        "alike the one across",
        "alike the one up",
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

#[test]
fn hand_light_boxes_answer_as_raster() {
    use pxl_render::frame::shader::record::SHADER_LIGHT_SAMPLE_BOX;
    use pxl_render::frame::shader::word::SQUARE_SIDE;
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
        corner: [1300, -900],
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
            corner: [77, 5],
            ..about
        },
    ];
    let hit = gpu.pipeline(src(SHADER_LIGHT_SAMPLE_HIT), "sample_hit_cs");
    let world = gpu.pipeline(src(SHADER_LIGHT_SAMPLE), "sample_cs");
    let pass = gpu.pipeline(src(SHADER_LIGHT_SAMPLE_BOX), "light_sample_box_cs");
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
            stopped(&gpu, b, &o, &lines, None, &t, &hit, &world)
        });
        let words = stopped(
            &gpu,
            b,
            &obs,
            &lines,
            prior.as_deref(),
            &table,
            &hit,
            &world,
        );
        let Rays::Light {
            light: l, starts, ..
        } = &obs.rays
        else {
            unreachable!()
        };
        // Every box from box texel (0, 0)'s start.
        let origin = light::ray(l, starts, [0, 0]).0;
        // Whole, a thread a sample; listed, the rows the list's entries open, as the sample pass's.
        let groups = if c.listed.is_some() {
            obs.groups
        } else {
            obs.lattice.map(|n| n.div_ceil(SQUARE_SIDE))
        };
        kinds.add("listed", c.listed.is_some());
        boxes_case(
            &gpu,
            &pass,
            &obs,
            &words,
            origin,
            None,
            groups,
            kinds,
            &mut failed,
        );
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
    eprintln!("  KINDS light boxes {kinds:?}");
    for what in [
        "boxes",
        "boxes, lacks",
        "boxes, short of its cube",
        "boxes, in another root",
        "alike the one across",
        "alike the one up",
        "listed",
        "a sample before not needed, alike as it stood",
    ] {
        if kinds.get(what) == 0 {
            failed.push(format!("none of {what} to say"));
        }
    }
    if kinds.get("boxes, eyed") != 0 {
        failed.push("a light's box said eyed".to_string());
    }
    if kinds.get("boxes, an inset's product rounded") != 0 {
        failed
            .push("an inset's product rounded, which a fused sum may round otherwise".to_string());
    }
    assert!(failed.is_empty(), "{}", failed.join("\n"));
}

/// `beam_hit_cs` then `beam_cs` over a node cache that holds part of what the beam rays meet: a
/// beam ray the walk over the node cache leaves is walked on from where it was left
/// (resume.wgsl `line_resumed`), and every corner stops where `cast::walk` from the root stops
/// the same ray, at the same node and the same distance handed on (`Grid::hand_dist`). The
/// first frame over an empty cache, the next over that frame's with every other entry finer than
/// `KEPT_ZOOM` dropped. Resumed from a node that does not hold where it was left, the walk climbs
/// to one that does (`line_resumed`); resumed past where it was left, it fails here.
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

/// `beam_hit_cs` then `beam_cs` for the light's map (beam_light.wgsl) over a node cache that
/// holds part of what its beam rays meet: every beam ray stops where `cast::walk` from the root
/// stops the same ray from the origin the pass laid, at the same node, hands on the same
/// distance (`light::margin`, under a rounding a device may take, `margins`), and holds in its
/// line what that walk handed the node and every node over it, or, through a node the node cache
/// keeps at other zooms, what was handed it with the rest marked unknown (walk.wgsl `hand`). The
/// first frame over an empty cache, the next over that frame's with every other entry finer than
/// `KEPT_ZOOM` dropped.
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
            corner: [1300, -900],
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
    // What a line holds past a node the node cache keeps at other zooms (walk.wgsl `hand`).
    let unknown = [
        Shard::UNKNOWN.word(),
        Lookup::NONE.word(),
        NodeSlot::NOTHING.word(),
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
                        let want = h.words();
                        g == want || (g[0] == want[0] && g[1..] == unknown)
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

/// A sample whose walk over the node cache goes blind, into a split the cache holds only at other
/// zooms, then is left there (walk.wgsl): `sample_cs` goes on from where it was left, and every
/// sample stops as `cast::walk` from the root stops it. A split's box holds all its descendants
/// answer at the zooms asked, a lack among them (`world::node`), so the box the walk did not read
/// passes by nothing a ray stops at. The cache the eye's first frame leaves at coarse zooms, the
/// next frame asking finer, every other entry finer than `KEPT_ZOOM` dropped.
#[test]
fn hand_blind_samples_answer_as_cast_walk() {
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
    let hit = gpu.pipeline(src(SHADER_SAMPLE_HIT), "sample_hit_cs");
    let world = gpu.pipeline(src(SHADER_SAMPLE), "sample_cs");
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
        "coarse",
        &mut kinds,
        &mut failed,
        &hit,
        &world,
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
        "fine",
        &mut kinds,
        &mut failed,
        &hit,
        &world,
    );
    assert!(failed.is_empty(), "{}", failed.join("\n"));
}
