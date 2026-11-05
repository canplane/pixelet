//! `light_miss_cs` by hand (frame/shader/light_miss.wgsl) answers as `cast::walk`: over the
//! real world of tests/real_world, each listed slot's word as `light::word(light::depth(..))` of
//! its walk, and no other slot written; and the next frame, `light_miss_hit_cs` over the node
//! cache it left, listing what it leaves (`light::misses_left_at`), and `light_miss_cs` again over
//! that list, each slot's word as the world's again; and where
//! bricks changed between the frames, the next frame's after the app's forgetting (`forget_cs`)
//! as the changed world's. Every walk resumes off its cell's line, the cells laid as the cast
//! lays them each frame (beam_light.wgsl, light_cell.wgsl): the next frame's where the walk over
//! the node cache left it, which its slot's word holds; the first frame's at 0, the map's words
//! laid none. Skipped where no adapter answers.

mod gpu;
mod real_world;

use std::collections::BTreeMap;

use gpu::{array_view, read_texture, texture, Gpu};

use pxl_engine::answer;
use pxl_engine::cast::{self, Detail, End, Ray, Until};
use pxl_engine::clipmap::{self, Entry, ENTRIES, FIRST_CELL};
use pxl_engine::light::{self, Light, Starts};
use pxl_engine::node_cache::{self, MOVED_WORDS};
use pxl_engine::overwrite::Cut;
use pxl_engine::{beam, eye, overwrite, ray, value, Zoom, FRAC_W, LEAF_CELL};
use pxl_render::frame::shader::record::{
    src, SHADER_FORGET, SHADER_LIGHT_CELL, SHADER_LIGHT_MISS, SHADER_LIGHT_MISS_HIT,
};
use pxl_render::frame::shader::word::ROW_LANES;

use real_world::{q8, store_of, truth, Asking, Built, World, WorldBufs, FINEST, NOTCH};

/// Where the eye is, a leaf cell: east of X = 0, by the seam on the far side, a few cells over
/// the service area's south edge.
const EYE: [i32; 2] = [(1 << 23) - 300, (40 << 16) + 30];
/// One west of X = 0, a mile from the first over the seam.
const EYE_WEST: [i32; 2] = [-(1 << 23) + 300, (40 << 16) + 30];
/// By X = 0, where X's roots part.
const EYE_ZERO: [i32; 2] = [150, (40 << 16) + 30];

const SIZE: u32 = 256;
/// A word a GPU's rounding may move, as tests/walk.rs takes it: its depth's product fused into
/// the push after it, and its product with `light::DEPTH_ONE` into the half after it, a word's
/// step each. Nothing is divided: the light's reciprocals and starts are the host's.
const WITHIN: u32 = 2;
const FRAME: u32 = 9;
/// Slots told of a case that differs, its table a copy each; those past some times as many
/// are read as they are, a wrong shader differing nearly everywhere.
const TELLS: u32 = 4;

/// A light box and where its rays start, as a host lays them for the eye (light.rs `Starts`).
fn box_of(c: &Case) -> (Light, Starts) {
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
    let starts = Starts {
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
    /// Where the box's middle lies, in the steps a ray starts on.
    origin: [i32; 3],
    dir: [f32; 3],
    /// A texel's side, in units, and how far back along the light its near plane lies.
    side: f32,
    back: f32,
    t_max: f32,
    range: [Zoom; 2],
    corner: [i32; 2],
    cuts: [Cut; 2],
    /// Least of each kind of thing this seeded case meets.
    least: &'static [(&'static str, u32)],
}

/// What a slot's walk and the table left of the node that decides it.
fn slot_ray(c: &Case, s: &Light, starts: &Starts, slot: [u32; 2]) -> ([i32; 3], Ray, Detail) {
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
    // A way's middle where the world answers with a way at the leaves, of the lines `lines`.
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
            range: [LEAF_CELL, FINEST],
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
            range: [LEAF_CELL, FINEST],
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
            range: [LEAF_CELL - 2, FINEST],
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
            // On the planes between its cells, X and Y a whole unit, which holds a ray to the
            // cell on its upper side.
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
            range: [LEAF_CELL - 2, FINEST],
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
fn hand_light_miss_answers_as_light_miss_cs() {
    let Some(gpu) = Gpu::new() else {
        eprintln!("no adapter: skipped");
        return;
    };

    let built = real_world::build(EYE);
    let w = built.world();
    let store = wgpu::BufferUsages::STORAGE;
    let flat_u = |rows: &[[u32; 4]]| rows.iter().flatten().copied().collect::<Vec<u32>>();
    let clip_flat: Vec<u32> = built.clipmap.iter().flatten().map(|&v| v as u32).collect();
    let bound = WorldBufs {
        clip: gpu.buffer(&clip_flat, store),
        ground: gpu.buffer(&built.ground, store),
        palette: gpu.buffer(&built.palette, store),
        shards: gpu.buffer(&flat_u(&built.shards), store),
        placed: gpu.buffer(&built.overwrites, store),
        pool: gpu.buffer(&built.pool, store),
    };
    let miss = gpu.pipeline(src(SHADER_LIGHT_MISS), "light_miss_cs");
    let hit = gpu.pipeline(src(SHADER_LIGHT_MISS_HIT), "light_miss_hit_cs");

    let mut failed = Vec::new();
    for case in cases(&built) {
        run_case(&gpu, &bound, &miss, &hit, w, &case, None, &mut failed);
    }
    // West of X = 0, every node's X negative as the world names it: kept, found and read.
    let west = real_world::build(EYE_WEST);
    let bound_west = WorldBufs {
        clip: gpu.buffer(
            &west
                .clipmap
                .iter()
                .flatten()
                .map(|&v| v as u32)
                .collect::<Vec<u32>>(),
            store,
        ),
        ground: gpu.buffer(&west.ground, store),
        palette: gpu.buffer(&west.palette, store),
        shards: gpu.buffer(&flat_u(&west.shards), store),
        placed: gpu.buffer(&west.overwrites, store),
        pool: gpu.buffer(&west.pool, store),
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
    run_case(
        &gpu,
        &bound_west,
        &miss,
        &hit,
        west.world(),
        &case,
        None,
        &mut failed,
    );
    // Over X = 0, where X's roots part: rays end in the next root.
    let zero = real_world::build(EYE_ZERO);
    let bound_zero = WorldBufs {
        clip: gpu.buffer(
            &zero
                .clipmap
                .iter()
                .flatten()
                .map(|&v| v as u32)
                .collect::<Vec<u32>>(),
            store,
        ),
        ground: gpu.buffer(&zero.ground, store),
        palette: gpu.buffer(&zero.palette, store),
        shards: gpu.buffer(&flat_u(&zero.shards), store),
        placed: gpu.buffer(&zero.overwrites, store),
        pool: gpu.buffer(&zero.pool, store),
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
    run_case(
        &gpu,
        &bound_zero,
        &miss,
        &hit,
        zero.world(),
        &case,
        None,
        &mut failed,
    );
    assert!(failed.is_empty(), "{}", failed.join("\n"));
}

/// The slots listed: all but the last few and one in so many, the rest walked by none.
const NOT_LISTED: u32 = 30;
const SKIPPED_EVERY: u32 = 11;
/// The cast the head names, its epoch begun with it: every tile read.
const CAST: u32 = 7;

/// The beam's lines and each cell's start laid in `list` as the cast lays them (beam_light.wgsl,
/// light_cell.wgsl), every tile read, its beam over a table of its own.
fn lay_cells(gpu: &Gpu, b: &WorldBufs, [eye, head]: [&wgpu::Buffer; 2], list: &mut [u32]) {
    let tiles = light::tiles(SIZE) as usize;
    let reads = light::reads_at(SIZE) as usize;
    list[reads..reads + tiles * tiles].fill(CAST);
    let squares = (light::coarse(SIZE) * light::coarse(SIZE)) as usize;
    let coarse = light::coarse_at(SIZE) as usize;
    list[coarse..coarse + squares].fill(CAST);
    let mut table = vec![0u32; node_cache::WORDS as usize];
    table[node_cache::FRAME as usize] = FRAME;
    let table = gpu.buffer(&table, store_of());
    let buf = gpu.buffer(list, store_of());
    let side = beam::CORNER_SIDE;
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
            (11, head.as_entire_binding()),
        ],
        &buf,
        SIZE,
    );
    let cells = light::cells_across(SIZE);
    gpu.dispatch(
        &gpu.pipeline(src(SHADER_LIGHT_CELL), "light_cell_cs"),
        &[
            (0, eye.as_entire_binding()),
            (8, buf.as_entire_binding()),
            (11, head.as_entire_binding()),
        ],
        [cells.div_ceil(side), cells.div_ceil(side), 1],
    );
    // The beam's lines and hands and the cells' starts, all before the samples.
    let laid = light::misses_cnt(SIZE) as usize;
    list[..laid].copy_from_slice(&gpu.read(&buf)[..laid]);
}

/// The world the next frame casts over, its bricks changed since the first, as the app hands
/// it: its buffers, what the moved entries' words say (gpu/ray/cast.rs), and the pass that
/// forgets by them.
struct Edited<'a> {
    b: &'a WorldBufs,
    w: World<'a>,
    moved: [u32; MOVED_WORDS as usize],
    forget: &'a wgpu::ComputePipeline,
}

#[expect(
    clippy::too_many_arguments,
    reason = "a case's passes and worlds, each its own"
)]
fn run_case(
    gpu: &Gpu,
    b: &WorldBufs,
    miss: &wgpu::ComputePipeline,
    hit: &wgpu::ComputePipeline,
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
    let mut head = vec![0u32; light::HEAD_WORDS];
    head[light::HEAD_CAST] = CAST;
    head[light::HEAD_EPOCH] = CAST;
    for (k, v) in light::head_layout(SIZE) {
        head[k] = v;
    }
    head[light::STARTS_AT * 4..(light::STARTS_AT + light::STARTS_VEC4S) * 4]
        .copy_from_slice(&starts.words());
    let eye_buf = gpu.buffer(&eye_words, wgpu::BufferUsages::UNIFORM);
    let head_buf = gpu.buffer(&head, wgpu::BufferUsages::UNIFORM);

    // The slots listed, the box's rows wrapped: in the misses' list, which the next frame's walk
    // over the node cache reads, and in the list of what it left, which `light_miss_cs` reads.
    let n = SIZE * SIZE;
    let walked: Vec<bool> = (0..n)
        .map(|k| k < n - NOT_LISTED && !k.is_multiple_of(SKIPPED_EVERY))
        .collect();
    let slots: Vec<u32> = (0..n)
        .filter(|&k| walked[k as usize])
        .map(|k| (k % SIZE) | ((k / SIZE) << 16))
        .collect();
    let mut list = vec![0u32; light::list_words(SIZE) as usize];
    for at in [light::misses_cnt(SIZE), light::misses_left_at(SIZE)] {
        let at = at as usize;
        list[at] = slots.len() as u32;
        list[at + 1..at + 1 + slots.len()].copy_from_slice(&slots);
    }
    let left_at = light::misses_left_at(SIZE) as usize;

    let mut table0 = vec![0u32; node_cache::WORDS as usize];
    table0[node_cache::FRAME as usize] = FRAME;
    // The reference: a slot at a time, every node asked of the world.
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
            add("hit", t.ends == End::Hit);
            add("past its reach", t.ends == End::Miss);
            add("lacks", t.ends == End::Lacks);
            let a = t.hit.asset[0];
            let hits = t.ends == End::Hit;
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

    // Each cell's start, off the beam the cast walks first, which every slot's walk resumes from.
    lay_cells(gpu, b, [&eye_buf, &head_buf], &mut list);

    // The hand pass, over the same table.
    let map = map_of(gpu);
    let view = array_view(&map);
    let table_buf = gpu.buffer(&table0, store_of());
    let list_buf = gpu.buffer(&list, store_of());
    let rows = (slots.len() as u32).div_ceil(ray::MISS_LANES * ray::MISS_ACROSS);
    gpu.dispatch(
        miss,
        &miss_binds(b, [&eye_buf, &head_buf, &table_buf, &list_buf], &view),
        [ray::MISS_ACROSS, rows, 1],
    );
    let got = read_texture(&gpu.device, &gpu.queue, &map);
    let table_hand = gpu.read(&table_buf);

    // Each slot's word, as `hand_light_walk_answers_as_cast_walk` takes it.
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

    // The next frame: the walk over the table alone first, then this pass for what it left.
    let mut next = table_hand.clone();
    next[node_cache::FRAME as usize] = FRAME + 1;
    if let Some(e) = edited {
        next[node_cache::FORGETS as usize] = node_cache::FORGETS_MOVED;
        let m = node_cache::MOVED_AT as usize;
        next[m..m + MOVED_WORDS as usize].copy_from_slice(&e.moved);
    }
    let next_buf = gpu.buffer(&next, store_of());
    if let Some(e) = edited {
        gpu.dispatch(
            e.forget,
            &[(9, next_buf.as_entire_binding())],
            [node_cache::ENTRIES.div_ceil(ROW_LANES), 1, 1],
        );
    }
    // What the next frame must answer: the changed world's, where it changed.
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
    lay_cells(gpu, b, [&eye_buf, &head_buf], &mut list);
    let list2 = gpu.buffer(&list, store_of());
    let left = gpu.buffer(&[ray::MISS_ACROSS, 0, 1, 0], store_of());
    let map2 = map_of(gpu);
    let view2 = array_view(&map2);
    gpu.dispatch(
        hit,
        &[
            (0, eye_buf.as_entire_binding()),
            (8, list2.as_entire_binding()),
            (9, next_buf.as_entire_binding()),
            (10, wgpu::BindingResource::TextureView(&view2)),
            (11, head_buf.as_entire_binding()),
            (13, left.as_entire_binding()),
        ],
        [ray::MISS_ACROSS, rows, 1],
    );
    let after = gpu.read(&list2);
    let args = gpu.read(&left);
    // What it left, listed once each.
    let mut still = vec![false; n as usize];
    let mut twice = 0u32;
    for p in 0..args[3] as usize {
        let v = after[left_at + 1 + p];
        let k = ((v >> 16) * SIZE + (v & 0xffff)) as usize;
        twice += u32::from(still[k] || !walked[k]);
        still[k] = true;
    }
    let slots_left = args[3];
    let ended = slots.len() as u32 - slots_left;
    if twice != 0 {
        failed.push(format!(
            "{name}: {twice} slots left listed twice or not listed"
        ));
        return;
    }
    gpu.queue
        .write_buffer(&list2, left_at as u64 * 4, gpu::bytes(&[args[3]]));
    gpu.dispatch(
        miss,
        &miss_binds(b, [&eye_buf, &head_buf, &next_buf, &list2], &view2),
        [args[0], args[1], 1],
    );
    let got2 = read_texture(&gpu.device, &gpu.queue, &map2);
    // Each slot's word the next frame, ended by the table or walked again: the world's again.
    let mut next_differ = 0u32;
    let mut next_axis_over = 0u32;
    let mut first_next = None;
    for k in 0..n as usize {
        if walked[k] && got2[k].abs_diff(want[k]) > WITHIN {
            if an_axis_over(&s, got2[k], want[k]) {
                next_axis_over += 1;
            } else {
                next_differ += 1;
                first_next.get_or_insert((k, still[k], got2[k], want[k]));
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
    eprintln!(
        "  next frame: {ended} ended by the table, {slots_left} left, {next_axis_over} an axis over, {next_differ} differ"
    );
    let mut fail = |what: String| failed.push(format!("{name}: {what}"));
    if differ != 0 {
        fail(format!("{differ} slots differ, first {first_differ:?}"));
    }
    if axis_over > 256 {
        fail(format!("too many an axis over: {axis_over}"));
    }
    if next_differ != 0 {
        fail(format!(
            "{next_differ} differ the next frame, first slot, walked again, got, wanted {first_next:?}"
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

/// Whether `got` is `want` for a ray entering at an edge, the axis it enters across another
/// as a GPU rounds its planes: apart by the two axes' push back alone.
fn an_axis_over(s: &Light, got: u32, want: u32) -> bool {
    let apart = got.abs_diff(want);
    let one = light::DEPTH_ONE as f32;
    [(0, 1), (0, 2), (1, 2)].iter().any(|&(a, b)| {
        let by = (libm::fabsf(s.bias[a] - s.bias[b]) * one) as u32;
        apart.abs_diff(by) <= WITHIN
    })
}

/// A slot that differs said where it can be read: its words and where its walk stops.
fn tell_slot(w: World, case: &Case, s: &Light, starts: &Starts, k: u32, [got, want]: [u32; 2]) {
    let (o, r, d) = slot_ray(case, s, starts, [k % SIZE, k / SIZE]);
    let mut q = Asking { w, cuts: case.cuts };
    let t = cast::first(&mut q, o, &r, d, Until::Opaque);
    let (z, c) = (t.hit.node.zoom, t.hit.node.idx(o));
    eprintln!(
        "DIFFERS {} slot {k} got {got} want {want} band {:?}: its walk stops {:?} z{z} {c:?} t {} axis {} asset {:#x}",
        case.name, d.range, t.ends, t.hit.t, t.hit.axis, t.hit.asset[0]
    );
}

/// The map the passes write a word a texel of, `SIZE` a side, laid with zeros.
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

/// `light_miss_cs`'s group, over the eye, the head, the table and the list.
fn miss_binds<'a>(
    b: &'a WorldBufs,
    [eye, head, table, list]: [&'a wgpu::Buffer; 4],
    view: &'a wgpu::TextureView,
) -> Vec<(u32, wgpu::BindingResource<'a>)> {
    vec![
        (0, eye.as_entire_binding()),
        (1, b.clip.as_entire_binding()),
        (2, b.ground.as_entire_binding()),
        (4, b.palette.as_entire_binding()),
        (5, b.shards.as_entire_binding()),
        (6, b.placed.as_entire_binding()),
        (7, b.pool.as_entire_binding()),
        (9, table.as_entire_binding()),
        (10, wgpu::BindingResource::TextureView(view)),
        (11, head.as_entire_binding()),
        (8, list.as_entire_binding()),
    ]
}

/// The node cache never answers the world as it was: a frame's walks keep what a world whose
/// leaf clip level lacks its bricks answers, every brick of that level is then handed, and the
/// next frame, after the app forgets by the entries that moved, answers as the world that holds
/// them.
#[test]
fn an_edited_brick_is_never_answered_as_it_was() {
    let Some(gpu) = Gpu::new() else {
        eprintln!("no adapter: skipped");
        return;
    };
    let built = real_world::build(EYE);
    // Before: the leaf clip level's entries wanted and not yet filled, folded again.
    let mut was = built.clipmap.clone();
    let leaf = ((LEAF_CELL - FIRST_CELL) * ENTRIES) as usize;
    for row in &mut was[leaf..leaf + ENTRIES as usize] {
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
    // The host's rule (gpu/ray/cast.rs): an eye's entry moved where its brick did.
    let mut moved = [0u32; MOVED_WORDS as usize];
    for k in 0..(clipmap::EYE_CLIP_LEVELS * ENTRIES) as usize {
        if Entry::of(now[k]).moved_from(Entry::of(was[k])) {
            moved[k / 32] |= 1 << (k % 32);
        }
    }
    assert!(moved.iter().any(|&m| m != 0), "no entry moved");

    let store = wgpu::BufferUsages::STORAGE;
    let bufs = |rows: &[[i32; 4]]| WorldBufs {
        clip: gpu.buffer(
            &rows
                .iter()
                .flatten()
                .map(|&v| v as u32)
                .collect::<Vec<u32>>(),
            store,
        ),
        ground: gpu.buffer(&built.ground, store),
        palette: gpu.buffer(&built.palette, store),
        shards: gpu.buffer(
            &built.shards.iter().flatten().copied().collect::<Vec<u32>>(),
            store,
        ),
        placed: gpu.buffer(&built.overwrites, store),
        pool: gpu.buffer(&built.pool, store),
    };
    let (before, after) = (bufs(&was), bufs(now));
    let miss = gpu.pipeline(src(SHADER_LIGHT_MISS), "light_miss_cs");
    let hit = gpu.pipeline(src(SHADER_LIGHT_MISS_HIT), "light_miss_hit_cs");
    let forget = gpu.pipeline(src(SHADER_FORGET), "forget_cs");
    let over = |g: [i32; 2], up: i32| q8([g[0], g[1], built.top_at(g) + up]);
    let case = Case {
        name: "about the eye, its leaf bricks handed between the frames",
        origin: over([EYE[0] + 90, EYE[1] + 90], 40),
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
        moved,
        forget: &forget,
    };
    let mut failed = Vec::new();
    run_case(
        &gpu,
        &before,
        &miss,
        &hit,
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

/// Each slot's word in a map cast fresh over the world `b`, the eye's cuts `cuts`, every tile
/// read (`run_case`'s first frame): the slots listed, and `light_miss_cs`'s words.
fn fresh_map(
    gpu: &Gpu,
    b: &WorldBufs,
    miss: &wgpu::ComputePipeline,
    case: &Case,
    cuts: [Cut; 2],
) -> (Vec<bool>, Vec<u32>) {
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
    let mut head = vec![0u32; light::HEAD_WORDS];
    head[light::HEAD_CAST] = CAST;
    head[light::HEAD_EPOCH] = CAST;
    for (k, v) in light::head_layout(SIZE) {
        head[k] = v;
    }
    head[light::STARTS_AT * 4..(light::STARTS_AT + light::STARTS_VEC4S) * 4]
        .copy_from_slice(&starts.words());
    let eye_buf = gpu.buffer(&eye_words, wgpu::BufferUsages::UNIFORM);
    let head_buf = gpu.buffer(&head, wgpu::BufferUsages::UNIFORM);
    let n = SIZE * SIZE;
    let walked: Vec<bool> = (0..n)
        .map(|k| k < n - NOT_LISTED && !k.is_multiple_of(SKIPPED_EVERY))
        .collect();
    let slots: Vec<u32> = (0..n)
        .filter(|&k| walked[k as usize])
        .map(|k| (k % SIZE) | ((k / SIZE) << 16))
        .collect();
    let mut list = vec![0u32; light::list_words(SIZE) as usize];
    for at in [light::misses_cnt(SIZE), light::misses_left_at(SIZE)] {
        let at = at as usize;
        list[at] = slots.len() as u32;
        list[at + 1..at + 1 + slots.len()].copy_from_slice(&slots);
    }
    lay_cells(gpu, b, [&eye_buf, &head_buf], &mut list);
    let mut table = vec![0u32; node_cache::WORDS as usize];
    table[node_cache::FRAME as usize] = FRAME;
    let map = map_of(gpu);
    let view = array_view(&map);
    let table_buf = gpu.buffer(&table, store_of());
    let list_buf = gpu.buffer(&list, store_of());
    let rows = (slots.len() as u32).div_ceil(ray::MISS_LANES * ray::MISS_ACROSS);
    gpu.dispatch(
        miss,
        &miss_binds(b, [&eye_buf, &head_buf, &table_buf, &list_buf], &view),
        [ray::MISS_ACROSS, rows, 1],
    );
    (walked, read_texture(&gpu.device, &gpu.queue, &map))
}

/// A world's change keeps the tiles it does not reach (pxl-engine `Light::reaches`): over the
/// real world, where bricks of the leaf clip level were handed and where a preview's cut
/// moved, each slot of a tile no changed column reaches holds in a map cast fresh after the
/// change what one cast fresh before it holds, the GPU's word and the engine's walk alike;
/// and the change moves slots of the tiles it reaches.
#[test]
fn a_world_change_keeps_the_tiles_it_does_not_reach() {
    let Some(gpu) = Gpu::new() else {
        eprintln!("no adapter: skipped");
        return;
    };
    let built = real_world::build(EYE);
    let store = wgpu::BufferUsages::STORAGE;
    let bufs = |rows: &[[i32; 4]]| WorldBufs {
        clip: gpu.buffer(
            &rows
                .iter()
                .flatten()
                .map(|&v| v as u32)
                .collect::<Vec<u32>>(),
            store,
        ),
        ground: gpu.buffer(&built.ground, store),
        palette: gpu.buffer(&built.palette, store),
        shards: gpu.buffer(
            &built.shards.iter().flatten().copied().collect::<Vec<u32>>(),
            store,
        ),
        placed: gpu.buffer(&built.overwrites, store),
        pool: gpu.buffer(&built.pool, store),
    };
    let miss = gpu.pipeline(src(SHADER_LIGHT_MISS), "light_miss_cs");
    let over = |g: [i32; 2], up: i32| q8([g[0], g[1], built.top_at(g) + up]);
    let now = &built.clipmap;
    let after = bufs(now);

    // Bricks handed: the leaf clip level's bricks under part of the box, wanted and not yet
    // filled before, folded again; their columns as the host takes them (gpu/ray/cast.rs).
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
    let leaf = ((LEAF_CELL - FIRST_CELL) * ENTRIES) as usize;
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
        brick_columns.push(light::column_of(
            bricks_case.origin,
            LEAF_CELL,
            [q[0] << s, q[1] << s],
            [(q[0] + 1) << s, (q[1] + 1) << s],
        ));
    }
    real_world::fold(
        &mut was,
        &built.ground,
        &built.palette,
        &built.shards,
        &built.pool,
    );
    let before = bufs(&was);

    // A preview's cut moved: none before, one over what is placed after.
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
        range: [LEAF_CELL, FINEST],
        corner: [8, 16],
        cuts: [Cut::NONE; 2],
        least: &[],
    };
    let b = overwrite::cut_box(cut.zoom, cut.at);
    let cut_columns = vec![light::column_of(
        cut_case.origin,
        LEAF_CELL,
        [b[0], b[1]],
        [b[2], b[3]],
    )];

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
            &cut_case,
            [&after, &after],
            [built.world(), built.world()],
            [[Cut::NONE; 2], [cut, Cut::NONE]],
            cut_columns,
        ),
    ];
    for (case, [b0, b1], [w0, w1], [c0, c1], columns) in changes {
        let (s, starts) = box_of(case);
        let (walked, got0) = fresh_map(&gpu, b0, &miss, case, c0);
        let (_, got1) = fresh_map(&gpu, b1, &miss, case, c1);
        let (mut q0, mut q1) = (Asking { w: w0, cuts: c0 }, Asking { w: w1, cuts: c1 });
        let (mut kept, mut reached, mut moved, mut differ, mut walks_differ) =
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
                moved += u32::from(want0 != want1);
                continue;
            }
            kept += 1;
            walks_differ += u32::from(want0 != want1);
            if got0[k].abs_diff(got1[k]) > WITHIN && !an_axis_over(&s, got0[k], got1[k]) {
                differ += 1;
            }
        }
        eprintln!(
            "{}: {kept} slots kept, {reached} reached, {moved} of them moved; kept but moved: {walks_differ} walked, {differ} cast",
            case.name
        );
        if walks_differ != 0 || differ != 0 {
            failed.push(format!(
                "{}: slots of tiles kept moved, {walks_differ} by the engine's walk, {differ} by the GPU's",
                case.name
            ));
        }
        if kept < 1000 || moved < 100 {
            failed.push(format!(
                "{}: too few to say: {kept} kept, {moved} moved",
                case.name
            ));
        }
    }
    assert!(failed.is_empty(), "{}", failed.join("\n"));
}
