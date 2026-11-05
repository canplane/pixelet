//! The light's cells' starts (beam_light.wgsl `beam_hand`, light_cell.wgsl
//! `light_cell_cs`) against pxl-engine `light::margin` of `Light::spacing`: over the real world
//! of tests/real_world, each beam ray's hand and each cell's `CELL_T` as the engine takes them
//! off where the engine's walk of that ray from the origin the pass laid stops. Skipped where
//! no adapter answers.

mod gpu;
mod real_world;

use gpu::{margins, Gpu};
use pxl_engine::cast::{Detail, Keep, Node, Query, Ray, Root, Trace, Until, DEPTH};
use pxl_engine::light::{self, Light, Starts};
use pxl_engine::overwrite::Cut;
use pxl_engine::world::Inheritance;
use pxl_engine::{beam, cast};
use pxl_engine::{eye, node_cache, Zoom, FRAC_W};
use pxl_render::frame::shader::record::{src, SHADER_LIGHT_CELL};

use real_world::{q8, store_of, Asking, World, WorldBufs, FINEST};

/// East of X = 0 by the seam, as tests/sample.rs stands.
const EYE: [i32; 2] = [(1 << 23) - 300, (40 << 16) + 30];
const SIZE: u32 = 256;
const FRAME: u32 = 9;
const CAST: u32 = 7;
const WORKGROUP: u32 = beam::SAMPLE_SIDE;
/// A light's box over the world.
struct LightCase {
    name: &'static str,
    origin: [i32; 3],
    dir: [f32; 3],
    side: f32,
    back: f32,
    t_max: f32,
    range: [Zoom; 2],
    corner: [i32; 2],
}

fn unit(v: [f32; 3]) -> [f32; 3] {
    let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    v.map(|x| x / l)
}

fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

/// The light and where its rays start, as a host lays them (light.rs `Starts`).
fn box_of(c: &LightCase) -> (Light, Starts) {
    let dir = unit(c.dir);
    let a = unit(cross(dir, [0.0, 0.0, 1.0]));
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
            [0, 1, 2].map(|a| c.origin[a].wrapping_add(v(a).round_ties_even() as i32)),
            [0, 1, 2].map(|a| v(a) - v(a).round_ties_even()),
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

/// A walk that keeps nothing of its line.
struct Unkept;

impl Keep<Inheritance> for Unkept {
    fn keep(&mut self, _: &Trace<Inheritance>, _: &[Inheritance; DEPTH]) {}
}

/// Where beam ray `c` of the light's box, from origin `o`, stops, as `cast::walk_at_most` from
/// the world's root takes it: its hand with the margin zero.
fn stop_of(w: World, l: &Light, range: [Zoom; 2], c: [u32; 2], o: [i32; 3]) -> f32 {
    let p = light::corner_at(c);
    let band = l.band(SIZE, range, p.map(|v| v.div_euclid(light::TILE as i32)));
    let detail = Detail {
        range: [band, band],
        spacing: l.spacing(),
    };
    // A table no frame has written: every node asked of the world.
    let mut q = Asking {
        w,
        cuts: [Cut::NONE; 2],
    };
    let at = q.root(Node::ROOT.idx(o));
    let met = cast::walk_at_most(
        &mut q,
        o,
        &Ray::new(l.dir, l.t_max),
        detail,
        Until::Beam,
        (Node::ROOT, at, 0.0),
        &Root(at),
        &mut Unkept,
        cast::STEPS_MOST,
    );
    met.hit.t.max(0.0)
}

fn device() -> Option<Gpu> {
    // A hand shader's loop may hang the device's wait: the run gives up.
    std::thread::spawn(|| {
        std::thread::sleep(std::time::Duration::from_secs(170));
        eprintln!("gave up after 170 s");
        std::process::exit(124);
    });
    let gpu = Gpu::new();
    if gpu.is_none() {
        eprintln!("no adapter: skipped");
    }
    gpu
}

/// The light's list after its beam and `light_cell_cs` over a whole cast, from a table no frame
/// has written.
fn beams_of(gpu: &Gpu, b: &WorldBufs, eye: &[u32], head: &[u32]) -> Vec<u32> {
    let eye = gpu.buffer(eye, wgpu::BufferUsages::UNIFORM);
    let head = gpu.buffer(head, wgpu::BufferUsages::UNIFORM);
    let mut table = vec![0u32; node_cache::WORDS as usize];
    table[node_cache::FRAME as usize] = FRAME;
    let table = gpu.buffer(&table, store_of());
    let tiles = light::tiles(SIZE);
    let mut list = vec![0u32; light::list_words(SIZE) as usize];
    let reads_at = light::reads_at(SIZE) as usize;
    list[reads_at..reads_at + (tiles * tiles) as usize].fill(CAST);
    let coarse_at = light::coarse_at(SIZE) as usize;
    let squares = light::coarse(SIZE) * light::coarse(SIZE);
    list[coarse_at..coarse_at + squares as usize].fill(CAST);
    let beams = gpu.buffer(&list, store_of());
    gpu.light_beam(
        &[
            (0, eye.as_entire_binding()),
            (1, b.clip.as_entire_binding()),
            (2, b.ground.as_entire_binding()),
            (4, b.palette.as_entire_binding()),
            (5, b.shards.as_entire_binding()),
            (6, b.placed.as_entire_binding()),
            (7, b.pool.as_entire_binding()),
            (8, beams.as_entire_binding()),
            (9, table.as_entire_binding()),
            (11, head.as_entire_binding()),
        ],
        &beams,
        SIZE,
    );
    let cells = light::cells_across(SIZE);
    gpu.dispatch(
        &gpu.pipeline(src(SHADER_LIGHT_CELL), "light_cell_cs"),
        &[
            (0, eye.as_entire_binding()),
            (8, beams.as_entire_binding()),
            (11, head.as_entire_binding()),
        ],
        [cells.div_ceil(WORKGROUP), cells.div_ceil(WORKGROUP), 1],
    );
    gpu.read(&beams)
}

/// Each beam ray's hand and each cell's `CELL_T` by hand against `light::margin` of the light's
/// spacing, off where the engine's walk of each beam ray stops: two boxes, one a texel a unit,
/// one narrow and slanting.
#[test]
fn light_cells_start_short_by_the_margin() {
    let Some(gpu) = device() else { return };
    let built = real_world::build(EYE);
    let store = store_of();
    let flat_u = |rows: &[[u32; 4]]| rows.iter().flatten().copied().collect::<Vec<u32>>();
    let clip: Vec<u32> = built.clipmap.iter().flatten().map(|&v| v as u32).collect();
    let b = WorldBufs {
        clip: gpu.buffer(&clip, store),
        ground: gpu.buffer(&built.ground, store),
        palette: gpu.buffer(&built.palette, store),
        shards: gpu.buffer(&flat_u(&built.shards), store),
        placed: gpu.buffer(&built.overwrites, store),
        pool: gpu.buffer(&built.pool, store),
    };
    let over = |g: [i32; 2], up: i32| q8([g[0], g[1], built.top_at(g) + up]);
    let cases = [
        LightCase {
            name: "the light, about the eye",
            origin: over([EYE[0] + 90, EYE[1] + 90], 40),
            dir: [0.3, 0.2, -0.93],
            side: 1.0,
            back: 200.0,
            t_max: 700.0,
            range: [8, FINEST],
            corner: [1300, -900],
        },
        LightCase {
            name: "the light, narrow and slanting",
            origin: over([EYE[0] + 40, EYE[1] - 20], 25),
            dir: [0.6, -0.3, -0.74],
            side: 0.37,
            back: 120.0,
            t_max: 500.0,
            range: [8, FINEST],
            corner: [-16, 32],
        },
    ];
    let across = light::beam_across(SIZE);
    let cells = light::cells_across(SIZE);
    let side = 2 + 2 * light::BEAM_WINDOW;
    let mut failed = Vec::new();
    for c in &cases {
        let (l, starts) = box_of(c);
        let mut eye = vec![0u32; eye::WORDS];
        for at in [eye::CARVE, eye::HIDE] {
            eye[at + 3] = Cut::NONE.zoom;
        }
        eye[eye::LIGHT..eye::LIGHT + eye::LIGHT_WORDS].copy_from_slice(&l.words());
        eye[eye::RANGE] = c.range[0];
        eye[eye::RANGE + 1] = c.range[1];
        let mut head = vec![0u32; light::HEAD_WORDS];
        head[light::HEAD_CAST] = CAST;
        head[light::HEAD_EPOCH] = CAST;
        for (k, v) in light::head_layout(SIZE) {
            head[k] = v;
        }
        head[light::STARTS_AT * 4..(light::STARTS_AT + light::STARTS_VEC4S) * 4]
            .copy_from_slice(&starts.words());
        let got = beams_of(&gpu, &b, &eye, &head);
        let hand_of = |w: &[u32], k: u32| f32::from_bits(w[light::hand_at(SIZE, k) as usize]);
        // Each beam ray's stop, the engine's walk from the origin the pass laid: a device rounds
        // `Starts::start` a Q3 step off the engine's for some.
        let w = built.world();
        let t: Vec<f32> = (0..across * across)
            .map(|k| {
                let f = (k * light::LINE_WORDS + light::LINE_ORIGIN) as usize;
                let o = [0, 1, 2].map(|a| got[f + a] as i32);
                stop_of(w, &l, c.range, [k % across, k / across], o)
            })
            .collect();
        let hands: Vec<f32> = (0..across * across).map(|k| hand_of(&got, k)).collect();
        let hand = |m: f32| -> Vec<f32> { t.iter().map(|&t| (t - m).max(0.0)).collect() };
        let least = |h: &[f32], [i, j]: [u32; 2]| {
            (0..side)
                .flat_map(|y| (0..side).map(move |x| (j + y) * across + i + x))
                .map(|k| h[k as usize])
                .fold(f32::INFINITY, f32::min)
        };
        let margins = margins(l.spacing().across);
        let engine = hand(margins[0].1);
        // The one rounding every hand of the pass takes: one spacing, one margin.
        let Some(&(cause, m)) = margins.iter().find(|&&(_, m)| {
            hand(m)
                .iter()
                .zip(&hands)
                .all(|(w, g)| w.to_bits() == g.to_bits())
        }) else {
            let k = (0..hands.len())
                .find(|&k| hands[k].to_bits() != engine[k].to_bits())
                .unwrap();
            failed.push(format!(
                "{}: beam ray {k}'s hand {:?} under no margin's rounding, the engine's {:?}",
                c.name, hands[k], engine[k]
            ));
            continue;
        };
        let taken = hand(m);
        let moved = (0..hands.len())
            .filter(|&k| hands[k].to_bits() != engine[k].to_bits())
            .count();
        let past = hands.iter().filter(|&&h| h > 0.0).count();
        let (mut cells_moved, mut cells_past) = (0, 0);
        for j in 0..cells {
            for i in 0..cells {
                let at = light::cell(SIZE, [i, j]) as usize + beam::CELL_T as usize;
                let got_t = f32::from_bits(got[at]);
                let want = least(&taken, [i, j]);
                if got_t.to_bits() != want.to_bits() {
                    failed.push(format!(
                        "{}: cell {i} {j}'s start {got_t:?}, its window's least {want:?}",
                        c.name
                    ));
                }
                cells_moved += u32::from(got_t.to_bits() != least(&engine, [i, j]).to_bits());
                cells_past += u32::from(got_t > 0.0);
            }
        }
        eprintln!(
            "{}: the engine's margin {:?}, taken as {cause}: {m:?}; off the engine's by it {moved} of {} hands, {cells_moved} of {} cells; past zero {past} hands, {cells_past} cells",
            c.name,
            margins[0].1,
            hands.len(),
            cells * cells,
        );
        assert!(
            past > hands.len() / 4 && cells_past > cells * cells / 4,
            "{}: too few past zero to say: {past} hands, {cells_past} cells",
            c.name
        );
    }
    assert!(failed.is_empty(), "{}", failed.join("\n"));
}
