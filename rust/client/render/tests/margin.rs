//! The light's cells' starts (light_beam.wgsl `light_beam_cs`, light_cell.wgsl
//! `light_cell_cs`) against pxl-engine `light::margin` of `Light::spacing`: over the real world
//! of tests/real_world, each beam ray's hand and each cell's `CELL_T` as the engine takes them
//! off where the ray stopped. Where it stopped is read off the same pass with the margin's
//! constants set to zero, which leaves the walk as it is and hands its `t` whole. A walk that
//! runs out of steps hands on the light's reach (beam_walk.wgsl, as ask.wgsl `walk_asking`
//! ends one): read off that pass again with its bound, `STEPS_MOST`, set to `STEPS_FEW`, each
//! walk the fewer bound cuts short against its reach, every other against its whole walk.
//! Each beam ray's words, under either bound, as pxl-engine `cast::walk_at_most` of that bound
//! leaves them. Skipped where no adapter answers.

mod gpu;
mod keeping;
mod real_world;

use keeping::*;
use pxl_engine::cast::{Detail, Keep, Node, Query, Ray, Root, Trace, Until, DEPTH};
use pxl_engine::light::{self, Light, Starts};
use pxl_engine::overwrite::Cut;
use pxl_engine::world::Inheritance;
use pxl_engine::{beam, cast};
use pxl_engine::{eye, node_cache, Zoom, FRAC_W};
use pxl_render::frame::shader::record::{src, SHADER_LIGHT_BEAM, SHADER_LIGHT_CELL};

use real_world::{World, FINEST};

/// East of X = 0 by the seam, as tests/sample.rs stands.
const EYE: [i32; 2] = [(1 << 23) - 300, (40 << 16) + 30];
const SIZE: u32 = 256;
const FRAME: u32 = 9;
const CAST: u32 = 7;
const WORKGROUP: u32 = beam::SAMPLE_SIDE;
/// The walk's bound (`cast::STEPS_MOST`) as the test lays it: short of many beam rays' walks,
/// past others.
const STEPS_FEW: u32 = 24;
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

/// A beam ray's line as light_beam.wgsl leaves it: the node it ended at and what was handed
/// each node from the root to it.
struct Line {
    node: Node,
    at: [Inheritance; DEPTH],
}

impl Keep<Inheritance> for Line {
    fn keep(&mut self, met: &Trace<Inheritance>, stack: &[Inheritance; DEPTH]) {
        let z = met.hit.node.zoom as usize;
        self.node = met.hit.node;
        self.at[..z].copy_from_slice(&stack[..z]);
        self.at[z] = met.hit.at;
    }
}

/// The words beam ray `c` of the light's box, from origin `o`, leaves in its line, and its
/// hand, as `cast::walk_at_most` of `steps` from the world's root leaves them with the margin
/// zero: each word's place in the light's list beside its name.
fn line_of(
    w: World,
    l: &Light,
    range: [Zoom; 2],
    c: [u32; 2],
    o: [i32; 3],
    steps: u32,
) -> Vec<(u32, u32, String)> {
    let p = light::corner_at(c);
    let band = l.band(SIZE, range, p.map(|v| v.div_euclid(light::TILE as i32)));
    let detail = Detail {
        range: [band, band],
        spacing: l.spacing(),
    };
    // A table no frame has written: every node asked of the world, as the pass's own.
    let mut table = vec![0u32; node_cache::WORDS as usize];
    let mut q = Keeping {
        w,
        cuts: [Cut::NONE; 2],
        table: &mut table,
        tag: node_cache::tag(FRAME, node_cache::LIGHT_BEAM_PASS),
        asks: Asks::default(),
        met: None,
        writes: false,
    };
    let at = q.root(Node::ROOT.idx(o));
    let mut line = Line {
        node: Node::ROOT,
        at: [at; DEPTH],
    };
    let met = cast::walk_at_most(
        &mut q,
        o,
        &Ray::new(l.dir, l.t_max),
        detail,
        Until::Beam,
        (Node::ROOT, at, 0.0),
        &Root(at),
        &mut line,
        steps,
    );
    let across = light::beam_across(SIZE);
    let k = c[1] * across + c[0];
    let base = k * light::LINE_WORDS;
    let n = base + beam::LINE_NODE;
    let mut words = vec![
        (
            light::hand_at(SIZE, k),
            met.hit.t.max(0.0).to_bits(),
            "hand".to_string(),
        ),
        (base + beam::LINE_VISITS, met.visits, "visits".to_string()),
        (n + beam::NODE_ZOOM, line.node.zoom, "zoom".to_string()),
    ];
    for a in 0..3 {
        words.push((
            n + beam::NODE_CORNER + a,
            line.node.corner[a as usize],
            format!("corner {a}"),
        ));
        words.push((
            n + beam::NODE_ROOTS + a,
            line.node.roots[a as usize] as u32,
            format!("roots {a}"),
        ));
    }
    for z in 0..=line.node.zoom {
        let at = base + beam::LINE_INHERITANCE + beam::LINE_STEP * z;
        for (e, v) in line.at[z as usize].words().into_iter().enumerate() {
            words.push((at + e as u32, v, format!("inheritance at {z}, word {e}")));
        }
    }
    words
}

fn device() -> Option<Gpu> {
    // A hand shader's loop may hang the device's wait: the run gives up.
    std::thread::spawn(|| {
        std::thread::sleep(std::time::Duration::from_secs(170));
        eprintln!("gave up after 170 s");
        std::process::exit(124);
    });
    let instance =
        wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    let Ok(adapter) = pollster::block_on(instance.request_adapter(&Default::default())) else {
        eprintln!("no adapter: skipped");
        return None;
    };
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        required_limits: adapter.limits(),
        ..Default::default()
    }))
    .unwrap();
    Some(Gpu { device, queue })
}

/// The light's list after `light_beam_cs` of `beam_source` and `light_cell_cs` over a whole
/// cast, from a table no frame has written.
fn beams_of(gpu: &Gpu, b: &WorldBufs, eye: &[u32], head: &[u32], beam_source: &str) -> Vec<u32> {
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
    let across = light::beam_across(SIZE);
    gpu.dispatch(
        &gpu.pipeline(beam_source, "light_beam_cs"),
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
        [across.div_ceil(WORKGROUP), across.div_ceil(WORKGROUP)],
    );
    let cells = light::cells_across(SIZE);
    gpu.dispatch(
        &gpu.pipeline(src(SHADER_LIGHT_CELL), "light_cell_cs"),
        &[
            (0, eye.as_entire_binding()),
            (8, beams.as_entire_binding()),
            (11, head.as_entire_binding()),
        ],
        [cells.div_ceil(WORKGROUP), cells.div_ceil(WORKGROUP)],
    );
    gpu.read(&beams)
}

/// The margins a device may take for `light::margin` of a beam `width` wide, a rounding each,
/// named: the engine's, and the product fused into the sum. The width is the host's word
/// (`Light::width`), so nothing else rounds apart.
fn margins(width: f32) -> [(&'static str, f32); 2] {
    let fused = (2.0 * cast::SQRT_6).mul_add(width, light::margin(0.0));
    [
        ("the engine's", light::margin(width)),
        ("a fused sum's", fused),
    ]
}

/// Each beam ray's hand and each cell's `CELL_T` by hand against `light::margin` of the light's
/// spacing, off where each beam ray stopped: two boxes, one a texel a unit, one narrow and
/// slanting. Each walk `STEPS_FEW` cuts short hands on the light's reach.
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
    let source = src(SHADER_LIGHT_BEAM);
    let stopped = gpu::set_const(
        &gpu::set_const(source, "LIGHT_MARGIN_PER_WIDTH", "0.0"),
        "LIGHT_MARGIN_STEPS",
        "0.0",
    );
    let few = gpu::set_const(&stopped, "STEPS_MOST", &format!("{STEPS_FEW}u"));
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
        let got = beams_of(&gpu, &b, &eye, &head, source);
        let ts = beams_of(&gpu, &b, &eye, &head, &stopped);
        let cut = beams_of(&gpu, &b, &eye, &head, &few);
        let hand_of = |w: &[u32], k: u32| f32::from_bits(w[light::hand_at(SIZE, k) as usize]);
        let visits_of = |w: &[u32], k: u32| w[(k * light::LINE_WORDS + beam::LINE_VISITS) as usize];
        // A walk cut short asked fewer nodes than its whole walk; any other is its whole walk.
        let (mut short, mut whole, mut most) = (0, 0, 0);
        for k in 0..across * across {
            let (v, all) = (visits_of(&cut, k), visits_of(&ts, k));
            most = most.max(all);
            let (got, want) = (hand_of(&cut, k), hand_of(&ts, k));
            if v < all {
                short += 1;
                if got.to_bits() != c.t_max.to_bits() || v > STEPS_FEW {
                    failed.push(format!(
                        "{}: beam ray {k} cut short at {v} of {all} visits hands {got:?}, its reach {:?}",
                        c.name, c.t_max
                    ));
                }
            } else {
                whole += u32::from(all > 0);
                if v != all || got.to_bits() != want.to_bits() {
                    failed.push(format!(
                        "{}: beam ray {k} of {all} visits, {v} under {STEPS_FEW} steps, hands {got:?}, its whole walk {want:?}",
                        c.name
                    ));
                }
            }
        }
        eprintln!(
            "{}: under {STEPS_FEW} steps {short} beam rays cut short, {whole} whole; the most visits {most} of {}",
            c.name,
            cast::STEPS_MOST
        );
        assert!(
            short > 0 && whole > 0,
            "{}: too few to say: {short} cut short, {whole} whole",
            c.name
        );
        // Every word each beam ray leaves under either bound, as the engine's walk of that bound
        // from the origin the pass laid: a device rounds `Starts::start` a Q3 step off the
        // engine's for some.
        let w = built.world();
        let mut words = 0;
        for (pass, steps) in [(&ts, cast::STEPS_MOST), (&cut, STEPS_FEW)] {
            for k in 0..across * across {
                let at = [k % across, k / across];
                let f = (k * light::LINE_WORDS + light::LINE_ORIGIN) as usize;
                let o = [0, 1, 2].map(|a| pass[f + a] as i32);
                for (i, want, name) in line_of(w, &l, c.range, at, o, steps) {
                    let got = pass[i as usize];
                    words += 1;
                    // Where it starts at zero, a device may hand -0.
                    let zero =
                        name == "hand" && f32::from_bits(got) == 0.0 && f32::from_bits(want) == 0.0;
                    if got != want && !zero {
                        failed.push(format!(
                            "{}: under {steps} steps beam ray {k}'s {name} {got:#x}, the engine's {want:#x}",
                            c.name
                        ));
                        break;
                    }
                }
            }
        }
        eprintln!("{}: {words} words of the beam rays as the engine's", c.name);
        let t: Vec<f32> = (0..across * across).map(|k| hand_of(&ts, k)).collect();
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
