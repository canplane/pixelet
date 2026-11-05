//! `face_cs` and `cast_cs` by hand (frame/shader/face.wgsl, cast.wgsl) answer as the Rust over
//! the real world of tests/real_world, off the samples the eye's passes leave there (beam,
//! cell, sample, the samples' boxes).
//!
//! `face_cs`: each ask's bits as `face::word` gives them, every cell beside the hit asked off
//! the line down to it as `support::sub_line` and `support::asset` ask it, over the boxes'
//! asks and over asks crossed onto other lines and faces, the count and the asks past it held.
//!
//! `cast_cs`: each drawn pixel's words as `cast::entry` takes them off the visible sample
//! the test lays at it, the sky where the four samples about it (`Grid::sample_before`) went
//! on, the rest marked and listed, a tile not claimed passed by, a cell past the eye's reach
//! not entered; the walk again's words and the tally; and screens of lacks far past the list's
//! room, one past it, and one pixel. The device's own tangents, way and reciprocal of each
//! pixel are read off a probe appended to the module (`probe_cs`), which calls what `cast_cs`
//! calls, a mutant's too: its tangents and way are held near `px_tan`'s and `cast::through`'s
//! (`RAY_OFF`), its reciprocal near `Ray::inverse` (`INV_ULPS`), the four samples it takes to
//! `Grid::sample_before`'s exactly; and the Rust's entry is taken along that ray and that
//! reciprocal (`cast::entry_with`), as the device divides as it will. A plane's t is held
//! within a few ulps, more along a small component of the way, and a pixel its rounding
//! takes across an edge between the planes of two axes goes either way, one in a thousand of
//! a run's at most.
//!
//! Every word of the beam rays', the rays', the walk again's and the pyramid's compared;
//! each kind the test says something of counted, and too few of one fails. Skipped where no
//! adapter answers.

mod gpu;
mod keeping;
mod real_world;

use std::collections::BTreeSet;

use gpu::{array_view, cross, mix, read_texture, texture, ulps, unit, Gpu, Kinds, Tally};
use keeping::{q8, WorldBufs};
use pxl_engine::answer::{self, In};
use pxl_engine::beam::{self, Grid};
use pxl_engine::cast::{self, Entry, Node, Query, Ray};
use pxl_engine::overwrite::{ancestor_at, Cut};
use pxl_engine::support::{self, Ancestors};
use pxl_engine::way::tree;
use pxl_engine::world::{self, Inheritance};
use pxl_engine::{eye, face, pyramid, raster, ray, screen, tally, Zoom, LEAF_CELL};
use pxl_render::frame::shader::record::{
    src, SHADER_BEAM, SHADER_BEAM_HIT, SHADER_CAST, SHADER_CELL, SHADER_FACE, SHADER_SAMPLE,
    SHADER_SAMPLE_BOX, SHADER_SAMPLE_HIT,
};
use pxl_render::frame::shader::word::SQUARE_SIDE;

use real_world::{Built, World, FINEST};

/// East of X = 0 by the seam, and by X = 0, where X's roots part.
const EYE: [i32; 2] = [(1 << 23) - 300, (40 << 16) + 30];
const EYE_ZERO: [i32; 2] = [150, (40 << 16) + 30];
/// On X's seam, where the world goes round.
const EYE_SEAM: [i32; 2] = [(1 << 23) - 8, (40 << 16) + 30];
const FRAME: u32 = 9;
const WORKGROUP: u32 = beam::SAMPLE_SIDE;
/// A word no pass writes: one written where none should be is told apart.
const UNTOUCHED: u32 = 0xdead_beef;
/// The ulps a device's division may put a plane's t off the Rust's, and the ulps within which
/// its rounding may take a pixel across an edge of its cell, each past what its way's own
/// rounding takes (`slack`).
const T_ULPS: u32 = 4;
const EDGE_ULPS: u32 = 16;
/// The ulps the device's reciprocal of a pixel's way may lie off the Rust's correctly rounded
/// one: WGSL's bound on a division, 2.5 ulps, taken up to a whole ulp. Metal measured 1 here.
const INV_ULPS: u32 = 3;
/// How far the device's tangents and way of a pixel may lie off `px_tan`'s and
/// `cast::through`'s, on a component of at most 1: the division of a tangent, and the square
/// root, the sum and the division of the way, each within WGSL's 2.5 ulps of 1, summed and
/// taken up. Metal measured 1.5.
const RAY_OFF: f32 = 8.0 * f32::EPSILON;
/// The words the probe leaves a pixel: its tangents, its ray's way and inverse, the first of
/// the four samples about it.
const PROBE_WORDS: u32 = 10;

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
  let s = sample_before(px);
  let k = (id.y * size.x + id.x) * 10u;
  probe[k] = bitcast<u32>(px.x);
  probe[k + 1u] = bitcast<u32>(px.y);
  for (var a = 0u; a < 3u; a += 1u) {
    probe[k + 2u + a] = bitcast<u32>(dir[a]);
    probe[k + 5u + a] = bitcast<u32>(inv[a]);
  }
  probe[k + 8u] = s.x;
  probe[k + 9u] = s.y;
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
/// boxes' asks answered, the pyramid, and the boxes' count.
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
    boxes: u32,
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
/// cells' starts, the samples, their boxes and their asks, then the boxes' faces asked by
/// `face_cs`.
fn cast_of(gpu: &Gpu, b: &WorldBufs, c: &EyeCase, face_p: &wgpu::ComputePipeline) -> Cast {
    let (eye, grid, axes) = eye_of(c);
    let eye_buf = gpu.buffer(&eye, wgpu::BufferUsages::UNIFORM);
    let mut table0 = vec![0u32; pyramid::WORDS as usize];
    table0[pyramid::FRAME as usize] = FRAME;
    let table = gpu.buffer(&table0, store());
    let beams = gpu.buffer(&vec![0u32; grid.size() as usize], store());
    let left = gpu.buffer(&[0, 1, 1, 0], store());
    gpu.dispatch(
        &gpu.pipeline(src(SHADER_BEAM_HIT), "beam_hit_cs"),
        &[
            (0, eye_buf.as_entire_binding()),
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
    let beams = gpu.buffer(&words, store());
    gpu.dispatch(
        &gpu.pipeline(src(SHADER_BEAM), "beam_cs"),
        &world_binds(b, &eye_buf, &beams, &table),
        [args[0].max(1), 1, 1],
    );
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
    let left = gpu.buffer(&[0, 1, 1, 0], store());
    gpu.dispatch(
        &gpu.pipeline(src(SHADER_SAMPLE_HIT), "sample_hit_cs"),
        &[
            (0, eye_buf.as_entire_binding()),
            (8, beams.as_entire_binding()),
            (9, table.as_entire_binding()),
            (10, left.as_entire_binding()),
        ],
        [
            grid.samples[0].div_ceil(WORKGROUP),
            grid.samples[1].div_ceil(WORKGROUP),
            1,
        ],
    );
    let args = gpu.read(&left);
    let mut words = gpu.read(&beams);
    words[grid.lefts_at as usize] = args[3];
    let beams = gpu.buffer(&words, store());
    if args[3] != 0 {
        gpu.dispatch(
            &gpu.pipeline(src(SHADER_SAMPLE), "sample_cs"),
            &world_binds(b, &eye_buf, &beams, &table),
            [args[0], 1, 1],
        );
    }
    let samples = grid.samples[0] * grid.samples[1];
    let boxes = gpu.buffer(&vec![0u32; raster::words(samples) as usize], store());
    let mut laid = [0u32; raster::ARGS_WORDS as usize];
    laid[raster::ARGS_VERTICES as usize] = raster::VERTICES;
    laid[raster::ARGS_FACES as usize + 1] = 1;
    laid[raster::ARGS_FACES as usize + 2] = 1;
    laid[raster::ARGS_ASKS as usize] = grid.asks_at;
    let box_args = gpu.buffer(&laid, store());
    gpu.dispatch(
        &gpu.pipeline(src(SHADER_SAMPLE_BOX), "sample_box_cs"),
        &[
            (0, eye_buf.as_entire_binding()),
            (8, beams.as_entire_binding()),
            (14, boxes.as_entire_binding()),
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
        "{}: {} samples, {stopped} stopped, {lacks} lack, {} boxes, {} left",
        c.name,
        samples,
        laid[raster::ARGS_INSTANCES as usize],
        args[3]
    );

    gpu.dispatch(
        face_p,
        &face_binds(b, &eye_buf, &beams, &box_args),
        [laid[raster::ARGS_FACES as usize], 1, 1],
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
        boxes: laid[raster::ARGS_INSTANCES as usize],
    }
}

fn face_binds<'a>(
    b: &'a WorldBufs,
    eye: &'a wgpu::Buffer,
    beams: &'a wgpu::Buffer,
    args: &'a wgpu::Buffer,
) -> Vec<(u32, wgpu::BindingResource<'a>)> {
    vec![
        (0, eye.as_entire_binding()),
        (1, b.clip.as_entire_binding()),
        (2, b.ground.as_entire_binding()),
        (5, b.shards.as_entire_binding()),
        (6, b.placed.as_entire_binding()),
        (7, b.pool.as_entire_binding()),
        (8, beams.as_entire_binding()),
        (16, args.as_entire_binding()),
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

/// The ask at word `a` as the Rust asks it (`Want`).
fn face_want(q: &mut Asking, beams: &[u32], origin: [i32; 3], a: u32) -> Want {
    let w = |k: u32| beams[(a + k) as usize];
    let hit = node_at(beams, w(beam::ASK_NODE));
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

        // The boxes' asks, as the frame asks them.
        let mut want = cast.asked.clone();
        for i in 0..cast.boxes {
            let a = g.ask_at(i);
            let to = cast.asked[(a + beam::ASK_TO) as usize];
            kinds.add("boxes", true);
            kinds.add("boxes, none asked", to == beam::ASK_NONE);
            if to == beam::ASK_NONE {
                continue;
            }
            let Want { bits, cut } = face_want(&mut q, &cast.asked, cast.origin, a);
            kinds.add("asked, the cuts swapped answer otherwise", cut);
            want[to as usize] = bits;
            kinds.add("asked", true);
            kinds.add("asked, a rise", bits & 0xf != 0);
            kinds.add("asked, a line", bits >> face::LINE_SHIFT & 0xf != 0);
            kinds.add("asked, a corner", bits >> face::CORNER_SHIFT != 0);
            kinds.add("asked, none set", bits == 0);
        }
        tally.compare(
            &format!("{}, the boxes' asks", c.name),
            &want,
            &cast.beams,
            |k| format!("word {k}"),
        );

        // Asks crossed onto another box's line and onto every face, laid past the grid's
        // words, some none, and asks past the count that must not be answered.
        let asks: Vec<u32> = (0..cast.boxes)
            .map(|i| g.ask_at(i))
            .filter(|&a| cast.asked[(a + beam::ASK_TO) as usize] != beam::ASK_NONE)
            .collect();
        if asks.is_empty() {
            continue;
        }
        let n = (asks.len() as u32 * 2).min(6000) + 37;
        let past = 70;
        let at = g.size();
        let to_at = at + (n + past) * beam::ASK_WORDS;
        let mut laid = cast.asked.clone();
        laid.resize((to_at + n + past) as usize, UNTOUCHED);
        for i in 0..n + past {
            let h = mix(i, 0xface);
            let from = asks[(h as usize >> 4) % asks.len()];
            let other = asks[(mix(h, 3) as usize) % asks.len()];
            let w = |k: u32| cast.asked[(from + k) as usize];
            let k = (at + i * beam::ASK_WORDS) as usize;
            laid[k + beam::ASK_NODE as usize] = w(beam::ASK_NODE);
            laid[k + beam::ASK_FACE as usize] = if h.is_multiple_of(3) {
                w(beam::ASK_FACE)
            } else {
                mix(h, 7) % 6
            };
            laid[k + beam::ASK_LINE as usize] = if h.is_multiple_of(2) {
                w(beam::ASK_LINE)
            } else {
                cast.asked[(other + beam::ASK_LINE) as usize]
            };
            laid[k + beam::ASK_TO as usize] = if h % 11 == 5 {
                beam::ASK_NONE
            } else {
                to_at + i
            };
        }
        let mut want = laid.clone();
        for i in 0..n {
            let a = at + i * beam::ASK_WORDS;
            let to = laid[(a + beam::ASK_TO) as usize];
            if to == beam::ASK_NONE {
                kinds.add("crossed, none asked", true);
                continue;
            }
            let Want { bits, .. } = face_want(&mut q, &laid, cast.origin, a);
            want[to as usize] = bits;
            kinds.add("crossed", true);
            kinds.add("crossed, a rise", bits & 0xf != 0);
            kinds.add("crossed, a corner", bits >> face::CORNER_SHIFT != 0);
        }
        let eye_buf = gpu.buffer(&cast.eye, wgpu::BufferUsages::UNIFORM);
        let beams = gpu.buffer(&laid, store());
        let mut args = [0u32; raster::ARGS_WORDS as usize];
        args[raster::ARGS_INSTANCES as usize] = n;
        args[raster::ARGS_ASKS as usize] = at;
        let args = gpu.buffer(&args, store());
        gpu.dispatch(
            &face_p,
            &face_binds(&b, &eye_buf, &beams, &args),
            [(n + past).div_ceil(raster::FACES_LANES), 1, 1],
        );
        tally.compare(
            &format!("{}, {n} asks crossed", c.name),
            &want,
            &gpu.read(&beams),
            |k| format!("word {k}"),
        );
    }
    eprintln!("  KINDS faces {kinds:?}");
    let short = kinds.short(&[
        ("asked", 1000),
        ("boxes, none asked", 1),
        ("asked, a rise", 1),
        ("asked, a line", 1),
        ("asked, a corner", 1),
        ("asked, none set", 1),
        ("asked, the cuts swapped answer otherwise", 1),
        ("crossed", 1000),
        ("crossed, none asked", 1),
        ("crossed, a rise", 1),
        ("crossed, a corner", 1),
    ]);
    tally.end("faces");
    assert!(short.is_empty(), "{}", short.join("\n"));
}

/// A screen the pixels are cast on: its pixels, the eye's jitter, whether the steps view is on
/// and the tally counts, the share of the samples' reach the eye now has, one tile in so many
/// not claimed and the tiles named not claimed, and whether every pixel lacks.
struct Screen {
    size: [u32; 2],
    jitter: [f32; 2],
    steps: bool,
    tally: bool,
    reach: f32,
    unclaimed_in: u32,
    unclaimed: &'static [u32],
    lacks: bool,
}

/// The tangents of pixel `p`'s ray, as `cast_cs` takes them (grid.wgsl `px_tan`).
fn px_tan(s: &Screen, tan: [f32; 2], p: [u32; 2]) -> [f32; 2] {
    let ndc_x = (p[0] as f32 + 0.5 + s.jitter[0]) / s.size[0] as f32 * 2.0 - 1.0;
    let ndc_y = 1.0 - (p[1] as f32 + 0.5 + s.jitter[1]) / s.size[1] as f32 * 2.0;
    [ndc_x * tan[0], ndc_y * tan[1]]
}

/// The ulps a plane's t may move by as the device works out the ray's way `d` along an axis
/// itself: a sum of terms about 1, a few ulps of 1 off, is a large share of a small way. As
/// many as a u32 holds where the way is so small its share has no bound.
fn slack(d: f32) -> u32 {
    if d == 0.0 {
        0
    } else {
        (4.0 / d.abs()).ceil() as u32
    }
}

/// Whether the device's rounding may decide entry `e` otherwise along the way `dir`: where it
/// enters lies within `EDGE_ULPS` of where it leaves, or the plane it enters across within
/// `EDGE_ULPS` of another axis's near plane, each past what the two planes' own axes' slack
/// takes (`slack`); a bound the origin or the ray's reach sets moves by none.
fn at_edge(e: &Entry, dir: [f32; 3]) -> bool {
    let moves = |a: usize| dir[a] != 0.0;
    let room = |a: Option<usize>, b: Option<usize>| {
        let s = |a: Option<usize>| a.map_or(0, |a| slack(dir[a]));
        EDGE_ULPS.saturating_add(s(a)).saturating_add(s(b))
    };
    let enter = (0..3).find(|&a| moves(a) && e.near[a] == e.span[0]);
    let leave = (0..3).find(|&a| moves(a) && e.far[a] == e.span[1]);
    let axis = e.axis as usize;
    ulps(e.span[0], e.span[1]) <= room(enter, leave)
        || (0..3)
            .filter(|&a| a != axis && moves(a))
            .any(|a| ulps(e.near[a], e.near[axis]) <= room(Some(a), Some(axis)))
}

/// What the probe left of each pixel: its tangents, its ray and the first of the four
/// samples about it.
struct Probed {
    px: [f32; 2],
    dir: [f32; 3],
    inv: [f32; 3],
    before: [u32; 2],
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
            before: [w[8], w[9]],
        })
        .collect()
}

#[test]
fn hand_pixels_answer_as_cast_entry() {
    let Some(gpu) = device() else { return };
    let face_p = gpu.pipeline(src(SHADER_FACE), "face_cs");
    let cast_p = gpu.pipeline(src(SHADER_CAST), "cast_cs");
    let probe_p = gpu.pipeline(
        &format!("{}\n{PROBE}", gpu::source(src(SHADER_CAST), "cast_cs")),
        "probe_cs",
    );
    let screens = [
        Screen {
            size: [256, 160],
            jitter: [0.0, 0.0],
            steps: false,
            tally: false,
            reach: 1.0,
            unclaimed_in: 0,
            unclaimed: &[],
            lacks: false,
        },
        Screen {
            size: [301, 173],
            jitter: [0.25, -0.375],
            steps: true,
            tally: true,
            reach: 1.0,
            unclaimed_in: 3,
            unclaimed: &[],
            lacks: false,
        },
        // The eye's reach cut since its samples were cast: a cell past it is not entered.
        Screen {
            size: [199, 121],
            jitter: [-0.5, 0.125],
            steps: false,
            tally: true,
            reach: 0.4,
            unclaimed_in: 5,
            unclaimed: &[],
            lacks: false,
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
    // The sky again, in the steps view.
    runs.push((&worlds[0].0, &worlds[0].1[1], &screens[1]));
    // Every pixel lacking, more than the list holds; and one past it, two whole tiles of 1025
    // by 1025 not claimed.
    let full = Screen {
        size: [1088, 1000],
        jitter: [0.125, 0.0625],
        steps: false,
        tally: true,
        reach: 1.0,
        unclaimed_in: 0,
        unclaimed: &[],
        lacks: true,
    };
    let one_past = Screen {
        size: [1025, 1025],
        jitter: [0.0, 0.0],
        steps: false,
        tally: true,
        reach: 1.0,
        unclaimed_in: 0,
        unclaimed: &[0, 1],
        lacks: true,
    };
    runs.push((&worlds[0].0, &worlds[0].1[0], &full));
    runs.push((&worlds[0].0, &worlds[0].1[0], &one_past));
    // One pixel, lacking: the list's one row raised by its first place.
    let one = Screen {
        size: [1, 1],
        jitter: [0.0, 0.0],
        steps: false,
        tally: true,
        reach: 1.0,
        unclaimed_in: 0,
        unclaimed: &[],
        lacks: true,
    };
    runs.push((&worlds[0].0, &worlds[0].1[0], &one));
    for (built, c, s) in runs {
        let b = world_bufs(&gpu, built);
        let cast = &cast_of(&gpu, &b, c, &face_p);
        let g = &cast.grid;
        let name = format!("{}, {}x{}", c.name, s.size[0], s.size[1]);
        let [wd, ht] = s.size;
        let texels = (wd * ht) as usize;
        let mut eye = cast.eye.clone();
        eye[eye::STEPS] = u32::from(s.steps);
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

        // The device's ray against the Rust's for the same pixel, and the four samples about
        // it, which the Rust takes off the device's tangents.
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
            kinds.add("probe, an inverse not the Rust's", off != 0);
            kinds.add("probe, an inverse past INV_ULPS", off > INV_ULPS);
            kinds.add(
                "probe, the samples about it not the Rust's",
                before[p] != at.before,
            );
        }

        // The visible sample the raster would leave: mostly one of the four about the pixel,
        // some none, some a lack, some a sample far off.
        let n = g.samples[0];
        let stopped: Vec<u32> = (0..n * g.samples[1])
            .filter(|&i| {
                let k = g.sample_at([i % n, i / n]);
                cast.beams[(k + beam::SAMPLE_NODE + beam::NODE_ZOOM) as usize] != beam::SAMPLE_NONE
            })
            .collect();
        let visible: Vec<u32> = (0..wd * ht)
            .map(|p| {
                if s.lacks {
                    return raster::id_of(p % (n * g.samples[1]), true);
                }
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
                raster::id_of(i, w(beam::SAMPLE_LACKS) != 0 || h % 16 == 2)
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
                if s.unclaimed.contains(&t)
                    || s.unclaimed_in != 0 && mix(t, 5).is_multiple_of(s.unclaimed_in)
                {
                    now - screen::CLAIMED_FOR
                } else {
                    now - mix(t, 6) % screen::CLAIMED_FOR
                }
            })
            .collect();
        let claimed = |p: [u32; 2]| {
            let t = (p[1] / screen::TILE) * tiles[0] + p[0] / screen::TILE;
            screen::claimed(screen::age(now, changed[t as usize]), screen::CLAIMED_FOR)
        };
        let mut uniform = [0u32; screen::TILES_WORDS];
        uniform[..4].copy_from_slice(&[tiles[0], tiles[1], screen::CLAIMED_FOR, now]);
        let changed_buf = gpu.buffer(&changed, store());
        let tiles_buf = gpu.buffer(&uniform, wgpu::BufferUsages::UNIFORM);

        let mut table = cast.table.clone();
        table[pyramid::TALLY_ON as usize] = u32::from(s.tally);
        let table_buf = gpu.buffer(&table, store());
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
                (30, changed_buf.as_entire_binding()),
                (31, tiles_buf.as_entire_binding()),
            ],
            [wd.div_ceil(ray::CAST_SIDE), ht.div_ceil(ray::CAST_SIDE), 1],
        );

        // The Rust, pixel by pixel, along the device's ray.
        let got = read_texture(&gpu.device, &gpu.queue, &rays);
        let mut want = ray_words.clone();
        let mut walked = BTreeSet::new();
        let mut counts = [0u32; 3];
        let mut edges = 0u64;
        for (p, at) in probed.iter().enumerate() {
            let px = [p as u32 % wd, p as u32 / wd];
            if !claimed(px) {
                kinds.add("pixels, not claimed", true);
                continue;
            }
            let put = |want: &mut Vec<u32>, w: [u32; 4]| {
                for (l, v) in w.into_iter().enumerate() {
                    want[l * texels + p] = v;
                }
            };
            let v = visible[p];
            let r = Ray::new(at.dir, far);
            if v != 0 && v & raster::LACK == 0 {
                let i = raster::sample_of(v);
                let k = g.sample_at([i % n, i / n]);
                let w = |a: u32| cast.beams[(k + a) as usize];
                let node = node_at(&cast.beams, k + beam::SAMPLE_NODE);
                let asset = [w(beam::SAMPLE_ASSET), w(beam::SAMPLE_ASSET + 1)];
                // The entry along the device's reciprocal, its plane's t a few ulps off as the
                // device divides; a pixel at an edge as the device rounds it.
                let axis_was = w(beam::SAMPLE_AXIS);
                let e = cast::entry_with(cast.origin, &r, at.inv, &node, asset[0]);
                let (_, rust_axis, rust_enters) = cast::entry(cast.origin, &r, &node, asset[0]);
                kinds.add(
                    "the Rust's reciprocal decides otherwise",
                    (rust_enters && rust_axis == axis_was) != (e.enters && e.axis == axis_was),
                );
                let mut takes = e.enters && e.axis == axis_was;
                let took = got[p] != ray::MISS;
                if takes != took && at_edge(&e, at.dir) {
                    takes = took;
                    edges += 1;
                }
                if takes {
                    let face = ray::face(at.dir, axis_was);
                    let t = e.near[axis_was as usize].max(0.0);
                    let had = f32::from_bits(got[texels + p]);
                    let t = if ulps(t, had)
                        <= T_ULPS.saturating_add(slack(at.dir[axis_was as usize]))
                    {
                        had
                    } else {
                        t
                    };
                    kinds.add("took, t a few ulps off", t.to_bits() != e.t.to_bits());
                    put(
                        &mut want,
                        [
                            ray::first_word(asset[0], face, node.zoom),
                            t.to_bits(),
                            if s.steps {
                                w(beam::SAMPLE_VISITS)
                            } else {
                                asset[1]
                            },
                            w(beam::SAMPLE_FACES),
                        ],
                    );
                    counts[0] += 1;
                    kinds.add("took", true);
                    kinds.add(
                        "took, short of its cube",
                        answer::fill_eighths(asset[0]) != answer::EIGHTHS,
                    );
                    continue;
                }
                kinds.add("walked, not entered as its sample", true);
                kinds.add("walked, entered past the eye's reach", {
                    let r = Ray::new(at.dir, f32::INFINITY);
                    let e = cast::entry_with(cast.origin, &r, at.inv, &node, asset[0]);
                    e.enters && e.axis == axis_was
                });
            } else if v == 0 {
                let s0 = before[p];
                let went_on = |d: [u32; 2]| {
                    let k = g.sample_at([s0[0] + d[0], s0[1] + d[1]]);
                    cast.beams[(k + beam::SAMPLE_NODE + beam::NODE_ZOOM) as usize]
                        == beam::SAMPLE_NONE
                };
                if [[0, 0], [1, 0], [0, 1], [1, 1]].into_iter().all(went_on) {
                    let visits = cast.beams[(g.sample_at(s0) + beam::SAMPLE_VISITS) as usize];
                    put(&mut want, [0, 0, if s.steps { visits } else { 0 }, 0]);
                    counts[2] += 1;
                    kinds.add("sky", true);
                    continue;
                }
                kinds.add("walked, none visible and not the sky", true);
            } else {
                kinds.add("walked, a lack", true);
            }
            want[p] = ray::MISS;
            counts[1] += 1;
            walked.insert((px[1] << 16) | px[0]);
        }
        tally.compare(&format!("{name}: rays"), &want, &got, |k| {
            format!("pixel {} layer {}", k % texels, k / texels)
        });

        // The walk again's words: its rows raised by the place that opens each, the whole
        // screen's dispatch where one was past the list's room, the count and the tally.
        let cnt = counts[1];
        let per = ray::MISS_LANES * ray::MISS_ACROSS;
        let mut args = ray::MISS_ARGS_EMPTY;
        args[(ray::MISS_ARGS_LIST + 1) as usize] = cnt.min(pyramid::MARKS).div_ceil(per);
        if cnt > pyramid::MARKS {
            args[ray::MISS_ARGS_WHOLE as usize] = wd.div_ceil(ray::CAST_SIDE);
            args[ray::MISS_ARGS_WHOLE as usize + 1] = ht.div_ceil(ray::CAST_SIDE);
            kinds.add("past the list's room", true);
        }
        args[ray::MISS_ARGS_CNT as usize] = cnt;
        if s.tally {
            for (lane, v) in [tally::TOOK, tally::WALKED, tally::SKY]
                .into_iter()
                .zip([counts[0], counts[1], counts[2]])
            {
                args[(ray::MISS_ARGS_TALLY + lane - tally::TOOK) as usize] = v;
            }
        }
        tally.compare(
            &format!("{name}: the walk again's"),
            &args,
            &gpu.read(&marks),
            |k| format!("word {k}"),
        );

        // The list: each walked pixel once, as many as it holds, the rest of the pyramid as
        // it stood; the beam rays' words as they stood.
        let out = gpu.read(&table_buf);
        let listed = cnt.min(pyramid::MARKS) as usize;
        let at = pyramid::MARKS_AT as usize;
        let list: BTreeSet<u32> = out[at..at + listed].iter().copied().collect();
        let mut want_table = table.clone();
        want_table[at..at + listed].copy_from_slice(&out[at..at + listed]);
        let whole = list.len() == listed && list.is_subset(&walked);
        kinds.add("listed", whole);
        if !whole {
            eprintln!("{name}: the list is not the walked pixels, each once");
            tally.differ.push((format!("{name}: list"), 1));
        }
        tally.compare(&format!("{name}: pyramid"), &want_table, &out, |k| {
            format!("word {k}")
        });
        tally.compare(
            &format!("{name}: beam rays"),
            &cast.beams,
            &gpu.read(&beams),
            |k| format!("word {k}"),
        );
        eprintln!(
            "{name}: took {}, walked {}, sky {}, {edges} decided at an edge",
            counts[0], counts[1], counts[2]
        );
        kinds.count("decided at an edge", edges);
        if edges * 1000 > u64::from(counts[0]) {
            eprintln!(
                "{name}: {edges} pixels decided at an edge of {} taken",
                counts[0]
            );
            tally.differ.push((format!("{name}: edges"), edges));
        }
    }
    eprintln!(
        "  KINDS pixels {kinds:?}; the ray {dir_off:e} off the Rust's, its tangents {px_off:e}, \
         its reciprocal {inv_off} ulps"
    );
    let short = kinds.short(&[
        ("took", 1000),
        ("took, short of its cube", 1),
        ("sky", 100),
        ("walked, not entered as its sample", 100),
        ("walked, none visible and not the sky", 10),
        ("walked, a lack", 100),
        ("walked, entered past the eye's reach", 10),
        ("pixels, not claimed", 100),
        ("past the list's room", 2),
        ("listed", 1),
    ]);
    tally.end("pixels");
    assert!(short.is_empty(), "{}", short.join("\n"));
    assert!(
        dir_off <= RAY_OFF && px_off <= RAY_OFF,
        "the device's ray {dir_off:e} off the Rust's, its tangents {px_off:e}"
    );
    assert!(
        kinds.get("probe, an inverse past INV_ULPS") == 0,
        "the device's reciprocal {inv_off} ulps off the Rust's"
    );
    assert!(
        kinds.get("probe, the samples about it not the Rust's") == 0,
        "the device's four samples about a pixel not `Grid::sample_before`'s"
    );
}
