//! Checks that the holes' queue over the light's slots (frame/shader/queue.wgsl,
//! queue_walk.wgsl, queue_ask.wgsl, queue_last.wgsl) answers as pxl-engine's walk does, over
//! the real world of tests/real_world.
//!
//! Over an empty node cache, each listed slot's word must be `light::word(light::depth(..))` of
//! its walk, and no other slot may be written. In the next frame, over the node cache the first
//! frame left, each slot's word must again be the world's. Where bricks changed between the
//! frames, the next frame's words, after the app's forgetting (`forget_cs`), must be the
//! changed world's. A change to the world must also keep the tiles it does not reach.
//!
//! Every walk starts from its cell's line, and the cells are laid as the cast lays them each
//! frame (beam.light.wgsl, light_cell.wgsl). The tests are skipped where no adapter answers.

mod gpu;
mod real_world;

use std::collections::BTreeMap;

use gpu::{array_view, read_texture, texture, Gpu};

use pxl_engine::answer;
use pxl_engine::cast::{self, Detail, End, Ray, Until};
use pxl_engine::clipmap::{self, Entry, ENTRIES, FIRST_CELL_ZOOM};
use pxl_engine::light::{self, Light, StartGrid};
use pxl_engine::node_cache::{self, CHANGED_WORDS};
use pxl_engine::object::Cut;
use pxl_engine::{beam, eye, object, ray, value, Zoom, FRAC_W, LEAF_CELL_ZOOM};
use pxl_render::frame::shader::record::{src, SHADER_FORGET, SHADER_LIGHT_CELL};
use pxl_render::frame::shader::word::ROW_LANES;

use real_world::{q8, store_of, truth, Asking, Built, World, WorldBuffer, FINEST, NOTCH};

/// The leaf cell the eye stands in: 300 cells west of X's seam, on the east side of X = 0,
/// and 30 cells north of the service area's south edge.
const EYE: [i32; 2] = [(1 << 23) - 300, (40 << 16) + 30];
/// An eye west of X = 0, 600 cells from `EYE` across the seam.
const EYE_WEST: [i32; 2] = [-(1 << 23) + 300, (40 << 16) + 30];
/// An eye by X = 0, where one root ends and the next begins in X.
const EYE_ZERO: [i32; 2] = [150, (40 << 16) + 30];
/// An eye on X's seam, where the world goes round.
const EYE_SEAM: [i32; 2] = [(1 << 23) - 8, (40 << 16) + 30];

const SIZE: u32 = 256;
/// How many steps a GPU's rounding may move a slot's word. The depth's product may be fused
/// into the push added after it, and its product with `light::DEPTH_ONE` into the half added
/// after it, and each moves the word by a step. Nothing is divided, since the light's
/// reciprocals and starts are the host's.
const WITHIN: u32 = 2;
const FRAME: u32 = 9;
/// How many differing slots of a case are told in full. A wrong shader differs in nearly
/// every slot, so the rest are only counted.
const TELLS: u32 = 4;

/// The light box of case `c` and where its rays start, laid as the host lays them (pxl-engine
/// light.rs `StartGrid`).
fn box_of(c: &Case) -> (Light, StartGrid) {
    let l = libm::sqrtf(c.dir[0] * c.dir[0] + c.dir[1] * c.dir[1] + c.dir[2] * c.dir[2]);
    let dir = c.dir.map(|v| v / l);
    let cross = |a: [f32; 3], b: [f32; 3]| {
        [
            a[1] * b[2] - a[2] * b[1],
            a[2] * b[0] - a[0] * b[2],
            a[0] * b[1] - a[1] * b[0],
        ]
    };
    let unit = |v: [f32; 3]| {
        let l = libm::sqrtf(v[0] * v[0] + v[1] * v[1] + v[2] * v[2]);
        v.map(|x| x / l)
    };
    let a = cross(dir, [0.0, 0.0, 1.0]);
    let a = if a == [0.0; 3] {
        [1.0, 0.0, 0.0]
    } else {
        unit(a)
    };
    let across = a.map(|v| v * c.side);
    let down = unit(cross(dir, a)).map(|v| v * c.side);
    let half = SIZE as f32 / 2.0;
    let s = Light {
        first: [0, 1, 2].map(|k| -half * (across[k] + down[k]) - c.back * dir[k]),
        t_max: c.t_max,
        across,
        down,
        dir,
        bias: [0.00025, 0.0005, 0.00075],
        corner: c.corner,
        reach_ndc: 1.0,
    };
    let base = c
        .corner
        .map(|v| v.div_euclid(light::ANCHOR) * light::ANCHOR);
    let q = (1u32 << FRAC_W) as f32;
    let anchor = |k: usize| {
        let w = [
            base[0] + (k % 2) as i32 * light::ANCHOR,
            base[1] + (k / 2) as i32 * light::ANCHOR,
        ];
        let (i, j) = ((w[0] - c.corner[0]) as f32, (w[1] - c.corner[1]) as f32);
        let v = |a: usize| (s.first[a] + i * across[a] + j * down[a]) * q;
        (
            [0, 1, 2].map(|a| c.origin[a].wrapping_add(libm::rintf(v(a)) as i32)),
            [0, 1, 2].map(|a| v(a) - libm::rintf(v(a))),
        )
    };
    let [a0, a1, a2, a3] = [0, 1, 2, 3].map(anchor);
    let starts = StartGrid {
        base,
        across: across.map(|v| light::fixed(v * q)),
        down: down.map(|v| light::fixed(v * q)),
        at: [a0.0, a1.0, a2.0, a3.0],
        frac: [a0.1, a1.1, a2.1, a3.1].map(|f| f.map(light::fixed)),
    };
    (s, starts)
}

/// A shadow map's box over some of the world.
struct Case {
    name: &'static str,
    /// Where the box's middle lies, in the Q3 steps a ray's start is counted in.
    origin: [i32; 3],
    dir: [f32; 3],
    /// A texel's side, in units, and how far back along the light its near plane lies.
    side: f32,
    back: f32,
    t_max: f32,
    range: [Zoom; 2],
    corner: [i32; 2],
    cuts: [Cut; 2],
    /// The fewest of each kind of case this seeded case must meet.
    least: &'static [(&'static str, u32)],
}

/// The ray of `slot`, with where it starts, and the detail it asks: the one band of its tile
/// (`Light::band`).
fn slot_ray(c: &Case, s: &Light, starts: &StartGrid, slot: [u32; 2]) -> ([i32; 3], Ray, Detail) {
    let p = s.texel(SIZE, slot);
    let (o, r) = light::ray(s, starts, p);
    let tile = [(p[0] / light::TILE) as i32, (p[1] / light::TILE) as i32];
    let band = s.band(SIZE, c.range, tile);
    (
        o,
        r,
        Detail {
            range: [band, band],
            spacing: s.spacing(),
        },
    )
}

fn cases(b: &Built) -> Vec<Case> {
    let none = [Cut::NONE; 2];
    let over = |g: [i32; 2], up: i32| q8([g[0], g[1], b.top_at(g) + up]);
    // A point on a way of the lines `lines` where the world answers with a way at the leaf
    // zoom.
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
    let on_placed = [leaf_c[0] >> 1, leaf_c[1] >> 1];
    let bridge = way([0, 3]);
    let bore = way([1, 4]);
    vec![
        Case {
            name: "about the eye",
            origin: over([EYE[0] + 90, EYE[1] + 90], 40),
            dir: [0.3, 0.2, -0.93],
            side: 1.0,
            back: 60.0,
            t_max: 400.0,
            range: [8, FINEST],
            corner: [1300, -900],
            cuts: none,
            least: &[
                ("hit", 19850),
                ("hit, a way's", 1161),
                ("hit, placed", 13329),
                ("hit, short of its cube", 364),
                ("slots", 19850),
            ],
        },
        Case {
            name: "about the eye, slanting",
            origin: over([EYE[0] + 90, EYE[1] + 90], 40),
            dir: [0.3, 0.2, -0.93],
            side: 1.0,
            back: 60.0,
            t_max: 400.0,
            range: [8, FINEST],
            corner: [1300, -900],
            cuts: none,
            least: &[
                ("hit", 19850),
                ("hit, a way's", 1161),
                ("hit, placed", 13329),
                ("hit, short of its cube", 364),
                ("slots", 19850),
            ],
        },
        Case {
            name: "wide, over the clip levels",
            origin: over([EYE[0] + 3000, EYE[1] + 3000], 900),
            dir: [0.2, -0.3, -0.93],
            side: 32.0,
            back: 200.0,
            t_max: 4000.0,
            range: [8, FINEST],
            corner: [77, 5],
            cuts: none,
            least: &[
                ("hit", 19850),
                ("hit, a way's", 1033),
                ("hit, short of its cube", 7909),
                ("hit, the wall's", 2894),
                ("slots", 19850),
            ],
        },
        Case {
            name: "on what is placed, cut and hid",
            origin: over(on_placed, 30),
            dir: [0.1, 0.25, -0.96],
            side: 0.25,
            back: 10.0,
            t_max: 120.0,
            range: [LEAF_CELL_ZOOM, FINEST],
            corner: [8, 16],
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
            least: &[
                ("hit", 19850),
                ("hit, a way's", 9),
                ("hit, placed", 4970),
                ("slots", 19850),
            ],
        },
        Case {
            name: "on a bridge",
            origin: q8([bridge[0], bridge[1], bridge[2] + 30]),
            dir: [0.3, -0.1, -0.95],
            side: 0.5,
            back: 10.0,
            t_max: 200.0,
            range: [LEAF_CELL_ZOOM, FINEST],
            corner: [-16, 32],
            cuts: none,
            least: &[
                ("hit", 18559),
                ("hit, a way's", 210),
                ("hit, placed", 13302),
                ("past its reach", 1290),
                ("slots", 19850),
            ],
        },
        Case {
            name: "down a bore",
            origin: q8([bore[0], bore[1], bore[2] + 20]),
            dir: [-0.2, 0.15, -0.97],
            side: 0.5,
            back: 10.0,
            t_max: 200.0,
            range: [LEAF_CELL_ZOOM - 2, FINEST],
            corner: [0, 0],
            cuts: none,
            least: &[
                ("hit", 18988),
                ("hit, placed", 9572),
                ("hit, the wall's", 2567),
                ("past its reach", 861),
                ("slots", 19850),
            ],
        },
        Case {
            name: "the area's wall, and past it",
            origin: over([NOTCH[0] << 16, NOTCH[1] << 16], 200),
            dir: [0.25, 0.1, -0.96],
            side: 8.0,
            back: 100.0,
            t_max: 2000.0,
            range: [8, FINEST],
            corner: [3, -7],
            cuts: none,
            least: &[
                ("hit", 19850),
                ("hit, short of its cube", 9110),
                ("hit, the wall's", 5631),
                ("slots", 19850),
            ],
        },
        Case {
            name: "across the seam",
            origin: over([(1 << 23) - 8, EYE[1] + 200], 12),
            dir: [0.9, 0.1, -0.05],
            side: 0.25,
            back: 0.0,
            t_max: 600.0,
            range: [8, FINEST],
            corner: [128, 128],
            cuts: none,
            least: &[("hit", 5380), ("past its reach", 14469), ("slots", 19850)],
        },
        Case {
            // The rays run on the planes between cells, X and Y each a whole unit. A ray on such
            // a plane belongs to the cell on its upper side.
            name: "straight down",
            origin: {
                let g = [EYE[0] + 40, EYE[1] + 40];
                [g[0] << FRAC_W, g[1] << FRAC_W, (b.top_at(g) + 30) << FRAC_W]
            },
            dir: [0.0, 0.0, -1.0],
            side: 1.0,
            back: 20.0,
            t_max: 300.0,
            range: [8, FINEST],
            corner: [16, 0],
            cuts: none,
            least: &[
                ("hit", 15437),
                ("hit, a way's", 260),
                ("hit, placed", 11960),
                ("hit, short of its cube", 316),
                ("past its reach", 4412),
                ("slots", 19850),
            ],
        },
        Case {
            name: "from under the ground",
            origin: over([EYE[0] + 100, EYE[1] + 100], -40),
            dir: [0.7, 0.5, 0.5],
            side: 0.5,
            back: 0.0,
            t_max: 200.0,
            range: [LEAF_CELL_ZOOM - 2, FINEST],
            corner: [8, 8],
            cuts: none,
            least: &[
                ("hit", 19457),
                ("hit, a way's", 129),
                ("hit, placed", 4101),
                ("past its reach", 392),
                ("slots", 19850),
            ],
        },
        Case {
            name: "through corners",
            origin: {
                let g = [EYE[0] + 40, EYE[1] + 40];
                [g[0] << FRAC_W, g[1] << FRAC_W, (b.top_at(g) + 30) << FRAC_W]
            },
            dir: [0.6, 0.6, -0.53],
            side: core::f32::consts::SQRT_2,
            back: 30.0,
            t_max: 400.0,
            range: [8, FINEST],
            corner: [0, 0],
            cuts: none,
            least: &[
                ("hit", 19612),
                ("hit, a way's", 675),
                ("hit, placed", 9507),
                ("hit, short of its cube", 343),
                ("hit, the wall's", 187),
                ("past its reach", 237),
                ("slots", 19850),
            ],
        },
        Case {
            name: "where no clip level holds the ground",
            origin: q8([1 << 15, 1 << 15, 40]),
            dir: [0.3, 0.2, -0.93],
            side: 8.0,
            back: 100.0,
            t_max: 800.0,
            range: [8, FINEST],
            corner: [0, 0],
            cuts: none,
            least: &[("lacks", 19850), ("slots", 19850)],
        },
        Case {
            name: "down past the roots",
            origin: q8([30, -(1 << 23) + 30, 1000]),
            dir: [-0.7, -0.7, 0.1],
            side: 0.25,
            back: 0.0,
            t_max: 200.0,
            range: [8, FINEST],
            corner: [0, 0],
            cuts: none,
            least: &[
                ("ended in another root", 19850),
                ("past its reach", 19850),
                ("slots", 19850),
            ],
        },
        Case {
            name: "past the pole and the height",
            origin: q8([EYE[0], (1 << 23) - 60, (1 << 15) - 60]),
            dir: [0.1, 0.7, 0.7],
            side: 1.0,
            back: 0.0,
            t_max: 300.0,
            range: [8, FINEST],
            corner: [8, 8],
            cuts: none,
            least: &[
                ("ended in another root", 16610),
                ("past its reach", 19850),
                ("slots", 19850),
            ],
        },
    ]
}

#[test]
fn hand_light_holes_answer_as_cast_walk() {
    let Some(gpu) = Gpu::new() else {
        eprintln!("no adapter: skipped");
        return;
    };

    let built = real_world::build(EYE);
    let w = built.world();
    let store = wgpu::BufferUsages::STORAGE;
    let flat_u = |rows: &[[u32; 4]]| rows.iter().flatten().copied().collect::<Vec<u32>>();
    let clip_flat: Vec<u32> = built.clipmap.iter().flatten().map(|&v| v as u32).collect();
    let bound = WorldBuffer {
        clip: gpu.buf(&clip_flat, store),
        ground: gpu.buf(&built.ground, store),
        palette: gpu.buf(&built.palette, store),
        shards: gpu.buf(&flat_u(&built.shards), store),
        placed: gpu.buf(&built.objects, store),
        pool: gpu.buf(&built.pool, store),
    };

    let mut failed = Vec::new();
    for case in cases(&built) {
        run_case(&gpu, &bound, w, &case, None, &mut failed);
    }
    // West of X = 0, every node's X is negative as the world names it, and the node cache must
    // keep, find and read such nodes.
    let west = real_world::build(EYE_WEST);
    let bound_west = WorldBuffer {
        clip: gpu.buf(
            &west
                .clipmap
                .iter()
                .flatten()
                .map(|&v| v as u32)
                .collect::<Vec<u32>>(),
            store,
        ),
        ground: gpu.buf(&west.ground, store),
        palette: gpu.buf(&west.palette, store),
        shards: gpu.buf(&flat_u(&west.shards), store),
        placed: gpu.buf(&west.objects, store),
        pool: gpu.buf(&west.pool, store),
    };
    let over = |g: [i32; 2], up: i32| q8([g[0], g[1], west.top_at(g) + up]);
    let case = Case {
        name: "about an eye west of X = 0",
        origin: over([EYE_WEST[0] + 90, EYE_WEST[1] + 90], 40),
        dir: [0.3, 0.2, -0.93],
        side: 1.0,
        back: 60.0,
        t_max: 400.0,
        range: [8, FINEST],
        corner: [1300, -900],
        cuts: [Cut::NONE; 2],
        least: &[("hit", 19850), ("slots", 19850)],
    };
    run_case(&gpu, &bound_west, west.world(), &case, None, &mut failed);
    // Over X = 0, where one root ends and the next begins in X, rays end in the next root.
    let zero = real_world::build(EYE_ZERO);
    let bound_zero = WorldBuffer {
        clip: gpu.buf(
            &zero
                .clipmap
                .iter()
                .flatten()
                .map(|&v| v as u32)
                .collect::<Vec<u32>>(),
            store,
        ),
        ground: gpu.buf(&zero.ground, store),
        palette: gpu.buf(&zero.palette, store),
        shards: gpu.buf(&flat_u(&zero.shards), store),
        placed: gpu.buf(&zero.objects, store),
        pool: gpu.buf(&zero.pool, store),
    };
    let case = Case {
        name: "over X = 0",
        origin: q8([
            10,
            EYE_ZERO[1] + 90,
            zero.top_at([10, EYE_ZERO[1] + 90]) + 40,
        ]),
        dir: [0.3, 0.2, -0.93],
        side: 1.0,
        back: 60.0,
        t_max: 400.0,
        range: [8, FINEST],
        corner: [1300, -900],
        cuts: [Cut::NONE; 2],
        least: &[("ended in another root", 1467), ("slots", 19850)],
    };
    run_case(&gpu, &bound_zero, zero.world(), &case, None, &mut failed);
    assert!(failed.is_empty(), "{}", failed.join("\n"));
}

/// The slots listed are all but the last `NOT_LISTED` and one in `SKIPPED_EVERY`. No walk may
/// write the rest.
const NOT_LISTED: u32 = 30;
const SKIPPED_EVERY: u32 = 11;
/// The cast the head names. Its epoch begins with it, so every tile is read.
const CAST: u32 = 7;

/// Lays the beam's lines and each cell's start in `list`, as the cast lays them
/// (beam.light.wgsl, light_cell.wgsl). Every tile is read, and the beam runs over a node cache
/// of its own.
fn lay_cells(gpu: &Gpu, b: &WorldBuffer, [eye, header]: [&wgpu::Buffer; 2], list: &mut [u32]) {
    let tiles = light::tiles(SIZE) as usize;
    let reads = light::reads_at(SIZE) as usize;
    list[reads..reads + tiles * tiles].fill(CAST);
    let mut table = vec![0u32; node_cache::WORDS as usize];
    table[node_cache::FRAME as usize] = FRAME;
    let table = gpu.buf(&table, store_of());
    let buf = gpu.buf(list, store_of());
    let side = beam::SAMPLE_SIDE;
    gpu.light_beam(
        &[
            (0, eye.as_entire_binding()),
            (1, b.clip.as_entire_binding()),
            (2, b.ground.as_entire_binding()),
            (4, b.palette.as_entire_binding()),
            (5, b.shards.as_entire_binding()),
            (6, b.placed.as_entire_binding()),
            (7, b.pool.as_entire_binding()),
            (8, buf.as_entire_binding()),
            (9, table.as_entire_binding()),
            (11, header.as_entire_binding()),
        ],
        &buf,
        &table,
        SIZE,
    );
    let cells = light::cells_across(SIZE);
    gpu.dispatch(
        &gpu.pipeline(src(SHADER_LIGHT_CELL), "light_cell_cs"),
        &[
            (0, eye.as_entire_binding()),
            (8, buf.as_entire_binding()),
            (11, header.as_entire_binding()),
        ],
        [cells.div_ceil(side), cells.div_ceil(side), 1],
    );
    // The beam's lines and hands and the cells' starts all lie before the holes' list
    // (`light::holes_at`), so the copy leaves the slots listed in `list` as they were.
    let laid = light::holes_at(SIZE) as usize;
    list[..laid].copy_from_slice(&gpu.read(&buf)[..laid]);
}

/// The world the next frame casts over, with bricks changed since the first frame, as the app
/// hands it: its buffers, the words that name the moved entries and the changed bricks
/// (gpu/ray/cast.rs), and the pass that forgets by them.
struct Edited<'a> {
    b: &'a WorldBuffer,
    w: World<'a>,
    changed: [u32; CHANGED_WORDS as usize],
    changed_bricks: [[i32; 4]; clipmap::EYE_CLIP_LEVELS as usize],
    forget: &'a wgpu::ComputePipeline,
}

fn run_case(
    gpu: &Gpu,
    b: &WorldBuffer,
    w: World,
    case: &Case,
    edited: Option<&Edited>,
    failed: &mut Vec<String>,
) {
    let name = case.name;
    let (s, starts) = box_of(case);

    let mut eye_words = vec![0u32; eye::WORDS];
    eye_words[eye::LIGHT..eye::LIGHT + eye::LIGHT_WORDS].copy_from_slice(&s.words());
    eye_words[eye::RANGE] = case.range[0];
    eye_words[eye::RANGE + 1] = case.range[1];
    for (at, cut) in [(eye::CARVE, case.cuts[0]), (eye::HIDE, case.cuts[1])] {
        eye_words[at] = cut.at[0] as u32;
        eye_words[at + 1] = cut.at[1] as u32;
        eye_words[at + 2] = cut.at[2] as u32;
        eye_words[at + 3] = cut.zoom;
    }
    let mut header = vec![0u32; light::HEADER_WORDS];
    header[light::HEADER_CAST] = CAST;
    header[light::HEADER_EPOCH] = CAST;
    for (k, v) in light::header_layout(SIZE) {
        header[k] = v;
    }
    header[light::STARTS_AT * 4..(light::STARTS_AT + light::STARTS_VEC4S) * 4]
        .copy_from_slice(&starts.words());
    let eye_buf = gpu.buf(&eye_words, wgpu::BufferUsages::UNIFORM);
    let header_buf = gpu.buf(&header, wgpu::BufferUsages::UNIFORM);

    // The slots are listed row after row of the map, in the holes' list the queue takes.
    let n = SIZE * SIZE;
    let walked: Vec<bool> = (0..n)
        .map(|k| k < n - NOT_LISTED && !k.is_multiple_of(SKIPPED_EVERY))
        .collect();
    let slots: Vec<u32> = (0..n)
        .filter(|&k| walked[k as usize])
        .map(|k| (k % SIZE) | ((k / SIZE) << 16))
        .collect();
    let mut list = list_of(&slots);

    let mut table0 = vec![0u32; node_cache::WORDS as usize];
    table0[node_cache::FRAME as usize] = FRAME;
    // The Rust's words, walked a slot at a time, every node asked of the world.
    let mut want = vec![u32::MAX; n as usize];
    let mut kinds: BTreeMap<&'static str, u32> = BTreeMap::new();
    {
        let mut q = Asking { w, cuts: case.cuts };
        for k in 0..n {
            if !walked[k as usize] {
                continue;
            }
            let (o, r, d) = slot_ray(case, &s, &starts, [k % SIZE, k / SIZE]);
            let t = cast::first(&mut q, o, &r, d, Until::Opaque);
            want[k as usize] = light::word(light::depth(&s, &t));
            let mut add = |name, yes: bool| *kinds.entry(name).or_default() += u32::from(yes);
            add("slots", true);
            add("hit", t.end == End::Hit);
            add("past its reach", t.end == End::Miss);
            add("lacks", t.end == End::Lacks);
            let a = t.hit.asset[0];
            let hits = t.end == End::Hit;
            add("hit, placed", hits && answer::is_placed(a));
            add(
                "hit, short of its cube",
                hits && answer::fill_eighths(a) < 8,
            );
            add("hit, a way's", hits && value::transports(answer::val(a)));
            add("ended in another root", t.hit.node.roots != [0; 3]);
            add("hit, the wall's", hits && answer::val(a) == value::UNKNOWN);
            add("hit, on z", hits && t.hit.axis == 2);
            add("hit, on x", hits && t.hit.axis == 0);
            add("hit, on y", hits && t.hit.axis == 1);
        }
    }

    // Every slot's walk resumes from its cell's start, which the beam the cast walks first lays.
    lay_cells(gpu, b, [&eye_buf, &header_buf], &mut list);

    // The queue runs over a node cache that holds nothing yet.
    let map = map_of(gpu);
    let view = array_view(&map);
    let table_buf = gpu.buf(&table0, store_of());
    let list_buf = gpu.buf(&list, store_of());
    holes(
        gpu,
        b,
        [&eye_buf, &header_buf, &table_buf, &list_buf],
        &view,
        &walked,
    );
    let got = read_texture(&gpu.device, &gpu.queue, &map);
    let table_hand = gpu.read(&table_buf);

    // Each listed slot's word must lie within `WITHIN` of the Rust's, or one axis's push back
    // off it (`an_axis_over`). A slot not listed must stay 0.
    let (mut same, mut axis_over, mut differ) = (0u32, 0u32, 0u32);
    let mut first_differ = None;
    for k in 0..n as usize {
        if !walked[k] {
            if got[k] != 0 {
                differ += 1;
                first_differ.get_or_insert((k, got[k], 0));
            }
            continue;
        }
        if got[k].abs_diff(want[k]) <= WITHIN {
            same += 1;
            continue;
        }
        if an_axis_over(&s, got[k], want[k]) {
            axis_over += 1;
        } else {
            differ += 1;
            first_differ.get_or_insert((k, got[k], want[k]));
            if differ <= TELLS {
                tell_slot(w, case, &s, &starts, k as u32, [got[k], want[k]]);
            }
        }
    }

    // The next frame runs the queue again, over the node cache the first frame left.
    let mut next = table_hand.clone();
    next[node_cache::FRAME as usize] = FRAME + 1;
    if let Some(e) = edited {
        next[node_cache::FORGETS as usize] = node_cache::FORGETS_CHANGED;
        let m = node_cache::CHANGED_AT as usize;
        next[m..m + CHANGED_WORDS as usize].copy_from_slice(&e.changed);
        for (k, b) in e.changed_bricks.iter().enumerate() {
            let at = node_cache::CHANGED_BRICKS_AT as usize + 4 * k;
            next[at..at + 4].copy_from_slice(&b.map(|v| v as u32));
        }
    }
    let next_buf = gpu.buf(&next, store_of());
    if let Some(e) = edited {
        gpu.dispatch(
            e.forget,
            &[(9, next_buf.as_entire_binding())],
            [node_cache::ENTRIES.div_ceil(ROW_LANES), 1, 1],
        );
    }
    // Where the world changed, the next frame must answer as the changed world.
    let mut changed = 0u32;
    if let Some(e) = edited {
        let mut q = Asking {
            w: e.w,
            cuts: case.cuts,
        };
        for k in 0..n {
            if walked[k as usize] {
                let (o, r, d) = slot_ray(case, &s, &starts, [k % SIZE, k / SIZE]);
                let t = cast::first(&mut q, o, &r, d, Until::Opaque);
                let now = light::word(light::depth(&s, &t));
                changed += u32::from(now.abs_diff(want[k as usize]) > WITHIN);
                want[k as usize] = now;
            }
        }
    }
    let b = edited.map_or(b, |e| e.b);
    let w = edited.map_or(w, |e| e.w);
    let mut list = list_of(&slots);
    lay_cells(gpu, b, [&eye_buf, &header_buf], &mut list);
    let list2 = gpu.buf(&list, store_of());
    let map2 = map_of(gpu);
    let view2 = array_view(&map2);
    holes(
        gpu,
        b,
        [&eye_buf, &header_buf, &next_buf, &list2],
        &view2,
        &walked,
    );
    let got2 = read_texture(&gpu.device, &gpu.queue, &map2);
    // In the next frame each slot is ended by the node cache or walked again, and its word
    // must again be the world's.
    let mut next_differ = 0u32;
    let mut next_axis_over = 0u32;
    let mut first_next = None;
    for k in 0..n as usize {
        if walked[k] && got2[k].abs_diff(want[k]) > WITHIN {
            if an_axis_over(&s, got2[k], want[k]) {
                next_axis_over += 1;
            } else {
                next_differ += 1;
                first_next.get_or_insert((k, got2[k], want[k]));
                if next_differ <= TELLS {
                    tell_slot(w, case, &s, &starts, k as u32, [got2[k], want[k]]);
                }
            }
        }
    }

    eprintln!(
        "{name}: {same} alike, {axis_over} an axis over, {differ} differ of {} ; {kinds:?}",
        kinds["slots"]
    );
    eprintln!("  next frame: {next_axis_over} an axis over, {next_differ} differ");
    let mut fail = |what: String| failed.push(format!("{name}: {what}"));
    if differ != 0 {
        fail(format!("{differ} slots differ, first {first_differ:?}"));
    }
    if axis_over > 256 {
        fail(format!("too many an axis over: {axis_over}"));
    }
    if next_differ != 0 {
        fail(format!(
            "{next_differ} differ the next frame, first slot, got, wanted {first_next:?}"
        ));
    }
    if edited.is_some() && changed < 1000 {
        fail(format!("too few slots the edit changes to say: {changed}"));
    }
    if next_axis_over > 256 {
        fail(format!(
            "too many an axis over the next frame: {next_axis_over}"
        ));
    }
    for (what, n) in case.least {
        let got = kinds.get(what).copied().unwrap_or(0);
        if got < *n {
            fail(format!("too few of {what} to say: {got}, wanted {n}"));
        }
    }
}

/// Whether `got` is `want` for a ray that enters at an edge, where a GPU's rounding of the
/// planes makes it enter across another axis. The two words then lie apart by the difference
/// of the two axes' push back alone.
fn an_axis_over(s: &Light, got: u32, want: u32) -> bool {
    let apart = got.abs_diff(want);
    let one = light::DEPTH_ONE as f32;
    [(0, 1), (0, 2), (1, 2)].iter().any(|&(a, b)| {
        let by = (libm::fabsf(s.bias[a] - s.bias[b]) * one) as u32;
        apart.abs_diff(by) <= WITHIN
    })
}

/// Prints a slot that differs: its words and where its walk stops.
fn tell_slot(w: World, case: &Case, s: &Light, starts: &StartGrid, k: u32, [got, want]: [u32; 2]) {
    let (o, r, d) = slot_ray(case, s, starts, [k % SIZE, k / SIZE]);
    let mut q = Asking { w, cuts: case.cuts };
    let t = cast::first(&mut q, o, &r, d, Until::Opaque);
    let (z, c) = (t.hit.node.zoom, t.hit.node.idx(o));
    eprintln!(
        "DIFFERS {} slot {k} got {got} want {want} band {:?}: its walk stops {:?} z{z} {c:?} t {} axis {} asset {:#x}",
        case.name, d.range, t.end, t.hit.t, t.hit.axis, t.hit.asset[0]
    );
}

/// The map the passes write, one word a texel and `SIZE` texels a side, laid with zeros.
fn map_of(gpu: &Gpu) -> wgpu::Texture {
    let words = vec![0; (SIZE * SIZE) as usize];
    texture(
        &gpu.device,
        &gpu.queue,
        [SIZE, SIZE],
        1,
        wgpu::TextureUsages::STORAGE_BINDING,
        &words,
    )
}

/// The light's list with `slots` in its holes' list (`light::holes_at`), and room past it for
/// its pool and its stops (`light::buf_words`).
fn list_of(slots: &[u32]) -> Vec<u32> {
    let mut list = vec![0u32; light::buf_words(SIZE) as usize];
    let at = light::holes_at(SIZE) as usize;
    list[at] = slots.len() as u32;
    list[at + 1..at + 1 + slots.len()].copy_from_slice(slots);
    list
}

/// Runs the holes' queue over the slots the list holds (`Gpu::holes`), writing the map `view`.
/// Each stop it keeps in the list (queue.wgsl `finish`, `light::hits_at`) must name a slot that
/// `walked` lists, and no slot may have two.
fn holes(
    gpu: &Gpu,
    b: &WorldBuffer,
    [eye, header, table, list]: [&wgpu::Buffer; 4],
    view: &wgpu::TextureView,
    walked: &[bool],
) {
    let binds = [
        (0, eye.as_entire_binding()),
        (1, b.clip.as_entire_binding()),
        (2, b.ground.as_entire_binding()),
        (3, wgpu::BindingResource::TextureView(view)),
        (4, b.palette.as_entire_binding()),
        (5, b.shards.as_entire_binding()),
        (6, b.placed.as_entire_binding()),
        (7, b.pool.as_entire_binding()),
        (8, list.as_entire_binding()),
        (9, table.as_entire_binding()),
        (11, header.as_entire_binding()),
    ];
    let pool = light::list_words(SIZE) + ray::POOL_CNT;
    gpu.holes(&binds, table, (list, pool));
    let hits = gpu.read(table)[node_cache::HITS_CNT as usize].min(ray::HITS_A_FRAME);
    let words = gpu.read(list);
    let at = light::hits_at(SIZE) as usize;
    let mut seen = std::collections::BTreeSet::new();
    let mut strays = 0u32;
    for r in 0..hits as usize {
        let px = words[at + r * beam::HIT_WORDS as usize + beam::HIT_PX as usize];
        let k = ((px >> 16) * SIZE + (px & 0xffff)) as usize;
        if k >= walked.len() || !walked[k] || !seen.insert(k) {
            strays += 1;
        }
    }
    assert_eq!(strays, 0, "hits of slots not walked, or twice, of {hits}");
}

/// The node cache must never answer the world as it was. A frame's walks keep the answers of a
/// world whose leaf clip level lacks its bricks. Every brick of that level is then handed in,
/// and the next frame, after the app forgets by the entries that moved, must answer as the
/// world that holds them.
#[test]
fn an_edited_brick_is_never_answered_as_it_was() {
    handed_between_frames(
        EYE,
        LEAF_CELL_ZOOM,
        [90, 90],
        "about the eye, its leaf bricks handed between the frames",
    );
}

/// `an_edited_brick_is_never_answered_as_it_was` at a clip level coarser than the leaves'.
#[test]
fn an_edited_coarse_brick_is_never_answered_as_it_was() {
    handed_between_frames(
        EYE,
        LEAF_CELL_ZOOM - 1,
        [90, 90],
        "about the eye, a coarser level's bricks handed between the frames",
    );
}

/// `an_edited_brick_is_never_answered_as_it_was` on X's seam, where the world goes round, with
/// the leaf clip level's bricks on both sides of it.
#[test]
fn an_edited_brick_over_the_seam_is_never_answered_as_it_was() {
    handed_between_frames(
        EYE_SEAM,
        LEAF_CELL_ZOOM,
        [4, 90],
        "on the seam, the leaf bricks either side of it handed between the frames",
    );
}

/// Runs a frame's walks from a box `by` leaf cells off `eye`, over a world whose clip level at
/// `cell_zoom` lacks its bricks. Every brick of that level is then handed in, and the next
/// frame, after the app forgets by the entries that moved, must answer as the world that holds
/// them.
fn handed_between_frames(eye: [i32; 2], cell_zoom: Zoom, by: [i32; 2], name: &'static str) {
    let Some(gpu) = Gpu::new() else {
        eprintln!("no adapter: skipped");
        return;
    };
    let built = real_world::build(eye);
    // Before the change, the clip level's entries are wanted and not yet filled, and the
    // clipmap is folded again over them.
    let mut was = built.clipmap.clone();
    let level = ((cell_zoom - FIRST_CELL_ZOOM) * ENTRIES) as usize;
    let rows = level..level + ENTRIES as usize;
    if eye == EYE_SEAM {
        let x = |west: bool| {
            was[rows.clone()]
                .iter()
                .any(|r| (r[0] < 0) == west && r[2] != 0)
        };
        assert!(
            x(true) && x(false),
            "the level's bricks held lie on one side of the seam"
        );
    }
    for row in &mut was[rows] {
        row[2..].fill(0);
    }
    real_world::fold(
        &mut was,
        &built.ground,
        &built.palette,
        &built.shards,
        &built.pool,
    );
    let now = &built.clipmap;
    // The host marks an eye's entry as moved where its brick changed, and joins each level's
    // changed bricks (gpu/ray/cast.rs).
    let mut changed = [0u32; CHANGED_WORDS as usize];
    let mut changed_bricks = [node_cache::NO_BRICKS; clipmap::EYE_CLIP_LEVELS as usize];
    for k in 0..(clipmap::EYE_CLIP_LEVELS * ENTRIES) as usize {
        if Entry::of(now[k]).changed_from(Entry::of(was[k])) {
            changed[k / 32] |= 1 << (k % 32);
        }
        if Entry::of(now[k]).present_changed_from(Entry::of(was[k])) {
            for q in [Entry::of(now[k]).brick, Entry::of(was[k]).brick] {
                if q != [i32::MIN; 2] {
                    let level = &mut changed_bricks[k / ENTRIES as usize];
                    *level = node_cache::bricks_joined(*level, [q[0], q[1], q[0], q[1]]);
                }
            }
        }
    }
    assert!(changed.iter().any(|&m| m != 0), "no entry changed");

    let store = wgpu::BufferUsages::STORAGE;
    let bufs = |rows: &[[i32; 4]]| WorldBuffer {
        clip: gpu.buf(
            &rows
                .iter()
                .flatten()
                .map(|&v| v as u32)
                .collect::<Vec<u32>>(),
            store,
        ),
        ground: gpu.buf(&built.ground, store),
        palette: gpu.buf(&built.palette, store),
        shards: gpu.buf(
            &built.shards.iter().flatten().copied().collect::<Vec<u32>>(),
            store,
        ),
        placed: gpu.buf(&built.objects, store),
        pool: gpu.buf(&built.pool, store),
    };
    let (before, after) = (bufs(&was), bufs(now));
    let forget = gpu.pipeline(src(SHADER_FORGET), "forget_cs");
    let over = |g: [i32; 2], up: i32| q8([g[0], g[1], built.top_at(g) + up]);
    let case = Case {
        name,
        origin: over([eye[0] + by[0], eye[1] + by[1]], 40),
        dir: [0.3, 0.2, -0.93],
        side: 1.0,
        back: 60.0,
        t_max: 400.0,
        range: [8, FINEST],
        corner: [1300, -900],
        cuts: [Cut::NONE; 2],
        least: &[("hit", 19850), ("slots", 19850)],
    };
    let edited = Edited {
        b: &after,
        w: built.world(),
        changed,
        changed_bricks,
        forget: &forget,
    };
    let mut failed = Vec::new();
    run_case(
        &gpu,
        &before,
        World {
            clipmap: &was,
            ..built.world()
        },
        &case,
        Some(&edited),
        &mut failed,
    );
    assert!(failed.is_empty(), "{}", failed.join("\n"));
}

/// Casts a map fresh over the world `b` with the eye's cuts `cuts` and every tile read, as
/// `run_case`'s first frame does. Returns the slots listed and the map's words.
fn fresh_map(gpu: &Gpu, b: &WorldBuffer, case: &Case, cuts: [Cut; 2]) -> (Vec<bool>, Vec<u32>) {
    let (s, starts) = box_of(case);
    let mut eye_words = vec![0u32; eye::WORDS];
    eye_words[eye::LIGHT..eye::LIGHT + eye::LIGHT_WORDS].copy_from_slice(&s.words());
    eye_words[eye::RANGE] = case.range[0];
    eye_words[eye::RANGE + 1] = case.range[1];
    for (at, cut) in [(eye::CARVE, cuts[0]), (eye::HIDE, cuts[1])] {
        eye_words[at] = cut.at[0] as u32;
        eye_words[at + 1] = cut.at[1] as u32;
        eye_words[at + 2] = cut.at[2] as u32;
        eye_words[at + 3] = cut.zoom;
    }
    let mut header = vec![0u32; light::HEADER_WORDS];
    header[light::HEADER_CAST] = CAST;
    header[light::HEADER_EPOCH] = CAST;
    for (k, v) in light::header_layout(SIZE) {
        header[k] = v;
    }
    header[light::STARTS_AT * 4..(light::STARTS_AT + light::STARTS_VEC4S) * 4]
        .copy_from_slice(&starts.words());
    let eye_buf = gpu.buf(&eye_words, wgpu::BufferUsages::UNIFORM);
    let header_buf = gpu.buf(&header, wgpu::BufferUsages::UNIFORM);
    let n = SIZE * SIZE;
    let walked: Vec<bool> = (0..n)
        .map(|k| k < n - NOT_LISTED && !k.is_multiple_of(SKIPPED_EVERY))
        .collect();
    let slots: Vec<u32> = (0..n)
        .filter(|&k| walked[k as usize])
        .map(|k| (k % SIZE) | ((k / SIZE) << 16))
        .collect();
    let mut list = list_of(&slots);
    lay_cells(gpu, b, [&eye_buf, &header_buf], &mut list);
    let mut table = vec![0u32; node_cache::WORDS as usize];
    table[node_cache::FRAME as usize] = FRAME;
    let map = map_of(gpu);
    let view = array_view(&map);
    let table_buf = gpu.buf(&table, store_of());
    let list_buf = gpu.buf(&list, store_of());
    holes(
        gpu,
        b,
        [&eye_buf, &header_buf, &table_buf, &list_buf],
        &view,
        &walked,
    );
    (walked, read_texture(&gpu.device, &gpu.queue, &map))
}

/// A change to the world must keep the tiles it does not reach (pxl-engine `Light::reaches`).
/// The changes are bricks of the leaf clip level handed in, a brick over the leaf's handed in,
/// and a preview's cut moved. In each, every slot of a tile that no changed column reaches must
/// hold the same word in a map cast fresh after the change as in one cast fresh before it, by
/// the GPU and by the engine's walk alike. The change must also move slots of the tiles it
/// reaches.
#[test]
fn a_world_change_keeps_the_tiles_it_does_not_reach() {
    let Some(gpu) = Gpu::new() else {
        eprintln!("no adapter: skipped");
        return;
    };
    let built = real_world::build(EYE);
    let store = wgpu::BufferUsages::STORAGE;
    let bufs = |rows: &[[i32; 4]]| WorldBuffer {
        clip: gpu.buf(
            &rows
                .iter()
                .flatten()
                .map(|&v| v as u32)
                .collect::<Vec<u32>>(),
            store,
        ),
        ground: gpu.buf(&built.ground, store),
        palette: gpu.buf(&built.palette, store),
        shards: gpu.buf(
            &built.shards.iter().flatten().copied().collect::<Vec<u32>>(),
            store,
        ),
        placed: gpu.buf(&built.objects, store),
        pool: gpu.buf(&built.pool, store),
    };
    let over = |g: [i32; 2], up: i32| q8([g[0], g[1], built.top_at(g) + up]);
    let now = &built.clipmap;
    let after = bufs(now);

    // Two of the leaf clip level's bricks under the box are wanted and not yet filled before
    // the change, and held after it. Their columns are taken as the host takes them
    // (gpu/ray/cast.rs).
    let g = [EYE[0] + 90, EYE[1] + 90];
    let bricks_case = Case {
        name: "bricks of the leaf clip level handed",
        origin: over(g, 40),
        dir: [0.3, 0.2, -0.93],
        side: 1.0,
        back: 60.0,
        t_max: 400.0,
        range: [8, FINEST],
        corner: [1300, -900],
        cuts: [Cut::NONE; 2],
        least: &[],
    };
    let leaf = ((LEAF_CELL_ZOOM - FIRST_CELL_ZOOM) * ENTRIES) as usize;
    let s = clipmap::BRICK_SHIFT;
    let mut was = now.clone();
    let mut brick_columns = Vec::new();
    for q in [[g[0] >> s, g[1] >> s], [(g[0] >> s) + 2, g[1] >> s]] {
        let row = &mut was[leaf + clipmap::entry(q) as usize];
        assert!(
            Entry::of(*row).holds(q),
            "the leaf clip level holds the brick"
        );
        row[2..].fill(0);
        light::brick_columns(
            bricks_case.origin,
            LEAF_CELL_ZOOM,
            q,
            |_| false,
            |c| brick_columns.push(c),
        );
    }
    real_world::fold(
        &mut was,
        &built.ground,
        &built.palette,
        &built.shards,
        &built.pool,
    );
    let before = bufs(&was);

    // A brick of a clip level over the leaf's is handed in. A band finer than that level reads
    // it only where the leaf level's bricks are not present.
    let over_case = Case {
        name: "a brick over the leaf's handed",
        ..bricks_case
    };
    let row_of = |cell_zoom: Zoom, q: [i32; 2]| {
        ((cell_zoom - FIRST_CELL_ZOOM) * ENTRIES) as usize + clipmap::entry(q) as usize
    };
    let (coarser, q) = (FIRST_CELL_ZOOM..LEAF_CELL_ZOOM)
        .rev()
        .map(|cell_zoom| (cell_zoom, g.map(|v| v >> (s + LEAF_CELL_ZOOM - cell_zoom))))
        .find(|&(cell_zoom, q)| Entry::of(now[row_of(cell_zoom, q)]).holds(q))
        .expect("a clip level over the leaf's holds a brick");
    eprintln!("the brick over the leaf's at {coarser}, the leaf's at {LEAF_CELL_ZOOM}");
    let mut was_over = now.clone();
    was_over[row_of(coarser, q)][2..].fill(0);
    let mut over_columns = Vec::new();
    light::brick_columns(
        over_case.origin,
        coarser,
        q,
        |c| Entry::of(now[row_of(coarser + 1, c)]).holds(c),
        |c| over_columns.push(c),
    );
    real_world::fold(
        &mut was_over,
        &built.ground,
        &built.palette,
        &built.shards,
        &built.pool,
    );
    let before_over = bufs(&was_over);

    // A preview's cut moves: there is none before, and one over what is placed after.
    let (_, leaf_c) = built
        .placed
        .iter()
        .copied()
        .find(|p| p.0 == 25)
        .expect("something placed finer than a cell");
    let cut = Cut {
        zoom: 20,
        at: [leaf_c[0] >> 5, leaf_c[1] >> 5, leaf_c[2] >> 5],
    };
    let cut_case = Case {
        name: "a preview's cut moved",
        origin: over([leaf_c[0] >> 1, leaf_c[1] >> 1], 30),
        dir: [0.1, 0.25, -0.96],
        side: 0.25,
        back: 10.0,
        t_max: 120.0,
        range: [LEAF_CELL_ZOOM, FINEST],
        corner: [8, 16],
        cuts: [Cut::NONE; 2],
        least: &[],
    };
    let b = object::cut_rect(cut.zoom, cut.at);
    let cut_columns = vec![light::Column::every_band(light::column_of(
        cut_case.origin,
        LEAF_CELL_ZOOM,
        [b[0], b[1]],
        [b[2], b[3]],
    ))];

    let mut failed = Vec::new();
    let changes = [
        (
            &bricks_case,
            [&before, &after],
            [
                World {
                    clipmap: &was,
                    ..built.world()
                },
                built.world(),
            ],
            [[Cut::NONE; 2]; 2],
            brick_columns,
        ),
        (
            &over_case,
            [&before_over, &after],
            [
                World {
                    clipmap: &was_over,
                    ..built.world()
                },
                built.world(),
            ],
            [[Cut::NONE; 2]; 2],
            over_columns,
        ),
        (
            &cut_case,
            [&after, &after],
            [built.world(), built.world()],
            [[Cut::NONE; 2], [cut, Cut::NONE]],
            cut_columns,
        ),
    ];
    for (case, [b0, b1], [w0, w1], [c0, c1], columns) in changes {
        let (s, starts) = box_of(case);
        let (walked, got0) = fresh_map(&gpu, b0, case, c0);
        let (_, got1) = fresh_map(&gpu, b1, case, c1);
        let (mut q0, mut q1) = (Asking { w: w0, cuts: c0 }, Asking { w: w1, cuts: c1 });
        let (mut kept, mut reached, mut changed, mut differ, mut walks_differ) =
            (0u32, 0u32, 0u32, 0u32, 0u32);
        for k in 0..(SIZE * SIZE) as usize {
            if !walked[k] {
                continue;
            }
            let slot = [k as u32 % SIZE, k as u32 / SIZE];
            let p = s.texel(SIZE, slot);
            let tile = [p[0] / light::TILE, p[1] / light::TILE];
            let (o, r, d) = slot_ray(case, &s, &starts, slot);
            let want0 = light::word(light::depth(
                &s,
                &cast::first(&mut q0, o, &r, d, Until::Opaque),
            ));
            let want1 = light::word(light::depth(
                &s,
                &cast::first(&mut q1, o, &r, d, Until::Opaque),
            ));
            if columns
                .iter()
                .any(|&c| s.reaches(SIZE, case.range, tile, c))
            {
                reached += 1;
                changed += u32::from(want0 != want1);
                continue;
            }
            kept += 1;
            walks_differ += u32::from(want0 != want1);
            if got0[k].abs_diff(got1[k]) > WITHIN && !an_axis_over(&s, got0[k], got1[k]) {
                differ += 1;
            }
        }
        eprintln!(
            "{}: {kept} slots kept, {reached} reached, {changed} of them changed; kept but changed: {walks_differ} walked, {differ} cast",
            case.name
        );
        if walks_differ != 0 || differ != 0 {
            failed.push(format!(
                "{}: slots of tiles kept changed, {walks_differ} by the engine's walk, {differ} by the GPU's",
                case.name
            ));
        }
        if kept < 1000 || changed < 100 {
            failed.push(format!(
                "{}: too few to say: {kept} kept, {changed} changed",
                case.name
            ));
        }
    }
    assert!(failed.is_empty(), "{}", failed.join("\n"));
}
