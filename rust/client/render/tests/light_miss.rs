//! `light_miss_cs` by hand (frame/shader/light_miss.wgsl) answers as `cast::walk` writing the
//! observed pyramid (tests/keeping `Writing`): over the real world of tests/real_world, each slot's word as
//! `light::word(light::depth(..))` of its walk, and what it leaves in the observed pyramid
//! as `Writing` leaves it (`Keeping` here, its rule over the same words), kept for the walk
//! to find next frame. The walk's from the root, as `light_miss_hit_cs`'s is: a walk resumed
//! from its cell's start is not asked of it. Skipped where no adapter answers.

mod keeping;
mod real_world;

use std::collections::{BTreeMap, BTreeSet};

use keeping::*;

use pxl_engine::answer::{self, In, Kind};
use pxl_engine::cast::{self, Detail, End, Ray, Spacing, Until};
use pxl_engine::light::{self, Light, Starts};
use pxl_engine::overwrite::Cut;
use pxl_engine::pyramid::{self, BUCKET};
use pxl_engine::{eye, ray, tally, value, Zoom, FRAC_W, LEAF_CELL};
use pxl_render::frame::shader::record::{
    src, SHADER_FORGET, SHADER_LIGHT_MISS, SHADER_LIGHT_MISS_HIT,
};

use real_world::{Built, World, FINEST, NOTCH};

/// Where the eye is, a leaf cell: east of X = 0, by the seam on the far side, a few cells over
/// the service area's south edge.
const EYE: [i32; 2] = [(1 << 23) - 300, (40 << 16) + 30];
/// One west of X = 0, a mile from the first over the seam.
const EYE_WEST: [i32; 2] = [-(1 << 23) + 300, (40 << 16) + 30];
/// By X = 0, where X's roots part.
const EYE_ZERO: [i32; 2] = [150, (40 << 16) + 30];

const SIZE: u32 = 256;
/// A word a GPU's rounding may move, as tests/walk.rs takes it.
const WITHIN: u32 = 1024;
const FRAME: u32 = 9;
/// Slots told of a case that differs, its table a copy each; those past some times as many
/// are read as they are, a wrong shader differing nearly everywhere.
const TELLS: u32 = 4;
/// Rays that visit a node for it to be no grazing's.
const SURE: u32 = 9;

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
        across: across.map(|v| v * q),
        down: down.map(|v| v * q),
        at: [a0.0, a1.0, a2.0, a3.0],
        frac: [a0.1, a1.1, a2.1, a3.1],
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
    /// Whether the eye's rays walked this frame first, as a frame's do, keeping what they
    /// asked at the eye's zooms.
    eye_first: bool,
    /// Least of each kind of thing this seeded case meets, a third of what it met: which walk
    /// reads what another keeps moves some.
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
            eye_first: false,
            least: &[
                ("found again", 10057),
                ("hit", 19850),
                ("hit, a way's", 1161),
                ("hit, placed", 13329),
                ("hit, short of its cube", 364),
                ("kept, filled", 14301),
                ("next frame, ended by the table", 17942),
                ("slots", 19850),
            ],
        },
        Case {
            name: "about the eye, after the eye's own walk",
            origin: over([EYE[0] + 90, EYE[1] + 90], 40),
            dir: [0.3, 0.2, -0.93],
            side: 1.0,
            back: 60.0,
            t_max: 400.0,
            range: [8, FINEST],
            corner: [1300, -900],
            cuts: none,
            eye_first: true,
            least: &[
                ("found again", 10059),
                ("hit", 19850),
                ("hit, a way's", 1161),
                ("hit, placed", 13329),
                ("hit, short of its cube", 364),
                ("kept, filled", 14299),
                ("next frame, ended by the table", 17812),
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
            eye_first: false,
            least: &[
                ("hit", 19850),
                ("hit, a way's", 1033),
                ("hit, short of its cube", 7909),
                ("hit, the wall's", 2894),
                ("kept, filled", 1729),
                ("next frame, ended by the table", 2431),
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
            eye_first: false,
            least: &[
                ("found again", 776),
                ("hit", 19850),
                ("hit, a way's", 9),
                ("hit, placed", 4970),
                ("kept, filled", 2416),
                ("next frame, ended by the table", 16183),
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
            eye_first: false,
            least: &[
                ("found again", 6207),
                ("hit", 18559),
                ("hit, a way's", 210),
                ("hit, placed", 13302),
                ("kept, filled", 9107),
                ("next frame, ended by the table", 18481),
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
            eye_first: false,
            least: &[
                ("found again", 5695),
                ("hit", 18988),
                ("hit, placed", 9572),
                ("hit, the wall's", 2567),
                ("kept, filled", 11123),
                ("next frame, ended by the table", 18166),
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
            eye_first: false,
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
            eye_first: false,
            least: &[
                ("hit", 5380),
                ("kept, filled", 934),
                ("next frame, ended by the table", 4971),
                ("past its reach", 14469),
                ("slots", 19850),
            ],
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
            eye_first: false,
            least: &[
                ("found again", 10800),
                ("hit", 15437),
                ("hit, a way's", 260),
                ("hit, placed", 11960),
                ("hit, short of its cube", 316),
                ("kept, filled", 13301),
                ("next frame, ended by the table", 18544),
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
            eye_first: false,
            least: &[
                ("found again", 2089),
                ("hit", 19457),
                ("hit, a way's", 129),
                ("hit, placed", 4101),
                ("kept, filled", 10077),
                ("next frame, ended by the table", 17970),
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
            eye_first: false,
            least: &[
                ("found again", 11555),
                ("hit", 19612),
                ("hit, a way's", 675),
                ("hit, placed", 9507),
                ("hit, short of its cube", 343),
                ("hit, the wall's", 187),
                ("kept, filled", 18869),
                ("next frame, ended by the table", 17561),
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
            eye_first: false,
            least: &[
                ("lacks", 19850),
                ("next frame, ended by the table", 9470),
                ("slots", 19850),
            ],
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
            eye_first: false,
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
            eye_first: false,
            least: &[
                ("ended in another root", 16610),
                ("next frame, ended by the table", 3240),
                ("past its reach", 19850),
                ("slots", 19850),
            ],
        },
    ]
}

#[test]
fn hand_light_miss_answers_as_light_miss_cs() {
    let instance =
        wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    let Ok(adapter) = pollster::block_on(instance.request_adapter(&Default::default())) else {
        eprintln!("no adapter: skipped");
        return;
    };
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        required_limits: adapter.limits(),
        ..Default::default()
    }))
    .unwrap();
    let gpu = Gpu { device, queue };

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
        run_case(&gpu, &bound, &miss, &hit, w, &case, &mut failed);
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
        eye_first: false,
        least: &[
            ("hit", 19850),
            ("kept", 26000),
            ("kept, filled", 13000),
            ("next frame, ended by the table", 16000),
            ("slots", 19850),
        ],
    };
    run_case(
        &gpu,
        &bound_west,
        &miss,
        &hit,
        west.world(),
        &case,
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
        eye_first: false,
        least: &[("ended in another root", 1467), ("slots", 19850)],
    };
    run_case(
        &gpu,
        &bound_zero,
        &miss,
        &hit,
        zero.world(),
        &case,
        &mut failed,
    );
    assert!(failed.is_empty(), "{}", failed.join("\n"));
}

/// The first `cnt` of the list counted; the rest past it, and some struck, walked by none.
const NOT_COUNTED: u32 = 30;
const STRUCK_EVERY: u32 = 11;

fn run_case(
    gpu: &Gpu,
    b: &WorldBufs,
    miss: &wgpu::ComputePipeline,
    hit: &wgpu::ComputePipeline,
    w: World,
    case: &Case,
    failed: &mut Vec<String>,
) {
    let name = case.name;
    let (s, starts) = box_of(case);
    let tag = pyramid::tag(FRAME, pyramid::LIGHT_MISSED_PASS);

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
    head[light::HEAD_CAST] = 7;
    head[light::HEAD_SIZE] = SIZE;
    head[light::HEAD_MISSES] = light::misses_cnt(SIZE);
    head[light::STARTS_AT * 4..(light::STARTS_AT + light::STARTS_VEC4S) * 4]
        .copy_from_slice(&starts.words());
    let eye_buf = gpu.buffer(&eye_words, wgpu::BufferUsages::UNIFORM);
    let head_buf = gpu.buffer(&head, wgpu::BufferUsages::UNIFORM);

    // The list, as the map's list lays it: the slots in turn, the box's rows wrapped.
    let n = SIZE * SIZE;
    let cnt = n - NOT_COUNTED;
    let at = light::misses_at(SIZE) as usize;
    let mut list = vec![0u32; at + n as usize];
    list[light::misses_cnt(SIZE) as usize] = cnt;
    let mut walked = vec![false; n as usize];
    for y in 0..SIZE {
        for x in 0..SIZE {
            let k = y * SIZE + x;
            list[at + k as usize] = if k.is_multiple_of(STRUCK_EVERY) {
                light::MISS_STRUCK
            } else {
                x | y << 16
            };
            walked[k as usize] = !k.is_multiple_of(STRUCK_EVERY) && k < cnt;
        }
    }

    // What the walks meet, on a table of nothing: the nodes the traps are laid on.
    let mut empty = vec![0u32; pyramid::WORDS as usize];
    let mut met = BTreeMap::new();
    {
        let mut q = Keeping {
            w,
            cuts: case.cuts,
            table: &mut empty,
            tag,
            asks: Asks::default(),
            writes: true,
            met: Some(&mut met),
        };
        for k in 0..n {
            if walked[k as usize] {
                let (o, r, d) = slot_ray(case, &s, &starts, [k % SIZE, k / SIZE]);
                cast::first(&mut q, o, &r, d, Until::Opaque);
            }
        }
    }
    drop(empty);
    let mut table0 = vec![0u32; pyramid::WORDS as usize];
    table0[pyramid::FRAME as usize] = FRAME;
    table0[pyramid::TALLY_ON as usize] = 1;
    if case.eye_first {
        let mut q = Keeping {
            w,
            cuts: case.cuts,
            table: &mut table0,
            tag: pyramid::tag(FRAME, pyramid::MISSED_PASS),
            asks: Asks::default(),
            writes: true,
            met: None,
        };
        let detail = Detail {
            range: case.range,
            spacing: Spacing {
                angle: 0.002,
                across: 0.0,
            },
        };
        for j in 0..96 {
            for i in 0..96 {
                let tan = |v: i32| (v as f32 - 47.5) * 0.015;
                let d = cast::through(
                    [0.0, 0.0, -1.0],
                    [1.0, 0.0, 0.0],
                    [0.0, 1.0, 0.0],
                    tan(i),
                    tan(j),
                );
                let r = Ray::new(d, 2000.0);
                cast::first(&mut q, case.origin, &r, detail, Until::Opaque);
            }
        }
    }
    let traps = lay_traps(
        &mut table0,
        w,
        case.cuts,
        &met,
        [
            pyramid::tag(FRAME - 1, pyramid::LIGHT_SAMPLES_PASS),
            pyramid::tag(FRAME, pyramid::LIGHT_SAMPLES_PASS),
            tag,
        ],
    );

    // The reference: a slot at a time, over that table.
    let mut table_ref = table0.clone();
    let mut met_ref = BTreeMap::new();
    let mut want = vec![u32::MAX; n as usize];
    let mut kinds: BTreeMap<&'static str, u32> = BTreeMap::new();
    let mut most_visits = 0u32;
    let asks = {
        let mut q = Keeping {
            w,
            cuts: case.cuts,
            table: &mut table_ref,
            tag,
            asks: Asks::default(),
            writes: true,
            met: Some(&mut met_ref),
        };
        for k in 0..n {
            if !walked[k as usize] {
                continue;
            }
            let (o, r, d) = slot_ray(case, &s, &starts, [k % SIZE, k / SIZE]);
            let t = cast::first(&mut q, o, &r, d, Until::Opaque);
            most_visits = most_visits.max(t.visits);
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
        q.asks
    };

    // The hand pass, over the same table.
    let map = gpu.map(SIZE);
    let view = map.create_view(&wgpu::TextureViewDescriptor {
        dimension: Some(wgpu::TextureViewDimension::D2Array),
        ..Default::default()
    });
    let table_buf = gpu.buffer(&table0, store_of());
    let list_buf = gpu.buffer(&list, store_of());
    let rows = n.div_ceil(ray::MISS_LANES * ray::MISS_ACROSS);
    gpu.dispatch(
        miss,
        &miss_binds(b, [&eye_buf, &head_buf, &table_buf, &list_buf], &view),
        [ray::MISS_ACROSS, rows],
    );
    let got = gpu.read_map(&map);
    let table_hand = gpu.read(&table_buf);
    // A slot left claimed: the dispatch did not run to its end, and its words are no answer.
    let claimed_before = left_claimed(&table0).len();
    let claimed = left_claimed(&table_hand);
    if claimed.len() > claimed_before {
        failed.push(format!(
            "{name}: the pass did not run to its end: {} slots left claimed, first {:?}",
            claimed.len() - claimed_before,
            claimed.first()
        ));
        return;
    }

    // Each slot's word, as `hand_light_walk_answers_as_cast_walk` takes it.
    let mut read_only = table0.clone();
    let (mut same, mut stepped, mut axis_over, mut differ) = (0u32, 0u32, 0u32, 0u32);
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
        } else if stepped + differ <= TELLS * 8
            && a_step_over(
                w,
                case,
                &s,
                &starts,
                (&mut read_only, tag),
                k as u32,
                got[k],
            )
        {
            stepped += 1;
        } else {
            differ += 1;
            first_differ.get_or_insert((k, got[k], want[k]));
            if differ <= TELLS {
                tell_slot(
                    w,
                    case,
                    &s,
                    &starts,
                    k as u32,
                    [got[k], want[k]],
                    [&table0, &table_hand],
                );
            }
        }
    }

    // What it left, each slot it wrote: in its node's bucket, at this pass's tag, what the
    // world answers word for word, and what `Writing` wrote where that wrote it.
    let entry = |t: &[u32], e: u32| {
        let w = pyramid::entry_at(e) as usize;
        t[w..w + pyramid::ENTRY_WORDS as usize].to_vec()
    };
    let by_node = |t: &[u32], before: &[u32]| {
        let mut m = BTreeMap::new();
        for e in 0..pyramid::ENTRIES {
            let v = entry(t, e);
            if v != entry(before, e) {
                let k = pyramid::W_NODE as usize;
                let z = v[k];
                let c = [v[k + 1] as i32, v[k + 2] as i32, v[k + 3] as i32];
                m.insert((z, c, v[pyramid::W_KEY as usize]), (e, v));
            }
        }
        m
    };
    let (hand_kept, ref_kept) = (by_node(&table_hand, &table0), by_node(&table_ref, &table0));
    let mut bad_entry = 0u32;
    let mut first_bad = None;
    let mut handed_other = 0u32;
    let mut only_hand = 0u32;
    let mut filled_kept = 0u32;
    for ((zoom, c, key), (e, v)) in &hand_kept {
        let (zoom, c, key) = (*zoom, *c, *key);
        let first = pyramid::entry_of(zoom, c);
        let [far, near] = key_zooms(key);
        let (a, words) = truth(w, case.cuts, zoom, c, [far, near]);
        let k = pyramid::W_INHERITANCE as usize;
        let at = pyramid::W_ASSET as usize;
        let ok = pyramid::keeps(zoom, c)
            && (*e >= first && *e < first + BUCKET)
            && pyramid::tag_of(key) == tag
            && far == near
            && v[pyramid::W_KIND as usize]
                == pyramid::stamped(a.kind, key, a.asset, words, zoom, c)
            && v[at..at + 2] == a.asset
            && v[k..k + 4] == words;
        if !ok {
            bad_entry += 1;
            first_bad.get_or_insert((zoom, c, key, v.clone(), a.kind as u32, a.asset, words));
        }
        filled_kept += u32::from(a.kind == Kind::Cell && !answer::transparent(a.asset[0]));
        match ref_kept.get(&(zoom, c, key)) {
            Some((_, r)) => {
                // What is placed and its shard are one whatever was handed; where its
                // ground's cells were held and the ways' cursor are found again from
                // nothing, where the Rust carries down its parents'.
                let same_answer = v[pyramid::W_KIND as usize] & pyramid::KIND_MASK
                    == r[pyramid::W_KIND as usize] & pyramid::KIND_MASK
                    && v[at..at + 2] == r[at..at + 2]
                    && v[k..k + 2] == r[k..k + 2];
                if !same_answer {
                    bad_entry += 1;
                    first_bad.get_or_insert((
                        zoom,
                        c,
                        key,
                        v.clone(),
                        r[1],
                        [r[2], r[3]],
                        [r[4], r[5], r[6], r[7]],
                    ));
                }
                handed_other += u32::from(v[k + 2..k + 4] != r[k + 2..k + 4]);
            }
            None => only_hand += 1,
        }
    }
    let only_ref = ref_kept
        .keys()
        .filter(|k| !hand_kept.contains_key(*k))
        .count() as u32;

    // A node few rays visit, a ray grazing its edge may visit or not as a GPU rounds. A
    // bucket one other node at one band is asked in leaves no order to its walks, so holds
    // word for word what `Writing` left, but for what is found again.
    let mut asked_in: BTreeMap<u32, u32> = BTreeMap::new();
    for &(z, c, _) in met_ref.keys() {
        if pyramid::keeps(z, c) {
            *asked_in.entry(pyramid::entry_of(z, c)).or_default() += 1;
        }
    }
    let by_hand: BTreeSet<_> = hand_kept
        .keys()
        .map(|k| (k.0, k.1, key_zooms(k.2)))
        .collect();
    let grazed = |k: &(Zoom, [i32; 3], [Zoom; 2])| met_ref.get(k).copied().unwrap_or(0) < SURE;
    let mut alone: BTreeSet<u32> = asked_in
        .iter()
        .filter(|(_, n)| **n == 1)
        .map(|(f, _)| *f)
        .collect();
    for k in met_ref.keys().chain(by_hand.iter()) {
        if grazed(k) && pyramid::keeps(k.0, k.1) {
            alone.remove(&pyramid::entry_of(k.0, k.1));
        }
    }
    let (mut alone_differ, mut twice) = (0u32, 0u32);
    let mut first_alone = None;
    for &first in &alone {
        let of = |t: &[u32]| {
            let mut slots: Vec<_> = (first..first + BUCKET)
                .map(|e| {
                    let mut v = entry(t, e);
                    let k = pyramid::W_INHERITANCE as usize;
                    v[k + 2..k + 4].fill(0);
                    v[pyramid::W_KIND as usize] &= pyramid::KIND_MASK;
                    v
                })
                .filter(|v| v[pyramid::W_KEY as usize] != 0)
                .collect();
            slots.sort();
            let all = slots.len();
            slots.dedup();
            (slots, all)
        };
        let ((h, all), (r, _)) = (of(&table_hand), of(&table_ref));
        twice += (all - h.len()) as u32;
        if h != r {
            alone_differ += 1;
            first_alone.get_or_insert((first, h, r));
        }
    }
    // What the Rust alone kept, a grazing's only.
    let only_ref_most = ref_kept
        .keys()
        .filter(|k| !hand_kept.contains_key(*k))
        .map(|k| {
            met_ref
                .get(&(k.0, k.1, key_zooms(k.2)))
                .copied()
                .unwrap_or(0)
        })
        .max()
        .unwrap_or(0);

    // The slots laid, as the Rust's rule leaves each.
    let mut trap_cnt: BTreeMap<Trap, (u32, u32, u32)> = BTreeMap::new();
    for l in &traps {
        if !alone.contains(&l.first) || !met_ref.contains_key(&(l.zoom, l.c, l.zooms)) {
            continue;
        }
        let e = trap_cnt.entry(l.trap).or_default();
        e.0 += 1;
        e.1 += u32::from(trap_holds(&table_ref, &table0, l, tag));
        e.2 += u32::from(trap_holds(&table_hand, &table0, l, tag));
    }

    // What it found again where the Rust carried down: only what a cell placed whole hands
    // on, its parent's, which names nothing of it, as it hands nothing on.
    let mut found_otherwise = 0u32;
    let k = pyramid::W_INHERITANCE as usize;
    for (key, (_, v)) in &hand_kept {
        let Some((_, r)) = ref_kept.get(key) else {
            continue;
        };
        let placed = v[pyramid::W_KIND as usize] & pyramid::KIND_MASK
            == pyramid::kind_word(Kind::Cell)
            && answer::is_placed(v[pyramid::W_ASSET as usize]);
        found_otherwise += u32::from(v[k + 2..k + 4] != r[k + 2..k + 4] && !placed);
    }

    // Its count of what it asked.
    let lane = pyramid::tally_at(tally::LIGHT, tally::MISS) as usize;
    let tallied = u64::from(table_hand[lane]);
    let asked_lane = pyramid::tally_at(tally::LIGHT, tally::ASKED) as usize;
    let asked = u64::from(table_hand[asked_lane]);

    // The next frame: the walk over the table alone first, then this pass for what it left.
    let mut next = table_hand.clone();
    next[pyramid::FRAME as usize] = FRAME + 1;
    next[lane] = 0;
    next[asked_lane] = 0;
    strike_traps(&mut next, &traps, tag);
    let next_buf = gpu.buffer(&next, store_of());
    let list2 = gpu.buffer(&list, store_of());
    let left = gpu.buffer(&[ray::MISS_ACROSS, 0, 1, 0], store_of());
    let map2 = gpu.map(SIZE);
    let view2 = map2.create_view(&wgpu::TextureViewDescriptor {
        dimension: Some(wgpu::TextureViewDimension::D2Array),
        ..Default::default()
    });
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
        [ray::MISS_ACROSS, rows],
    );
    let after = gpu.read(&list2);
    let mut still = vec![false; n as usize];
    let mut struck = 0u32;
    for k in 0..n as usize {
        if walked[k] {
            if after[at + k] == light::MISS_STRUCK {
                struck += 1;
            } else {
                still[k] = true;
            }
        }
    }
    let slots_left = still.iter().filter(|v| **v).count() as u32;
    // Its reference, off the table the hand pass left.
    let mut table_next = next.clone();
    let mut q = Keeping {
        w,
        cuts: case.cuts,
        table: &mut table_next,
        tag: pyramid::tag(FRAME + 1, pyramid::LIGHT_MISSED_PASS),
        asks: Asks::default(),
        writes: true,
        met: None,
    };
    let mut want2 = vec![0u32; n as usize];
    for k in 0..n {
        if still[k as usize] {
            let (o, r, d) = slot_ray(case, &s, &starts, [k % SIZE, k / SIZE]);
            let t = cast::first(&mut q, o, &r, d, Until::Opaque);
            want2[k as usize] = light::word(light::depth(&s, &t));
        }
    }
    let asks_next = q.asks;
    let args = gpu.read(&left);
    gpu.dispatch(
        miss,
        &miss_binds(b, [&eye_buf, &head_buf, &next_buf, &list2], &view2),
        [args[0], args[1]],
    );
    let got2 = gpu.read_map(&map2);
    let table_after = gpu.read(&next_buf);
    if left_claimed(&table_after).len() > left_claimed(&next).len() {
        failed.push(format!(
            "{name}: the next frame's pass did not run to its end"
        ));
        return;
    }
    let mut next_differ = 0u32;
    let mut next_stepped = 0u32;
    let mut first_next = None;
    for k in 0..n as usize {
        if still[k] && got2[k].abs_diff(want2[k]) > WITHIN {
            if an_axis_over(&s, got2[k], want2[k])
                || next_stepped + next_differ <= TELLS * 8
                    && a_step_over(
                        w,
                        case,
                        &s,
                        &starts,
                        (
                            &mut table_next,
                            pyramid::tag(FRAME + 1, pyramid::LIGHT_MISSED_PASS),
                        ),
                        k as u32,
                        got2[k],
                    )
            {
                next_stepped += 1;
            } else {
                next_differ += 1;
                first_next.get_or_insert((k, got2[k], want2[k]));
                if next_differ <= TELLS {
                    tell_slot(
                        w,
                        case,
                        &s,
                        &starts,
                        k as u32,
                        [got2[k], want2[k]],
                        [&next, &table_after],
                    );
                }
            }
        }
    }
    let rewritten = (0..pyramid::ENTRIES)
        .filter(|&e| entry(&table_after, e) != entry(&next, e))
        .count() as u64;

    eprintln!("{name}: {same} alike, {axis_over} an axis over, {stepped} a step over, {differ} differ of {} ; {kinds:?}", kinds["slots"]);
    let line = |name: &str, a: &Asks| {
        eprintln!(
            "  {name}: asks {} of {} visits: {} past the roots, {} absent, {} at other zooms, {} its own; {} off a slot",
            a.of_world(), a.visits, a.past_roots, a.absent, a.other_zooms, a.own_pass, a.kept
        );
    };
    line("asks", &asks);
    eprintln!(
        "  tally: {tallied} visits, {asked} asked, by hand; most visits of a ray {most_visits}"
    );
    eprintln!(
        "  kept: hand {} ref {}, {bad_entry} bad, {only_hand} the hand's alone, {only_ref} the Rust's alone, {handed_other} found again where the Rust carried, {found_otherwise} of them no cell placed whole, {filled_kept} filled",
        hand_kept.len(), ref_kept.len()
    );
    eprintln!("  buckets of one ask: {}, {alone_differ} differ, {twice} slots a node's twice; the Rust's alone visited by {only_ref_most} rays at most; traps (laid, Rust's, hand's): {trap_cnt:?}", alone.len());
    eprintln!(
        "  next frame: {struck} ended by the table, {slots_left} left, {next_stepped} a step over, {next_differ} differ, {rewritten} slots written; tally {} asked",
        table_after[asked_lane]
    );
    line("next frame", &asks_next);

    for (what, n) in [
        ("off a slot", asks.kept),
        ("next frame, ended by the table", u64::from(struck)),
        ("next frame, left", u64::from(slots_left)),
        ("next frame, off a slot", asks_next.kept),
        ("kept", hand_kept.len() as u64),
        ("kept, filled", u64::from(filled_kept)),
        ("found again", u64::from(handed_other)),
    ] {
        kinds.insert(what, n as u32);
    }
    let mut fail = |what: String| failed.push(format!("{name}: {what}"));
    if differ != 0 {
        fail(format!("{differ} slots differ, first {first_differ:?}"));
    }
    if stepped > 32 || axis_over > 256 {
        fail(format!(
            "too many a step over: {stepped}, an axis over: {axis_over}"
        ));
    }
    if bad_entry != 0 {
        fail(format!("{bad_entry} kept wrong, first {first_bad:?}"));
    }
    if alone_differ != 0 {
        fail(format!(
            "{alone_differ} buckets differ from Writing's, first {first_alone:?}"
        ));
    }
    if found_otherwise != 0 {
        fail(format!(
            "{found_otherwise} found again where the Rust's hand it on"
        ));
    }
    if next_differ != 0 {
        fail(format!(
            "{next_differ} differ the next frame, first slot, got, wanted {first_next:?}"
        ));
    }
    for (t, (n, r, h)) in &trap_cnt {
        if r != n || h != n {
            fail(format!(
                "{t:?}: of {n} laid, the Rust's {r}, the hand's {h}"
            ));
        }
    }
    let visits = asks.visits;
    if tallied.abs_diff(visits) * 1000 > visits {
        fail(format!("tally {tallied} against {visits} visits"));
    }
    // Walks at once ask what one before would have kept: about what one after another asks.
    let of_world = asks.of_world();
    if asked > visits || asked * 3 < of_world * 2 {
        fail(format!(
            "{asked} asked against {of_world} of {visits} visits"
        ));
    }
    // A ray down a column of no ground falls to its reach, as far as `t_max` is from it.
    if most_visits > cast::STEPS_MOST / 4 {
        fail(format!(
            "a ray of {most_visits} visits near the walk's bound"
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

/// Whether the word `got` of slot `k` is what its ray ends at from a start a step over, or,
/// on a tile whose distance lies on a boundary of the zooms asked, at the band a GPU's
/// rounding of its `length` takes it to.
fn a_step_over(
    w: World,
    case: &Case,
    s: &Light,
    starts: &Starts,
    (table, tag): (&mut [u32], u32),
    k: u32,
    got: u32,
) -> bool {
    let slot = [k % SIZE, k / SIZE];
    let (o, r, d) = slot_ray(case, s, starts, slot);
    let mut ends_at = |from: [i32; 3], r: &Ray, d: Detail| {
        let mut q = Keeping {
            w,
            cuts: case.cuts,
            table: &mut *table,
            tag,
            asks: Asks::default(),
            writes: true,
            met: None,
        };
        let t = cast::first(&mut q, from, r, d, Until::Opaque);
        let word = light::word(light::depth(s, &t));
        word.abs_diff(got) <= WITHIN || an_axis_over(s, got, word)
    };
    let mut asks = vec![d];
    for by in [1.000001f32, 0.999999] {
        let moved = Light {
            across: s.across.map(|v| v * by),
            down: s.down.map(|v| v * by),
            ..*s
        };
        asks.push(slot_ray(case, &moved, starts, slot).2);
    }
    asks.dedup_by(|a, b| a.range == b.range);
    let stepped = asks.iter().any(|&d| {
        (0..27).any(|m| {
            let from = [o[0] + m % 3 - 1, o[1] + (m / 3) % 3 - 1, o[2] + m / 9 - 1];
            ends_at(from, &r, d)
        })
    });
    if stepped {
        return true;
    }
    // A cell whose edge it stops at, its span in it a few of the steps its start is rounded
    // to: starts two steps apart on two axes pass by.
    let mut q = Keeping {
        w,
        cuts: case.cuts,
        table: &mut *table,
        tag,
        asks: Asks::default(),
        writes: true,
        met: None,
    };
    let t = cast::first(&mut q, o, &r, d, Until::Opaque);
    let og = cast::grid_of(o);
    let side = (1u64 << (32 - t.hit.node.zoom)) as f32 / (1u32 << FRAC_W) as f32;
    let leaves = (0..3)
        .filter(|&a| r.dir[a] != 0.0)
        .map(|a| {
            let from =
                t.hit.node.corner[a].wrapping_sub(og[a]) as i32 as f32 / (1u32 << FRAC_W) as f32;
            (if r.dir[a] > 0.0 { from + side } else { from }) / r.dir[a]
        })
        .fold(f32::MAX, f32::min);
    t.ends == End::Hit && leaves - t.hit.t < 4.0 / (1u32 << FRAC_W) as f32
}

/// A slot that differs said where it can be read: its words, the node its reference walk
/// stopped on, and that node's bucket before the walks and after.
fn tell_slot(
    w: World,
    case: &Case,
    s: &Light,
    starts: &Starts,
    k: u32,
    [got, want]: [u32; 2],
    [before, after]: [&[u32]; 2],
) {
    let (o, r, d) = slot_ray(case, s, starts, [k % SIZE, k / SIZE]);
    let mut q = Keeping {
        w,
        cuts: case.cuts,
        table: &mut before.to_vec(),
        tag: pyramid::tag(FRAME + 2, pyramid::LIGHT_MISSED_PASS),
        asks: Asks::default(),
        writes: true,
        met: None,
    };
    let t = cast::first(&mut q, o, &r, d, Until::Opaque);
    let (z, c) = (t.hit.node.zoom, t.hit.node.idx(o));
    let bucket = |table: &[u32]| {
        let first = pyramid::entry_at(pyramid::entry_of(z, c)) as usize;
        table[first..first + (BUCKET * pyramid::ENTRY_WORDS) as usize].to_vec()
    };
    eprintln!(
        "DIFFERS {} slot {k} got {got} want {want} band {:?}: its walk stops {:?} z{z} {c:?} t {} axis {} asset {:#x}; before {:?}; after {:?}",
        case.name, d.range, t.ends, t.hit.t, t.hit.axis, t.hit.asset[0], bucket(before), bucket(after)
    );
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
        (12, list.as_entire_binding()),
    ]
}

/// `forget_cs` over stamped slots: a split under a moved brick left forgotten, read as a split
/// through its stamp, and all else cleared.
#[test]
fn forget_reads_a_stamped_kind() {
    let instance =
        wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    let Ok(adapter) = pollster::block_on(instance.request_adapter(&Default::default())) else {
        eprintln!("no adapter: skipped");
        return;
    };
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        required_limits: adapter.limits(),
        ..Default::default()
    }))
    .unwrap();
    let gpu = Gpu { device, queue };
    let mut table = vec![0u32; pyramid::WORDS as usize];
    table[pyramid::FORGETS as usize] = pyramid::FORGETS_MOVED;
    table[pyramid::MOVED_AT as usize..(pyramid::MOVED_AT + pyramid::MOVED_WORDS) as usize]
        .fill(u32::MAX);
    let key = pyramid::key(pyramid::tag(3, pyramid::BEAM_PASS), [20, 20]);
    let (zoom, c) = (18, [5, 7, 9]);
    let split = In {
        kind: Kind::Split,
        asset: [11, 13],
    };
    let cell = In {
        kind: Kind::Cell,
        asset: [15, 0],
    };
    put_entry(&mut table, 40, zoom, c, key, &split, [1, 2, 3, 4]);
    put_entry(&mut table, 44, zoom, c, key, &cell, [1, 2, 3, 4]);
    let buf = gpu.buffer(&table, store_of());
    let forget = gpu.pipeline(src(SHADER_FORGET), "forget_cs");
    gpu.dispatch(
        &forget,
        &[(9, buf.as_entire_binding())],
        [pyramid::ENTRIES / 64, 1],
    );
    let out = gpu.read(&buf);
    assert_eq!(
        out[pyramid::entry_at(40) as usize],
        pyramid::forgotten_split(key)
    );
    assert_eq!(out[pyramid::entry_at(44) as usize], 0);
}
