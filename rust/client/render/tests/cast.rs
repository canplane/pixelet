//! `face_cs` and `cast_cs` by hand (frame/shader/face.wgsl, cast.wgsl) answer as the Rust over
//! the real world of tests/real_world, off the samples the eye's passes leave there (beam,
//! cell, sample, the record).
//!
//! `face_cs`: each of the record's asks answered with the bits `face::word` gives, every cell beside the
//! hit asked off the line down to it as `support::sub_line` and `support::asset` ask it.
//!
//! `cast_cs`: each pixel it draws holds the words `cast::entry` takes off the visible voxel
//! of the record the test lays at it, or the sky where the four samples about it (`Grid::sample_before`)
//! went on; a cell past the eye's reach is not entered. A pixel it leaves, or marks missed,
//! is the walk's to answer (tests/sample.rs). The device's own tangents, way and reciprocal of
//! each pixel are read off a probe appended to the module (`probe_cs`), which calls what
//! `cast_cs` calls: its tangents and way are held near `px_tan`'s and `cast::through`'s
//! (`RAY_OFF`), its reciprocal near `Ray::inverse` (`INV_ULPS`); and the Rust's entry is taken
//! along that ray and that reciprocal (`cast::entry_with`), as the device divides as it will.
//! A plane's t is held within a few ulps, more along a small component of the way, and a pixel
//! its rounding takes across an edge between the planes of two axes goes either way, one in a
//! thousand of a run's at most.
//!
//! Each kind the test says something of counted, and too few of one fails. Skipped where no
//! adapter answers.

mod gpu;
mod real_world;

use gpu::{
    array_view, at_edge, cross, edges_within, entry_held, entry_node, mix, read_texture, slack,
    texture, ulps, unit, Gpu, Kinds, Tally,
};
use pxl_engine::answer::{self, In, Kind};
use pxl_engine::beam::{self, Grid};
use pxl_engine::cast::{self, Node, Query, Ray};
use pxl_engine::overwrite::{ancestor_at, Cut};
use pxl_engine::support::{self, Ancestors};
use pxl_engine::way::tree;
use pxl_engine::world::{self, Inheritance};
use pxl_engine::{eye, face, node_cache, raster, ray, record, screen, Zoom, LEAF_CELL};
use pxl_render::frame::shader::record::{
    src, SHADER_CAST, SHADER_CAST_TILES, SHADER_CELL, SHADER_FACE, SHADER_SAMPLE_BOX,
};
use pxl_render::frame::shader::word::SQUARE_SIDE;

use real_world::{q8, Built, World, WorldBufs, FINEST};

/// East of X = 0 by the seam, and by X = 0, where X's roots part.
const EYE: [i32; 2] = [(1 << 23) - 300, (40 << 16) + 30];
const EYE_ZERO: [i32; 2] = [150, (40 << 16) + 30];
/// On X's seam, where the world goes round.
const EYE_SEAM: [i32; 2] = [(1 << 23) - 8, (40 << 16) + 30];
const FRAME: u32 = 9;
const WORKGROUP: u32 = beam::SAMPLE_SIDE;
/// The ulps a device's division may put a plane's t off the Rust's, past what its way's own
/// rounding takes (`slack`).
const T_ULPS: u32 = 4;
/// The ulps the device's reciprocal of a pixel's way may lie off the Rust's correctly rounded
/// one: WGSL's bound on a division, 2.5 ulps, taken up to a whole ulp. Metal measured 1 here.
const INV_ULPS: u32 = 3;
/// How far the device's tangents and way of a pixel may lie off `px_tan`'s and
/// `cast::through`'s, on a component of at most 1: the division of a tangent, and the square
/// root, the sum and the division of the way, each within WGSL's 2.5 ulps of 1, summed and
/// taken up. Metal measured 1.5.
const RAY_OFF: f32 = 8.0 * f32::EPSILON;
/// The words the probe leaves a pixel: its tangents, its ray's way and inverse.
const PROBE_WORDS: u32 = 8;

/// The pixel's ray as the module works it out, a thread a pixel, by the functions `cast_cs`
/// calls: the Rust is asked along the ray the device took.
const PROBE: &str = r"
@group(0) @binding(40) var<storage, read_write> probe: array<u32>;

@compute @workgroup_size(8, 8, 1)
fn probe_cs(@builtin(global_invocation_id) id: vec3<u32>) {
  let size = textureDimensions(rays);
  if (id.x >= size.x || id.y >= size.y) {
    return;
  }
  let px = px_tan(id.xy, size.xy);
  let dir = ray_dir(eye_aim(px).dir);
  let inv = ray_inv(dir);
  let k = (id.y * size.x + id.x) * 8u;
  probe[k] = bitcast<u32>(px.x);
  probe[k + 1u] = bitcast<u32>(px.y);
  for (var a = 0u; a < 3u; a += 1u) {
    probe[k + 2u + a] = bitcast<u32>(dir[a]);
    probe[k + 5u + a] = bitcast<u32>(inv[a]);
  }
}
";

/// An eye over the built world: where it stands and looks, and the zooms it asks.
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

/// What the eye's passes leave: its words and grid, the beam rays' words with the samples'
/// boxes' asks answered, the node cache, and the boxes' count.
struct Cast {
    eye: Vec<u32>,
    grid: Grid,
    origin: [i32; 3],
    axes: [[f32; 3]; 3],
    far: f32,
    cuts: [Cut; 2],
    /// The beam rays' words as the faces' pass takes them, and after it.
    asked: Vec<u32>,
    beams: Vec<u32>,
    table: Vec<u32>,
    /// The faces' asks laid, and the drawable entries the samples' stops set and the faces' pass
    /// left its faces' words in (pxl-engine `record`).
    asks: u32,
}

fn device() -> Option<Gpu> {
    // A hand shader's loop may hang the device's wait: the run gives up.
    std::thread::spawn(|| {
        std::thread::sleep(std::time::Duration::from_secs(600));
        eprintln!("gave up after 10 minutes");
        std::process::exit(124);
    });
    let gpu = Gpu::new();
    if gpu.is_none() {
        eprintln!("no adapter: skipped");
    }
    gpu
}

fn store() -> wgpu::BufferUsages {
    wgpu::BufferUsages::STORAGE
}

fn world_bufs(gpu: &Gpu, built: &Built) -> WorldBufs {
    let flat_u = |rows: &[[u32; 4]]| rows.iter().flatten().copied().collect::<Vec<u32>>();
    let clip: Vec<u32> = built.clipmap.iter().flatten().map(|&v| v as u32).collect();
    WorldBufs {
        clip: gpu.buffer(&clip, store()),
        ground: gpu.buffer(&built.ground, store()),
        palette: gpu.buffer(&built.palette, store()),
        shards: gpu.buffer(&flat_u(&built.shards), store()),
        placed: gpu.buffer(&built.overwrites, store()),
        pool: gpu.buffer(&built.pool, store()),
    }
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

/// The eye's words at `c`, as eye.rs `Eye` gives them, and its axes.
fn eye_of(c: &EyeCase) -> (Vec<u32>, Grid, [[f32; 3]; 3]) {
    let forward = unit(c.look);
    let right = unit(cross(forward, [0.0, 0.0, 1.0]));
    let up = cross(right, forward);
    let grid = Grid::of(c.tan, c.lattice);
    let mut words = vec![0u32; eye::WORDS];
    for k in 0..3 {
        words[eye::ORIGIN + k] = c.origin[k] as u32;
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
    (words, grid, [forward, right, up])
}

/// The eye's passes before the pixels, as a frame that casts runs them: the beam rays, the
/// cells' starts, the samples, their record and their asks, then the record's faces asked by
/// `face_cs`.
fn cast_of(gpu: &Gpu, b: &WorldBufs, c: &EyeCase, face_p: &wgpu::ComputePipeline) -> Cast {
    let (eye, grid, axes) = eye_of(c);
    let eye_buf = gpu.buffer(&eye, wgpu::BufferUsages::UNIFORM);
    let mut table0 = vec![0u32; node_cache::WORDS as usize];
    table0[node_cache::FRAME as usize] = FRAME;
    let table = gpu.buffer(&table0, store());
    let beams = gpu.buffer(&vec![0u32; grid.size() as usize], store());
    gpu.corners(false, &world_binds(b, &eye_buf, &beams, &table), &table);
    gpu.dispatch(
        &gpu.pipeline(src(SHADER_CELL), "cell_cs"),
        &[
            (0, eye_buf.as_entire_binding()),
            (8, beams.as_entire_binding()),
        ],
        [
            grid.span[0].div_ceil(WORKGROUP),
            grid.span[1].div_ceil(WORKGROUP),
            1,
        ],
    );
    gpu.samples(false, &world_binds(b, &eye_buf, &beams, &table), &table);
    let samples = grid.samples[0] * grid.samples[1];
    // The draw's args, then the faces' asks' (`raster::ASKED_AT`).
    let asked_at = raster::ASKED_AT as usize;
    let mut laid = vec![0u32; asked_at + raster::ARGS_WORDS as usize];
    laid[raster::ARGS_VERTICES as usize] = raster::VERTICES;
    laid[asked_at + raster::ARGS_FACES as usize + 1] = 1;
    laid[asked_at + raster::ARGS_FACES as usize + 2] = 1;
    laid[asked_at + raster::ARGS_ASKS as usize] = grid.asks_at;
    let box_args = gpu.buffer(&laid, store());
    // The samples' stops into the entries their walks kept.
    gpu.dispatch(
        &gpu.pipeline(src(SHADER_SAMPLE_BOX), "sample_box_cs"),
        &[
            (0, eye_buf.as_entire_binding()),
            (8, beams.as_entire_binding()),
            (9, table.as_entire_binding()),
            (16, box_args.as_entire_binding()),
        ],
        [
            grid.samples[0].div_ceil(SQUARE_SIDE),
            grid.samples[1].div_ceil(SQUARE_SIDE),
            1,
        ],
    );
    let asked = gpu.read(&beams);
    let laid = gpu.read(&box_args);
    let (mut stopped, mut lacks) = (0, 0);
    for j in 0..grid.samples[1] {
        for i in 0..grid.samples[0] {
            let k = grid.sample_at([i, j]) as usize;
            stopped += u32::from(asked[k + beam::NODE_ZOOM as usize] != beam::SAMPLE_NONE);
            lacks += u32::from(asked[k + beam::SAMPLE_LACKS as usize] != 0);
        }
    }
    eprintln!(
        "{}: {} samples, {stopped} stopped, {lacks} lack, {} asks",
        c.name,
        samples,
        laid[asked_at + raster::ARGS_INSTANCES as usize],
    );

    let mut binds = face_binds(b, &eye_buf, &beams, &box_args, raster::ASKED_AT);
    binds.push((9, table.as_entire_binding()));
    gpu.dispatch(
        face_p,
        &binds,
        [laid[asked_at + raster::ARGS_FACES as usize], 1, 1],
    );
    Cast {
        eye,
        grid,
        origin: c.origin,
        axes,
        far: c.far,
        cuts: c.cuts,
        asked,
        beams: gpu.read(&beams),
        table: gpu.read(&table),
        asks: laid[asked_at + raster::ARGS_INSTANCES as usize],
    }
}

fn face_binds<'a>(
    b: &'a WorldBufs,
    eye: &'a wgpu::Buffer,
    beams: &'a wgpu::Buffer,
    args: &'a wgpu::Buffer,
    at: u32,
) -> Vec<(u32, wgpu::BindingResource<'a>)> {
    vec![
        (0, eye.as_entire_binding()),
        (1, b.clip.as_entire_binding()),
        (2, b.ground.as_entire_binding()),
        (5, b.shards.as_entire_binding()),
        (6, b.placed.as_entire_binding()),
        (7, b.pool.as_entire_binding()),
        (8, beams.as_entire_binding()),
        (
            16,
            wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                buffer: args,
                offset: u64::from(at) * 4,
                size: std::num::NonZeroU64::new(u64::from(raster::ARGS_WORDS) * 4),
            }),
        ),
    ]
}

/// The world's one query with nothing kept: what `face_cs` asks the world.
struct Asking<'a> {
    w: World<'a>,
    cuts: [Cut; 2],
}

impl Query for Asking<'_> {
    type At = Inheritance;

    fn root(&mut self, node: [i32; 3]) -> Inheritance {
        Inheritance::of(&self.w, (0, node))
    }

    fn child(&mut self, at: &Inheritance, zoom: Zoom, node: [i32; 3]) -> Inheritance {
        at.child(&self.w, &tree::Read(self.w.pool), (zoom, node))
    }

    fn node(
        &mut self,
        n: &mut Inheritance,
        zoom: Zoom,
        c: [i32; 3],
        zooms: [Zoom; 2],
        _: f32,
    ) -> In<[u32; 2]> {
        let pool = tree::Read(self.w.pool);
        let mut held = self.w;
        world::node(
            &mut held,
            &self.w,
            &self.w,
            &pool,
            n,
            (zoom, c),
            zooms,
            self.cuts,
        )
    }
}

/// Each drawable entry's slot of the node cache's words `table` (pxl-engine `record`), by its
/// node's zoom and index, the first of its bucket where two hold one.
fn drawable_at(table: &[u32]) -> std::collections::BTreeMap<(u32, [i32; 3]), u32> {
    let mut at = std::collections::BTreeMap::new();
    for slot in (0..node_cache::ENTRIES).rev() {
        let e = node_cache::entry_at(slot) as usize;
        let kind = table[e + node_cache::W_KIND as usize] & node_cache::KIND_MASK;
        let state = table[e + node_cache::W_STATE as usize];
        if entry_held(table, e)
            && kind != Kind::Split as u32
            && state & (record::FACES | record::ASKED) != 0
        {
            let w = e + node_cache::W_NODE as usize;
            at.insert((table[w], [1, 2, 3].map(|a| table[w + a] as i32)), slot);
        }
    }
    at
}

/// The node at word `k` of the beam rays' words.
fn node_at(beams: &[u32], k: u32) -> Node {
    let k = k as usize;
    let w = |a: u32| beams[k + a as usize];
    Node {
        corner: [0, 1, 2].map(|a| w(beam::NODE_CORNER + a)),
        zoom: w(beam::NODE_ZOOM),
        roots: [0, 1, 2].map(|a| w(beam::NODE_ROOTS + a) as i32),
    }
}

/// A beam ray's line as a reader of the assets about a hit asks off it: its node, and what
/// was handed each node over it.
struct BeamLine<'a> {
    beams: &'a [u32],
    line: u32,
    at: [i32; 3],
    zoom: Zoom,
}

impl<'a> BeamLine<'a> {
    fn of(beams: &'a [u32], line: u32, origin: [i32; 3]) -> BeamLine<'a> {
        let n = node_at(beams, line + beam::LINE_NODE);
        BeamLine {
            beams,
            line,
            at: n.idx(origin),
            zoom: n.zoom,
        }
    }
}

impl Ancestors<Inheritance> for BeamLine<'_> {
    fn at(&self) -> [i32; 3] {
        self.at
    }

    fn zoom(&self) -> Zoom {
        self.zoom
    }

    fn ancestor(&self, z: Zoom) -> Inheritance {
        let k = (self.line + beam::LINE_INHERITANCE + beam::LINE_STEP * z) as usize;
        Inheritance::of_words([
            self.beams[k],
            self.beams[k + 1],
            self.beams[k + 2],
            self.beams[k + 3],
        ])
    }
}

/// The ask at word `a` as the Rust asks it (`Want`), its node the drawable entry's in the
/// node cache's words `table` where it is an entry's.
fn face_want(q: &mut Asking, beams: &[u32], table: &[u32], origin: [i32; 3], a: u32) -> Want {
    let w = |k: u32| beams[(a + k) as usize];
    // A drawable entry's ask names its entry (`record::TO_RECORD`), a hit's its node.
    let hit = if w(beam::ASK_TO) & record::TO_RECORD != 0 {
        entry_node(table, w(beam::ASK_NODE) as usize, origin)
    } else {
        node_at(beams, w(beam::ASK_NODE))
    };
    let (zoom, c) = (hit.zoom, hit.idx(origin));
    let face_of = w(beam::ASK_FACE);
    let base = BeamLine::of(beams, w(beam::ASK_LINE), origin);
    let below = support::sub_line(q, &base, zoom, c);
    let line = below.joined(&base);
    let bits = face::word(face_of, |d| {
        answer::whole(support::asset(q, &line, zoom, face::beside(zoom, c, d)))
    });
    // The same asked with the carve and the hide each other's.
    let cuts = q.cuts;
    q.cuts = [cuts[1], cuts[0]];
    let swapped = face::word(face_of, |d| {
        answer::whole(support::asset(q, &line, zoom, face::beside(zoom, c, d)))
    });
    q.cuts = cuts;
    Want {
        bits,
        cut: swapped != bits,
    }
}

/// An ask's bits as the Rust asks them, and whether the carve and the hide swapped change
/// them.
struct Want {
    bits: u32,
    cut: bool,
}

/// The eyes both tests stand on: over the ground, a lattice of two over the seam, cut and
/// hid on what is placed, by a way, out to the horizon, and down into a hole carved.
fn eyes(built: &Built) -> Vec<EyeCase> {
    let none = [Cut::NONE; 2];
    let over = |g: [i32; 2], up: i32| q8([g[0], g[1], built.top_at(g) + up]);
    let placed = built
        .placed
        .iter()
        .copied()
        .find(|p| p.0 == 25)
        .expect("something placed finer than a cell");
    let (leaf_zoom, leaf_c) = placed;
    let bridge = built
        .way_points
        .iter()
        .find(|p| p.0 == LEAF_CELL && [0, 3].contains(&p.1) && p.2[0] > 0)
        .expect("a way")
        .2;
    // A cube of the ground carved under an eye looking down into it, the leaf beside hid.
    let hole = [(leaf_c[0] >> 1) + 6, (leaf_c[1] >> 1) + 6];
    let carved = ancestor_at(LEAF_CELL, [hole[0], hole[1], built.top_at(hole) - 1], 3);
    vec![
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
            origin: over([leaf_c[0] >> 1, leaf_c[1] >> 1], 20),
            look: [0.02, 0.03, -1.0],
            tan: [0.3, 0.4],
            lattice: 1,
            far: 400.0,
            range: [LEAF_CELL, FINEST],
            cuts: [
                Cut {
                    zoom: 22,
                    at: [leaf_c[0] >> 3, leaf_c[1] >> 3, leaf_c[2] >> 3],
                },
                Cut {
                    zoom: leaf_zoom,
                    at: leaf_c,
                },
            ],
        },
        EyeCase {
            name: "the eye, by a way",
            origin: q8([
                bridge[0],
                bridge[1],
                built.top_at([bridge[0], bridge[1]]) + 12,
            ]),
            look: [0.5, -0.3, -0.8],
            tan: [0.3, 0.3],
            lattice: 1,
            far: 600.0,
            range: [LEAF_CELL, FINEST],
            cuts: none,
        },
        EyeCase {
            name: "the eye, out to the horizon",
            origin: over([EYE[0] + 40, EYE[1] - 60], 30),
            look: [0.7, 0.6, -0.08],
            tan: [0.5, 0.3],
            lattice: 1,
            far: 500.0,
            range: [8, FINEST],
            cuts: none,
        },
        EyeCase {
            name: "the eye, down into a hole carved, a leaf hid",
            origin: over(hole, 30),
            look: [0.05, 0.03, -1.0],
            tan: [0.4, 0.4],
            lattice: 1,
            far: 400.0,
            range: [8, FINEST],
            cuts: [
                Cut {
                    zoom: LEAF_CELL - 3,
                    at: carved,
                },
                Cut {
                    zoom: leaf_zoom,
                    at: leaf_c,
                },
            ],
        },
    ]
}

/// The worlds the eyes stand in, and the eyes: by the seam, one high over X = 0 and the
/// mountain west of it, its rays ending in either root, and one down on X's seam.
fn worlds() -> Vec<(Built, Vec<EyeCase>)> {
    let built = real_world::build(EYE);
    let cases = eyes(&built);
    let zero = real_world::build(EYE_ZERO);
    let c = EyeCase {
        name: "the eye, looking over X = 0",
        origin: q8([150, EYE_ZERO[1] + 90, 2300]),
        look: [-0.3, 0.1, -1.0],
        tan: [0.4, 0.3],
        lattice: 1,
        far: 4000.0,
        range: [8, FINEST],
        cuts: [Cut::NONE; 2],
    };
    let seam = real_world::build(EYE_SEAM);
    let d = EyeCase {
        name: "the eye, down on X's seam",
        origin: q8([
            EYE_SEAM[0] + 4,
            EYE_SEAM[1] + 20,
            seam.top_at([EYE_SEAM[0] + 4, EYE_SEAM[1] + 20]) + 25,
        ]),
        look: [0.1, 0.05, -1.0],
        tan: [0.4, 0.4],
        lattice: 1,
        far: 400.0,
        range: [8, FINEST],
        cuts: [Cut::NONE; 2],
    };
    vec![(built, cases), (zero, vec![c]), (seam, vec![d])]
}

#[test]
fn hand_faces_answer_as_face_word() {
    let Some(gpu) = device() else { return };
    let face_p = gpu.pipeline(src(SHADER_FACE), "face_cs");
    let mut tally = Tally::default();
    let mut kinds = Kinds::new();
    let worlds = worlds();
    for (built, c) in worlds
        .iter()
        .flat_map(|(w, cs)| cs.iter().map(move |c| (w, c)))
    {
        let b = world_bufs(&gpu, built);
        let cast = cast_of(&gpu, &b, c, &face_p);
        let mut q = Asking {
            w: built.world(),
            cuts: cast.cuts,
        };
        let g = &cast.grid;

        // Each box's ask answered where it says, as the Rust answers it.
        let (mut want, mut have) = (Vec::new(), Vec::new());
        for i in 0..cast.asks {
            let a = g.ask_at(i);
            let to = cast.asked[(a + beam::ASK_TO) as usize];
            if to == beam::ASK_NONE {
                continue;
            }
            let Want { bits, cut } = face_want(&mut q, &cast.asked, &cast.table, cast.origin, a);
            kinds.add("asked, the cuts swapped answer otherwise", cut);
            want.push(bits);
            have.push(if to & record::TO_RECORD != 0 {
                cast.table[(to & !record::TO_RECORD) as usize]
            } else {
                cast.beams[to as usize]
            });
            kinds.add("asked", true);
            kinds.add("asked, a rise", bits & 0xf != 0);
            kinds.add("asked, a line", bits >> face::LINE_SHIFT & 0xf != 0);
            kinds.add("asked, a corner", bits >> face::CORNER_SHIFT != 0);
            kinds.add("asked, none set", bits == 0);
        }
        tally.compare(
            &format!("{}, the record's asks", c.name),
            &want,
            &have,
            |k| format!("ask {k}"),
        );
    }
    eprintln!("  KINDS faces {kinds:?}");
    let short = kinds.short(&[
        ("asked", 1000),
        ("asked, a rise", 1),
        ("asked, a line", 1),
        ("asked, a corner", 1),
        ("asked, none set", 1),
        ("asked, the cuts swapped answer otherwise", 1),
    ]);
    tally.end("faces");
    assert!(short.is_empty(), "{}", short.join("\n"));
}

/// A screen the pixels are cast on: its pixels, the eye's jitter, the share of the samples'
/// reach the eye now has, and one tile in so many not claimed.
struct Screen {
    size: [u32; 2],
    jitter: [f32; 2],
    reach: f32,
    unclaimed_in: u32,
}

/// The tangents of pixel `p`'s ray, as `cast_cs` takes them (grid.wgsl `px_tan`).
fn px_tan(s: &Screen, tan: [f32; 2], p: [u32; 2]) -> [f32; 2] {
    let ndc_x = (p[0] as f32 + 0.5 + s.jitter[0]) / s.size[0] as f32 * 2.0 - 1.0;
    let ndc_y = 1.0 - (p[1] as f32 + 0.5 + s.jitter[1]) / s.size[1] as f32 * 2.0;
    [ndc_x * tan[0], ndc_y * tan[1]]
}

/// What the probe left of each pixel: its tangents and its ray.
struct Probed {
    px: [f32; 2],
    dir: [f32; 3],
    inv: [f32; 3],
}

fn probe(
    gpu: &Gpu,
    p: &wgpu::ComputePipeline,
    eye: &wgpu::Buffer,
    rays: &wgpu::TextureView,
    size: [u32; 2],
) -> Vec<Probed> {
    let out = gpu.buffer(
        &vec![0u32; (size[0] * size[1] * PROBE_WORDS) as usize],
        store(),
    );
    gpu.dispatch(
        p,
        &[
            (0, eye.as_entire_binding()),
            (3, wgpu::BindingResource::TextureView(rays)),
            (40, out.as_entire_binding()),
        ],
        [size[0].div_ceil(8), size[1].div_ceil(8), 1],
    );
    gpu.read(&out)
        .chunks(PROBE_WORDS as usize)
        .map(|w| Probed {
            px: [f32::from_bits(w[0]), f32::from_bits(w[1])],
            dir: [2, 3, 4].map(|k| f32::from_bits(w[k])),
            inv: [5, 6, 7].map(|k| f32::from_bits(w[k])),
        })
        .collect()
}

#[test]
fn hand_pixels_answer_as_cast_entry() {
    let Some(gpu) = device() else { return };
    let face_p = gpu.pipeline(src(SHADER_FACE), "face_cs");
    let cast_p = gpu.pipeline(src(SHADER_CAST), "cast_cs");
    let cast_tiles_p = gpu.pipeline(src(SHADER_CAST_TILES), "cast_tiles_cs");
    let probe_p = gpu.pipeline(
        &format!("{}\n{PROBE}", gpu::source(src(SHADER_CAST), "cast_cs")),
        "probe_cs",
    );
    let screens = [
        Screen {
            size: [256, 160],
            jitter: [0.0, 0.0],
            reach: 1.0,
            unclaimed_in: 0,
        },
        Screen {
            size: [301, 173],
            jitter: [0.25, -0.375],
            reach: 1.0,
            unclaimed_in: 3,
        },
        // The eye's reach cut since its samples were cast: a cell past it is not entered.
        Screen {
            size: [199, 121],
            jitter: [-0.5, 0.125],
            reach: 0.4,
            unclaimed_in: 5,
        },
    ];
    let mut tally = Tally::default();
    let mut kinds = Kinds::new();
    let (mut dir_off, mut px_off, mut inv_off) = (0f32, 0f32, 0u32);
    let worlds = worlds();
    let mut runs: Vec<(&Built, &EyeCase, &Screen)> = Vec::new();
    for (built, cases) in &worlds {
        for (k, c) in cases.iter().enumerate() {
            runs.push((built, c, &screens[(k + 1) % screens.len()]));
        }
    }
    for (built, c, s) in runs {
        let b = world_bufs(&gpu, built);
        let cast = &cast_of(&gpu, &b, c, &face_p);
        let g = &cast.grid;
        let name = format!("{}, {}x{}", c.name, s.size[0], s.size[1]);
        let [wd, ht] = s.size;
        let texels = (wd * ht) as usize;
        let mut eye = cast.eye.clone();
        eye[eye::JITTER] = s.jitter[0].to_bits();
        eye[eye::JITTER + 1] = s.jitter[1].to_bits();
        let far = cast.far * s.reach;
        eye[eye::FAR] = far.to_bits();
        let eye_buf = gpu.buffer(&eye, wgpu::BufferUsages::UNIFORM);
        let ray_words: Vec<u32> = (0..ray::WORDS as usize * texels)
            .map(|k| (k as u32).wrapping_mul(0x2c1b_3c6d) >> 2)
            .collect();
        let rays = texture(
            &gpu.device,
            &gpu.queue,
            s.size,
            ray::WORDS,
            wgpu::TextureUsages::STORAGE_BINDING,
            &ray_words,
        );
        let rays_view = array_view(&rays);
        let probed = probe(&gpu, &probe_p, &eye_buf, &rays_view, s.size);

        // The device's ray against the Rust's for the same pixel.
        let before: Vec<[u32; 2]> = probed.iter().map(|at| g.sample_before(at.px)).collect();
        for (p, at) in probed.iter().enumerate() {
            let px = [p as u32 % wd, p as u32 / wd];
            let t = px_tan(s, g.tan, px);
            px_off = px_off.max((t[0] - at.px[0]).abs().max((t[1] - at.px[1]).abs()));
            let d = cast::through(cast.axes[0], cast.axes[1], cast.axes[2], t[0], t[1]);
            dir_off = (0..3).fold(dir_off, |m, a| m.max((d[a] - at.dir[a]).abs()));
            let inv = Ray::new(at.dir, far).inverse();
            let off = (0..3).map(|a| ulps(at.inv[a], inv[a])).max().unwrap_or(0);
            inv_off = inv_off.max(off);
        }

        // The visible voxel the raster would leave: mostly that of one of the four samples
        // about the pixel, some none, some a lack, some a sample's far off.
        let n = g.samples[0];
        // Each drawable entry's slot by its node.
        let voxel_at = drawable_at(&cast.table);
        let voxel_of = |i: u32| -> Option<u32> {
            let k = g.sample_at([i % n, i / n]);
            let node = node_at(&cast.beams, k + beam::SAMPLE_NODE);
            voxel_at.get(&(node.zoom, node.idx(cast.origin))).copied()
        };
        let stopped: Vec<u32> = (0..n * g.samples[1])
            .filter(|&i| {
                let k = g.sample_at([i % n, i / n]);
                cast.beams[(k + beam::SAMPLE_NODE + beam::NODE_ZOOM) as usize] != beam::SAMPLE_NONE
            })
            .collect();
        let visible: Vec<u32> = (0..wd * ht)
            .map(|p| {
                let h = mix(p, 0xca57);
                let s0 = before[p as usize];
                let at = [s0[0] + (h >> 8) % 2, s0[1] + (h >> 9) % 2];
                let i = match h % 16 {
                    0 => return 0,
                    1 if !stopped.is_empty() => stopped[(h as usize >> 12) % stopped.len()],
                    _ => at[1] * n + at[0],
                };
                let k = g.sample_at([i % n, i / n]);
                let w = |a: u32| cast.beams[(k + a) as usize];
                if w(beam::SAMPLE_NODE + beam::NODE_ZOOM) == beam::SAMPLE_NONE {
                    return 0;
                }
                let Some(v) = voxel_of(i) else {
                    return 0;
                };
                raster::id_of(v, w(beam::SAMPLE_LACKS) != 0 || h % 16 == 2)
            })
            .collect();
        let vis = texture(
            &gpu.device,
            &gpu.queue,
            s.size,
            1,
            wgpu::TextureUsages::TEXTURE_BINDING,
            &visible,
        );
        let vis_view = array_view(&vis);

        // The tiles: one in so many not claimed this frame.
        let now = 77u32;
        let tiles = [wd.div_ceil(screen::TILE), ht.div_ceil(screen::TILE)];
        let changed: Vec<u32> = (0..tiles[0] * tiles[1])
            .map(|t| {
                if s.unclaimed_in != 0 && mix(t, 5).is_multiple_of(s.unclaimed_in) {
                    now - screen::CLAIMED_FOR
                } else {
                    now - mix(t, 6) % screen::CLAIMED_FOR
                }
            })
            .collect();
        let mut uniform = [0u32; screen::TILES_WORDS];
        uniform[..4].copy_from_slice(&[tiles[0], tiles[1], screen::CLAIMED_FOR, now]);
        let changed_buf = gpu.buffer(&changed, store());
        let tiles_buf = gpu.buffer(&uniform, wgpu::BufferUsages::UNIFORM);
        let mut list_words = vec![0u32; 4 + (tiles[0] * tiles[1]) as usize];
        let per_tile = screen::TILE / ray::CAST_SIDE;
        list_words[1..3].copy_from_slice(&[per_tile, per_tile]);
        let list = gpu.buffer(&list_words, store());
        gpu.dispatch(
            &cast_tiles_p,
            &[
                (0, list.as_entire_binding()),
                (30, changed_buf.as_entire_binding()),
                (31, tiles_buf.as_entire_binding()),
            ],
            [
                (tiles[0] * tiles[1]).div_ceil(screen::CAST_TILES_LANES),
                1,
                1,
            ],
        );
        let listed = gpu.read(&list)[0];

        let table_buf = gpu.buffer(&cast.table, store());
        let marks = gpu.buffer(&ray::MISS_ARGS_EMPTY, store());
        let beams = gpu.buffer(&cast.beams, store());
        gpu.dispatch(
            &cast_p,
            &[
                (0, eye_buf.as_entire_binding()),
                (3, wgpu::BindingResource::TextureView(&rays_view)),
                (8, beams.as_entire_binding()),
                (9, table_buf.as_entire_binding()),
                (13, wgpu::BindingResource::TextureView(&vis_view)),
                (14, marks.as_entire_binding()),
                (30, list.as_entire_binding()),
            ],
            [listed, per_tile, per_tile],
        );

        // The Rust, along the device's ray, at each pixel the device drew: a pixel the visible
        // voxel answers, the sky, or a hole the walk is to answer, which holds what the record
        // drew there until it does, the visible voxel at its box's entry or none.
        let got = read_texture(&gpu.device, &gpu.queue, &rays);
        let (mut want, mut have) = (Vec::new(), Vec::new());
        let mut drawn_cnt = 0u32;
        let mut edges = 0u64;
        for (p, at) in probed.iter().enumerate() {
            let words = |w: &[u32]| [0, 1, 2, 3].map(|l| w[l * texels + p]);
            let drawn = words(&got);
            let drew = drawn != words(&ray_words);
            let v = visible[p];
            let r = Ray::new(at.dir, far);
            let mut said = [0; 4];
            if v != 0 {
                let e = node_cache::entry_at(raster::voxel_of(v)) as usize;
                let w = |a: u32| cast.table[e + a as usize];
                let node = entry_node(&cast.table, e, cast.origin);
                let asset = [w(node_cache::W_ASSET), w(node_cache::W_ASSET + 1)];
                let faces = record::asked_faces(w(node_cache::W_STATE));
                // The entry along the device's reciprocal, its plane's t a few ulps off as the
                // device divides; a pixel at an edge as the device rounds it.
                let e = cast::entry_with(cast.origin, &r, at.inv, &node, asset[0]);
                let axis_was = e.axis;
                let lacks = v & raster::LACK != 0;
                let mut takes =
                    !lacks && e.enters && raster::accepts(faces, ray::face(at.dir, e.axis));
                kinds.add("enters its sample", takes);
                kinds.add(
                    "enters its sample past the eye's reach",
                    !takes && {
                        let r = Ray::new(at.dir, f32::INFINITY);
                        let e = cast::entry_with(cast.origin, &r, at.inv, &node, asset[0]);
                        e.enters && raster::accepts(faces, ray::face(at.dir, e.axis))
                    },
                );
                if !takes && !lacks && drew && at_edge(&e, at.dir) {
                    takes = true;
                    edges += 1;
                }
                // A hole holds the voxel the record drew at its box's entry, as one it takes.
                {
                    let face = ray::face(at.dir, axis_was);
                    let t = e.near[axis_was as usize].max(0.0);
                    let had = f32::from_bits(drawn[1]);
                    let t = if ulps(t, had)
                        <= T_ULPS.saturating_add(slack(at.dir[axis_was as usize]))
                    {
                        had
                    } else {
                        t
                    };
                    said = [
                        ray::first_word(asset[0], face, node.zoom),
                        t.to_bits(),
                        asset[1],
                        w(node_cache::W_FACES + face),
                    ];
                    kinds.add(
                        "enters its sample, short of its cube",
                        takes && answer::fill_eighths(asset[0]) != answer::EIGHTHS,
                    );
                    kinds.add("a hole the record drew", !takes);
                }
            } else if v == 0 {
                let s0 = before[p];
                let went_on = |d: [u32; 2]| {
                    let k = g.sample_at([s0[0] + d[0], s0[1] + d[1]]);
                    cast.beams[(k + beam::SAMPLE_NODE + beam::NODE_ZOOM) as usize]
                        == beam::SAMPLE_NONE
                };
                if [[0, 0], [1, 0], [0, 1], [1, 1]].into_iter().all(went_on) {
                    said = [0; 4];
                    kinds.add("sky", true);
                }
            }
            if drew {
                drawn_cnt += 1;
                want.extend(said);
                have.extend(drawn);
            }
        }
        tally.compare(&format!("{name}: pixels drawn"), &want, &have, |k| {
            format!("drawn pixel {} word {}", k / 4, k % 4)
        });
        eprintln!("{name}: {drawn_cnt} drawn, {edges} decided at an edge");
        if !edges_within(edges, u64::from(drawn_cnt)) {
            tally.differ.push((format!("{name}: edges"), edges));
        }
    }
    eprintln!(
        "  KINDS pixels {kinds:?}; the ray {dir_off:e} off the Rust's, its tangents {px_off:e}, \
         its reciprocal {inv_off} ulps"
    );
    let short = kinds.short(&[
        ("enters its sample", 1000),
        ("enters its sample, short of its cube", 1),
        ("sky", 100),
        ("enters its sample past the eye's reach", 10),
    ]);
    tally.end("pixels");
    assert!(short.is_empty(), "{}", short.join("\n"));
    assert!(
        dir_off <= RAY_OFF && px_off <= RAY_OFF,
        "the device's ray {dir_off:e} off the Rust's, its tangents {px_off:e}"
    );
    assert!(
        inv_off <= INV_ULPS,
        "the device's reciprocal {inv_off} ulps off the Rust's"
    );
}

/// A pixel whose visible voxel is stale from the eye that now casts (`record::fresh`) is a
/// hole: `cast_cs` lists it for the holes' queue, holding what the record drew there, and does
/// not take the stale voxel as the pixel's answer. The record is the one an eye's samples
/// wrote; the eye that casts asks every node finer than any it holds, so every entry is stale
/// from it, as a coarse voxel recorded from far is once the eye comes near.
#[test]
fn hand_stale_voxel_is_a_hole() {
    let Some(gpu) = device() else { return };
    let face_p = gpu.pipeline(src(SHADER_FACE), "face_cs");
    let cast_p = gpu.pipeline(src(SHADER_CAST), "cast_cs");
    let cast_tiles_p = gpu.pipeline(src(SHADER_CAST_TILES), "cast_tiles_cs");
    let size = [256u32, 160u32];
    let [wd, ht] = size;
    let texels = (wd * ht) as usize;
    let asked = FINEST + 1;
    let (mut stale_entered, mut stale_taken) = (0u32, 0u32);
    let worlds = worlds();
    for (built, cases) in &worlds {
        for c in cases {
            let b = world_bufs(&gpu, built);
            let cast = cast_of(&gpu, &b, c, &face_p);
            let g = &cast.grid;
            let mut eye = cast.eye.clone();
            eye[eye::RANGE] = asked;
            eye[eye::RANGE + 1] = asked;
            let eye_buf = gpu.buffer(&eye, wgpu::BufferUsages::UNIFORM);
            let rays = texture(
                &gpu.device,
                &gpu.queue,
                size,
                ray::WORDS,
                wgpu::TextureUsages::STORAGE_BINDING,
                &vec![0u32; ray::WORDS as usize * texels],
            );
            let rays_view = array_view(&rays);

            // Each pixel's visible voxel: the drawable entry of the sample at or before it.
            let voxel_at = drawable_at(&cast.table);
            let n = g.samples[0];
            let mut rays_of = Vec::with_capacity(texels);
            let visible: Vec<u32> = (0..wd * ht)
                .map(|p| {
                    let px = [p % wd, p / wd];
                    let t = px_tan(
                        &Screen {
                            size,
                            jitter: [0.0; 2],
                            reach: 1.0,
                            unclaimed_in: 0,
                        },
                        g.tan,
                        px,
                    );
                    let d = cast::through(cast.axes[0], cast.axes[1], cast.axes[2], t[0], t[1]);
                    rays_of.push(Ray::new(d, cast.far));
                    let s = g.sample_before(t);
                    let k = g.sample_at(s);
                    let w = |a: u32| cast.beams[(k + a) as usize];
                    if s[0] >= n || w(beam::SAMPLE_NODE + beam::NODE_ZOOM) == beam::SAMPLE_NONE {
                        return 0;
                    }
                    let node = node_at(&cast.beams, k + beam::SAMPLE_NODE);
                    match voxel_at.get(&(node.zoom, node.idx(cast.origin))) {
                        Some(&v) if w(beam::SAMPLE_LACKS) == 0 => raster::id_of(v, false),
                        _ => 0,
                    }
                })
                .collect();
            let vis = texture(
                &gpu.device,
                &gpu.queue,
                size,
                1,
                wgpu::TextureUsages::TEXTURE_BINDING,
                &visible,
            );
            let vis_view = array_view(&vis);

            // Every tile claimed this frame.
            let now = 77u32;
            let tiles = [wd.div_ceil(screen::TILE), ht.div_ceil(screen::TILE)];
            let changed = vec![now; (tiles[0] * tiles[1]) as usize];
            let mut uniform = [0u32; screen::TILES_WORDS];
            uniform[..4].copy_from_slice(&[tiles[0], tiles[1], screen::CLAIMED_FOR, now]);
            let changed_buf = gpu.buffer(&changed, store());
            let tiles_buf = gpu.buffer(&uniform, wgpu::BufferUsages::UNIFORM);
            let mut list_words = vec![0u32; 4 + (tiles[0] * tiles[1]) as usize];
            let per_tile = screen::TILE / ray::CAST_SIDE;
            list_words[1..3].copy_from_slice(&[per_tile, per_tile]);
            let list = gpu.buffer(&list_words, store());
            gpu.dispatch(
                &cast_tiles_p,
                &[
                    (0, list.as_entire_binding()),
                    (30, changed_buf.as_entire_binding()),
                    (31, tiles_buf.as_entire_binding()),
                ],
                [
                    (tiles[0] * tiles[1]).div_ceil(screen::CAST_TILES_LANES),
                    1,
                    1,
                ],
            );
            let listed = gpu.read(&list)[0];
            let table_buf = gpu.buffer(&cast.table, store());
            let marks = gpu.buffer(&ray::MISS_ARGS_EMPTY, store());
            // Room past the grid's words for this screen's holes.
            let mut padded = cast.beams.clone();
            let walked_at = g.walked_at() as usize;
            padded.resize(walked_at + ray::holes_words([wd, ht]) as usize, 0);
            let beams = gpu.buffer(&padded, store());
            gpu.dispatch(
                &cast_p,
                &[
                    (0, eye_buf.as_entire_binding()),
                    (3, wgpu::BindingResource::TextureView(&rays_view)),
                    (8, beams.as_entire_binding()),
                    (9, table_buf.as_entire_binding()),
                    (13, wgpu::BindingResource::TextureView(&vis_view)),
                    (14, marks.as_entire_binding()),
                    (30, list.as_entire_binding()),
                ],
                [listed, per_tile, per_tile],
            );
            let words = gpu.read(&beams);
            let walked = |px: [u32; 2]| {
                let (w, bit) = ray::hole_bit(px, [wd, ht]);
                words[walked_at + w as usize] & 1 << bit != 0
            };
            for (p, &v) in visible.iter().enumerate() {
                if v == 0 {
                    continue;
                }
                let e = node_cache::entry_at(raster::voxel_of(v)) as usize;
                let w = |a: u32| cast.table[e + a as usize];
                let node = entry_node(&cast.table, e, cast.origin);
                let whole = w(node_cache::W_STATE) & record::WHOLE != 0;
                if record::fresh(cast.origin, &node, whole, [asked, asked]) {
                    continue;
                }
                let faces = record::asked_faces(w(node_cache::W_STATE));
                let r = &rays_of[p];
                let en =
                    cast::entry_with(cast.origin, r, r.inverse(), &node, w(node_cache::W_ASSET));
                if !(en.enters && raster::accepts(faces, ray::face(r.dir, en.axis))) {
                    continue;
                }
                stale_entered += 1;
                let px = [p as u32 % wd, p as u32 / wd];
                if !walked(px) {
                    stale_taken += 1;
                }
            }
        }
    }
    eprintln!("stale voxels a pixel's ray enters: {stale_entered}, taken unwalked: {stale_taken}");
    assert!(
        stale_entered > 100,
        "too few stale voxels entered: {stale_entered}"
    );
    assert_eq!(
        stale_taken, 0,
        "a pixel took a stale voxel as its answer and was not walked"
    );
}
