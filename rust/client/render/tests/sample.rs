//! `sample_hit_cs` and `sample_cs` by hand (frame/shader/sample_hit.wgsl, sample.wgsl), the eye's
//! and the light's (sample_eye.wgsl, sample_light.wgsl), answer as `cast::walk` resumed off the
//! beam (pxl-shader observer.rs `resume`) through `Writing` (tests/keeping): over the real world
//! of tests/real_world, off the lines and cells' starts the beam's passes leave there, each
//! sample's words as the Rust's `sample_put`, what it left in the observed pyramid as `Writing`
//! leaves it, and the next frame off that. Skipped where no adapter answers.

mod keeping;
mod real_world;

use std::collections::{BTreeMap, BTreeSet};

use keeping::*;
use pxl_engine::answer::{self, Kind};
use pxl_engine::beam::{self, Grid};
use pxl_engine::cast::{self, Detail, End, Node, Over, Query, Ray, Spacing, Trace, Until};
use pxl_engine::light::{self, Light, Starts};
use pxl_engine::overwrite::Cut;
use pxl_engine::param;
use pxl_engine::pyramid::{self, BUCKET};
use pxl_engine::world::Inheritance;
use pxl_engine::{eye, tally, value, Zoom, FRAC_W, LEAF_CELL};
use pxl_render::frame::shader::record::{
    src, SHADER_BEAM, SHADER_BEAM_HIT, SHADER_CELL, SHADER_LIGHT_BEAM, SHADER_LIGHT_CELL,
    SHADER_LIGHT_SAMPLE, SHADER_LIGHT_SAMPLE_HIT, SHADER_SAMPLE, SHADER_SAMPLE_HIT,
};
use pxl_render::gpu::shadow;

use real_world::{Built, World, FINEST};

/// East of X = 0 by the seam, and west of it.
const EYE: [i32; 2] = [(1 << 23) - 300, (40 << 16) + 30];
const EYE_WEST: [i32; 2] = [-(1 << 23) + 300, (40 << 16) + 30];
/// By X = 0, where X's roots part.
const EYE_ZERO: [i32; 2] = [150, (40 << 16) + 30];
const SIZE: u32 = 256;
const FRAME: u32 = 9;
const CAST: u32 = 7;
const WORKGROUP: u32 = beam::SAMPLE_SIDE;
/// Samples apart from the reference tried from a start a step over (`a_step_over`), at most.
const STEPS_TRIED: u32 = 32;
/// Rays that visit a node for it to be no grazing's.
const SURE: u32 = 9;
/// Nodes a GPU's rounding has a ray enter that another's does not.
const VISITS_OFF: u32 = 2;
/// A sample not walked: its words stand.
const UNTOUCHED: u32 = 0xdead_beef;

/// A beam ray's line as a walk resumed off it reads what was handed each node over its start.
struct LineOver<'a> {
    beams: &'a [u32],
    line: u32,
}

impl Over<Inheritance> for LineOver<'_> {
    fn over(&self, zoom: Zoom) -> Inheritance {
        let k = (self.line + beam::LINE_INHERITANCE + beam::LINE_STEP * zoom) as usize;
        Inheritance::of_words([
            self.beams[k],
            self.beams[k + 1],
            self.beams[k + 2],
            self.beams[k + 3],
        ])
    }
}

/// An observer's samples as the test lays them: its rays and words, as eye.rs `Eye` and light.rs
/// `Map` give them, and the passes over it.
struct Observed {
    name: String,
    eye: Vec<u32>,
    head: Vec<u32>,
    words: Vec<u32>,
    lattice: [u32; 2],
    lefts_at: u32,
    line_words: u32,
    /// Whether its lines hold their nodes from their own origin (`Observer::apart`).
    apart: bool,
    passes: [u32; 2],
    lanes: [u32; 2],
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

    fn cell_at(&self, s: [u32; 2]) -> u32 {
        match &self.rays {
            Rays::Eye { grid, .. } => grid.cell_at(grid.sample_cell(s)),
            Rays::Light { light: l, size, .. } => {
                let d = param::light::SAMPLE;
                let p = l.texel(*size, [s[0] * d, s[1] * d]);
                light::cell(*size, [p[0] / light::BEAM, p[1] / light::BEAM])
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

    /// Sample `s`'s walk through `q`, resumed off the beam (`observer::resume`), over its
    /// words `beams`: its met and the nodes its beam ray visited.
    fn walk(&self, q: &mut Keeping, beams: &[u32], s: [u32; 2]) -> (Trace<Inheritance>, u32) {
        let (o, ray, detail) = self.aim(s);
        self.walk_from(q, beams, s, o, &ray, detail)
    }

    /// `walk` from the start `o`.
    fn walk_from(
        &self,
        q: &mut Keeping,
        beams: &[u32],
        s: [u32; 2],
        o: [i32; 3],
        ray: &Ray,
        detail: Detail,
    ) -> (Trace<Inheritance>, u32) {
        let at = self.cell_at(s) as usize;
        let line = beams[at + beam::CELL_CORNER as usize] * self.line_words;
        let over = LineOver { beams, line };
        let resumes = beams[at + beam::CELL_RESUMES as usize] != 0;
        let t = f32::from_bits(beams[at + beam::CELL_T as usize]);
        let root = (Node::ROOT, Node::ROOT.idx(o));
        let node = self.line_node(beams, line, o);
        let from = if !resumes {
            (root.0, q.root(root.1), 0.0)
        } else {
            let (h, held) = node.over(cast::point_at(o, ray, t));
            if held {
                let h = cast::holding(o, ray, h, t);
                (h, over.over(h.zoom), t)
            } else {
                (root.0, q.root(root.1), t)
            }
        };
        let met = cast::walk(q, o, ray, detail, Until::Opaque, from, &over, &mut ());
        let before = if resumes {
            beams[(line + beam::LINE_VISITS) as usize]
        } else {
            0
        };
        (met, before)
    }

    /// The node line `line` holds, as a walk from `o` holds it.
    fn line_node(&self, beams: &[u32], line: u32, o: [i32; 3]) -> Node {
        let k = (line + beam::LINE_NODE) as usize;
        let node = Node {
            zoom: beams[k + beam::NODE_ZOOM as usize],
            corner: [0, 1, 2].map(|a| beams[k + beam::NODE_CORNER as usize + a]),
            roots: [0, 1, 2].map(|a| beams[k + beam::NODE_ROOTS as usize + a] as i32),
        };
        if !self.apart {
            return node;
        }
        let f = (line + light::LINE_ORIGIN) as usize;
        node.moved(
            [beams[f] as i32, beams[f + 1] as i32, beams[f + 2] as i32],
            o,
        )
    }

    /// Whether sample `s` resumes off a line whose origin lies in another root than its own:
    /// the line's node, held from its origin, held from the sample's in other roots, where the
    /// sample is there.
    fn line_apart(&self, beams: &[u32], s: [u32; 2]) -> bool {
        let at = self.cell_at(s) as usize;
        if !self.apart || beams[at + beam::CELL_RESUMES as usize] == 0 {
            return false;
        }
        let (o, ray, _) = self.aim(s);
        let t = f32::from_bits(beams[at + beam::CELL_T as usize]);
        let line = beams[at + beam::CELL_CORNER as usize] * self.line_words;
        let k = (line + beam::LINE_NODE + beam::NODE_ROOTS) as usize;
        let roots = [0, 1, 2].map(|a| beams[k + a] as i32);
        let node = self.line_node(beams, line, o);
        node.roots != roots && node.over(cast::point_at(o, &ray, t)).1
    }

    /// Whether sample `s`, resumed at `t` off its cell's line, starts in a node over the one its
    /// line holds where it is there: the walk's planes have it out of that one (`holding`).
    fn climbs(&self, beams: &[u32], s: [u32; 2], t: f32) -> bool {
        let at = self.cell_at(s) as usize;
        if beams[at + beam::CELL_RESUMES as usize] == 0 {
            return false;
        }
        let (o, ray, _) = self.aim(s);
        let line = beams[at + beam::CELL_CORNER as usize] * self.line_words;
        let (h, held) = self
            .line_node(beams, line, o)
            .over(cast::point_at(o, &ray, t));
        held && cast::holding(o, &ray, h, t).zoom < h.zoom
    }

    /// Each resuming cell's t moved on to where its first sample's ray, by the finest grid,
    /// still lies in the node its line holds, and the walk's planes have it out: a ray that
    /// leaves across a face as it goes down an axis. Where none is found the cell stands.
    fn climbing(&self, beams: &mut [u32]) {
        let mut seen = BTreeSet::new();
        for s in self.taken() {
            let at = self.cell_at(s) as usize;
            if beams[at + beam::CELL_RESUMES as usize] == 0 || !seen.insert(at) {
                continue;
            }
            let (o, ray, _) = self.aim(s);
            let line = beams[at + beam::CELL_CORNER as usize] * self.line_words;
            let node = self.line_node(beams, line, o);
            let t = f32::from_bits(beams[at + beam::CELL_T as usize]);
            let (h, held) = node.over(cast::point_at(o, &ray, t));
            if !held {
                continue;
            }
            // The last t, f32 by f32, its point lies in `h` by the finest grid.
            let inside = |t: f32| {
                let (n, held) = h.over(cast::point_at(o, &ray, t));
                held && n.zoom == h.zoom && t < ray.t_max
            };
            let (mut lo, mut hi) = (t.to_bits(), t.to_bits());
            let mut step = 1u32;
            while inside(f32::from_bits(hi)) && step < 1 << 24 {
                lo = hi;
                hi = hi.saturating_add(step);
                step *= 2;
            }
            while hi - lo > 1 {
                let mid = lo + (hi - lo) / 2;
                if inside(f32::from_bits(mid)) {
                    lo = mid;
                } else {
                    hi = mid;
                }
            }
            if let Some(t) = (lo.saturating_sub(4)..=lo)
                .rev()
                .map(f32::from_bits)
                .find(|&t| self.climbs(beams, s, t))
            {
                beams[at + beam::CELL_T as usize] = t.to_bits();
            }
        }
    }
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
    least: &'static [(&'static str, u32)],
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
    least: &'static [(&'static str, u32)],
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
        line_words: beam::LINE_WORDS,
        apart: false,
        passes: [pyramid::SAMPLES_PASS, pyramid::BEAM_PASS],
        lanes: [
            pyramid::tally_at(tally::EYE, tally::SAMPLE),
            pyramid::tally_at(tally::EYE, tally::ASKED),
        ],
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
        [args[0].max(1), 1],
    );
    gpu.dispatch(
        &gpu.pipeline(src(SHADER_CELL), "cell_cs"),
        &[(0, eye.as_entire_binding()), (8, beams.as_entire_binding())],
        [
            grid.span[0].div_ceil(WORKGROUP),
            grid.span[1].div_ceil(WORKGROUP),
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

/// A light box and where its rays start, as a host lays them (light.rs `Starts`).
fn box_of(c: &LightCase, moved: [i32; 2]) -> (Light, Starts, [i32; 2]) {
    let dir = unit(c.dir);
    let a = cross(dir, [0.0, 0.0, 1.0]);
    let a = if a == [0.0; 3] {
        [1.0, 0.0, 0.0]
    } else {
        unit(a)
    };
    let across = a.map(|v| v * c.side);
    let down = unit(cross(dir, a)).map(|v| v * c.side);
    let half = c.size as f32 / 2.0;
    let corner = [c.corner[0] + moved[0], c.corner[1] + moved[1]];
    let s = Light {
        first: [0, 1, 2].map(|k| -half * (across[k] + down[k]) - c.back * dir[k]),
        t_max: c.t_max,
        across,
        down,
        dir,
        bias: [0.00025, 0.0005, 0.00075],
        corner,
        reach_ndc: 1.0,
    };
    let base = corner.map(|v| v.div_euclid(light::ANCHOR) * light::ANCHOR);
    let q = (1u32 << FRAC_W) as f32;
    let anchor = |k: usize| {
        let w = [
            base[0] + (k % 2) as i32 * light::ANCHOR,
            base[1] + (k / 2) as i32 * light::ANCHOR,
        ];
        let (i, j) = ((w[0] - corner[0]) as f32, (w[1] - corner[1]) as f32);
        let v = |a: usize| (s.first[a] + i * across[a] + j * down[a]) * q;
        (
            [0, 1, 2].map(|a| c.origin[a].wrapping_add(v(a).round_ties_even() as i32)),
            [0, 1, 2].map(|a| v(a) - v(a).round_ties_even()),
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
    (s, starts, corner)
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
        line_words: light::LINE_WORDS,
        apart: true,
        passes: [pyramid::LIGHT_SAMPLES_PASS, pyramid::LIGHT_BEAM_PASS],
        lanes: [
            pyramid::tally_at(tally::LIGHT, tally::SAMPLE),
            pyramid::tally_at(tally::LIGHT, tally::ASKED),
        ],
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
    let across = light::beam_across(size);
    let mut world = world_binds(b, &eye, &beams, table);
    world.push((11, head.as_entire_binding()));
    gpu.dispatch(
        &gpu.pipeline(src(SHADER_LIGHT_BEAM), "light_beam_cs"),
        &world,
        [across.div_ceil(WORKGROUP), across.div_ceil(WORKGROUP)],
    );
    let cells = light::cells_across(size);
    gpu.dispatch(
        &gpu.pipeline(src(SHADER_LIGHT_CELL), "light_cell_cs"),
        &[
            (0, eye.as_entire_binding()),
            (8, beams.as_entire_binding()),
            (11, head.as_entire_binding()),
        ],
        [cells.div_ceil(WORKGROUP), cells.div_ceil(WORKGROUP)],
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

/// What each kind of end of a walk it took, for what the test may say.
type Kinds = BTreeMap<&'static str, u64>;

/// One frame over `table`: the hit pass, the left walked, each sample's words against the
/// reference, what it kept against `Writing`'s. The table the hand left.
#[expect(
    clippy::too_many_arguments,
    reason = "a frame's device, world, observer, words, table, traps and what it tells"
)]
fn run_frame(
    gpu: &Gpu,
    b: &WorldBufs,
    w: World,
    obs: &Observed,
    words: &[u32],
    table0: &[u32],
    frame: u32,
    traps: &[TrapAt],
    what: &str,
    kinds: &mut Kinds,
    failed: &mut Vec<String>,
    hit: &wgpu::ComputePipeline,
    world: &wgpu::ComputePipeline,
) -> Vec<u32> {
    let name = format!("{} ({what})", obs.name);
    let tag = pyramid::tag(frame, obs.passes[0]);
    let eye_buf = gpu.buffer(&obs.eye, wgpu::BufferUsages::UNIFORM);
    let head_buf = gpu.buffer(&obs.head, wgpu::BufferUsages::UNIFORM);

    // The reference: a sample at a time, over that table.
    let taken = obs.taken();
    let mut want: BTreeMap<[u32; 2], (Trace<Inheritance>, u32)> = BTreeMap::new();
    {
        let mut scratch = table0.to_vec();
        let mut q = Keeping {
            w,
            cuts: obs.cuts,
            table: &mut scratch,
            tag,
            asks: Asks::default(),
            writes: true,
            met: None,
        };
        for &s in &taken {
            if obs.needed(s) {
                want.insert(s, obs.walk(&mut q, words, s));
            }
        }
    }

    // The words a sample's put leaves unwritten stand: a sample's every word marked.
    let mut laid = words.to_vec();
    for j in 0..obs.lattice[1] {
        for i in 0..obs.lattice[0] {
            let at = obs.sample_at([i, j]) as usize;
            laid[at..at + beam::SAMPLE_WORDS as usize].fill(UNTOUCHED);
        }
    }
    let beams = gpu.buffer(&laid, store_of());
    let table = gpu.buffer(table0, store_of());
    // A pass over the pyramid alone runs with no thread writing it: what only a pass that writes
    // meets is not there. And it goes through a split held at other zooms taking no box of it,
    // which a false answer outside that box would stop.
    let mut quiet = table0.to_vec();
    for l in traps {
        if matches!(l.trap, Trap::OwnPass | Trap::TornWords | Trap::KeptFalse) {
            let e = pyramid::entry_at(l.first) as usize;
            quiet[e] = 0;
        }
    }
    // What `Writing` asks of each, over that table, keeping none: the hit pass leaves only one
    // that asks, but for a grazing.
    let mut asking = BTreeSet::new();
    {
        let mut read = quiet.clone();
        let mut q = Keeping {
            w,
            cuts: obs.cuts,
            table: &mut read,
            tag,
            asks: Asks::default(),
            met: None,
            writes: false,
        };
        for &s in &taken {
            if obs.needed(s) {
                let was = q.asks.of_world();
                obs.walk(&mut q, words, s);
                if q.asks.of_world() != was {
                    asking.insert(s);
                }
            }
        }
    }
    let quiet = gpu.buffer(&quiet, store_of());
    let left = gpu.buffer(&[0, 1, 1, 0], store_of());
    let mut binds = vec![
        (0, eye_buf.as_entire_binding()),
        (8, beams.as_entire_binding()),
        (9, quiet.as_entire_binding()),
        (10, left.as_entire_binding()),
    ];
    if obs.apart {
        binds.push((11, head_buf.as_entire_binding()));
    }
    gpu.dispatch(hit, &binds, obs.groups);
    let args = gpu.read(&left);
    let cnt = args[3];
    let opened = args[0];
    let mut after = gpu.read(&beams);
    after[obs.lefts_at as usize] = cnt;
    let listed: BTreeSet<[u32; 2]> = (0..cnt)
        .map(|p| {
            let s = after[(obs.lefts_at + 1 + p) as usize];
            [s & 0xffff, s >> 16]
        })
        .collect();
    let ended: Vec<u32> = after.clone();
    let beams2 = gpu.buffer(&after, store_of());
    let mut binds = world_binds(b, &eye_buf, &beams2, &table);
    if obs.apart {
        binds.push((11, head_buf.as_entire_binding()));
    }
    if cnt != 0 {
        gpu.dispatch(world, &binds, [args[0], 1]);
    }
    let out = gpu.read(&beams2);
    let table_hand = gpu.read(&table);
    // A slot left claimed: the dispatch did not run to its end, and its words are no answer.
    let claimed = left_claimed(&table_hand);
    if claimed.len() > left_claimed(table0).len() {
        failed.push(format!(
            "{name}: the pass did not run to its end: {} slots left claimed, first {:?}",
            claimed.len(),
            claimed.first()
        ));
        return table_hand;
    }

    // `Writing` over what the pass left, as that pass walks it, one at a time.
    let mut table_ref = table0.to_vec();
    let mut met_ref = BTreeMap::new();
    let asks = {
        let mut q = Keeping {
            w,
            cuts: obs.cuts,
            table: &mut table_ref,
            tag,
            asks: Asks::default(),
            writes: true,
            met: Some(&mut met_ref),
        };
        for &s in &listed {
            obs.walk(&mut q, words, s);
        }
        q.asks
    };

    // Each sample's words: ended by the hit, or walked after it.
    let (mut same, mut differ, mut none_put) = (0u64, 0u64, 0u64);
    let (mut axis_over, mut stepped, mut visits_off) = (0u64, 0u64, 0u64);
    let mut first = None;
    let listed_case = matches!(obs.rays, Rays::Light { taken: Some(_), .. });
    for (place, &s) in taken.iter().enumerate() {
        // Taken by a thread of a later row of the list's workgroups.
        let row = light::LIST_ACROSS * light::LISTED_LANES;
        *kinds.entry("listed, past a row").or_default() +=
            u64::from(listed_case && place as u32 >= row);
        // Taken by a thread of a row's last workgroup's last row of lanes.
        *kinds.entry("listed, a row's last lanes").or_default() +=
            u64::from(listed_case && place as u32 % row >= row - beam::SAMPLE_SIDE);
        let at = obs.sample_at(s) as usize;
        let node = at + beam::SAMPLE_NODE as usize;
        let zoom_word = node + beam::NODE_ZOOM as usize;
        let Some((met, before)) = want.get(&s) else {
            // Not needed: its zoom's word says it met nothing, the rest stand.
            none_put += 1;
            let ok = out[zoom_word] == beam::SAMPLE_NONE
                && (0..beam::SAMPLE_WORDS as usize)
                    .all(|k| at + k == zoom_word || out[at + k] == UNTOUCHED);
            if !ok {
                differ += 1;
                first.get_or_insert((s, out[at..at + 12].to_vec()));
            }
            continue;
        };
        let was_left = listed.contains(&s);
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
        // The hit's walk may go in a split whose box it took no word of, and past air held at
        // other zooms whose split another walk goes in: more or fewer, off its beam ray's. The
        // left's as many, but for a node a GPU's rounding has it not enter, or enter.
        let v = out[at + beam::SAMPLE_VISITS as usize];
        let visits = met.visits + before;
        ok = ok
            && if was_left {
                v.abs_diff(visits) <= VISITS_OFF
            } else {
                v >= *before
            };
        visits_off += u64::from(was_left && v != visits);
        // Its faces' word is `face_cs`'s.
        ok = ok && out[at + beam::SAMPLE_FACES as usize] == UNTOUCHED;
        // Left: ended by the second pass, none of it put by the first.
        ok = ok && (!was_left || ended[zoom_word] == UNTOUCHED);
        if !ok
            && stepped + differ <= u64::from(STEPS_TRIED)
            && a_step_over(
                w,
                obs,
                words,
                table0,
                tag,
                s,
                &out[at..at + beam::SAMPLE_WORDS as usize],
            )
        {
            ok = true;
            stepped += 1;
        }
        if ok {
            same += 1;
        } else {
            differ += 1;
            if first.is_none() {
                eprintln!(
                    "DIFFERS {name} {s:?} left {was_left}: want {:?} z{} {:?} {:?} t{} axis {} asset {:x?} visits {}+{before}; got {:x?}",
                    met.ends, met.hit.node.zoom, met.hit.node.corner, met.hit.node.roots, met.hit.t, met.hit.axis, met.hit.asset, met.visits, &out[at..at + beam::SAMPLE_WORDS as usize]
                );
            }
            first.get_or_insert((s, out[at..at + 12].to_vec()));
        }
        // What the test may say.
        let hits = met.ends == End::Hit;
        let a = met.hit.asset[0];
        let mut add = |k: &'static str, yes: bool| *kinds.entry(k).or_default() += u64::from(yes);
        add("samples", true);
        add("a step over", false);
        add("left", was_left);
        add("ended", !was_left);
        add("hit", hits);
        add("past its reach", met.ends == End::Miss);
        add("lacks", met.ends == End::Lacks);
        add("left, hit", was_left && hits);
        add("left, past its reach", was_left && met.ends == End::Miss);
        add("left, lacks", was_left && met.ends == End::Lacks);
        add("hit, placed", hits && answer::is_placed(a));
        add(
            "hit, short of its cube",
            hits && answer::fill_eighths(a) < 8,
        );
        add("hit, a way's", hits && value::transports(answer::val(a)));
        add("hit, on x", hits && met.hit.axis == 0);
        add("hit, on y", hits && met.hit.axis == 1);
        add("hit, on z", hits && met.hit.axis == 2);
        add("ended in another root", met.hit.node.roots != [0; 3]);
        let cell = obs.cell_at(s) as usize;
        add(
            "from the root",
            words[cell + beam::CELL_RESUMES as usize] == 0,
        );
        add("resumed", words[cell + beam::CELL_RESUMES as usize] != 0);
        let climbs = obs.climbs(
            words,
            s,
            f32::from_bits(words[cell + beam::CELL_T as usize]),
        );
        add("resumed, climbing", climbs);
        add("left, climbing", was_left && climbs);
        add(
            "resumed, its line from another root",
            obs.line_apart(words, s),
        );
    }
    // No other sample put.
    let mut stray = 0u64;
    let wanted: BTreeSet<[u32; 2]> = taken.iter().copied().collect();
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
                let c = [v[k + 1] as i32, v[k + 2] as i32, v[k + 3] as i32];
                m.insert((v[k], c, v[pyramid::W_KEY as usize]), (e, v));
            }
        }
        m
    };
    let (hand_kept, ref_kept) = (by_node(&table_hand, table0), by_node(&table_ref, table0));
    let (mut bad, mut only_hand, mut filled) = (0u64, 0u64, 0u64);
    let mut first_bad = None;
    let k = pyramid::W_INHERITANCE as usize;
    let at = pyramid::W_ASSET as usize;
    for ((zoom, c, key), (e, v)) in &hand_kept {
        let (zoom, c, key) = (*zoom, *c, *key);
        let first = pyramid::entry_of(zoom, c);
        let zooms = key_zooms(key);
        let (a, handed) = truth(w, obs.cuts, zoom, c, zooms);
        let ok = pyramid::keeps(zoom, c)
            && (*e >= first && *e < first + BUCKET)
            && pyramid::tag_of(key) == tag
            && v[pyramid::W_KIND as usize]
                == pyramid::stamped(a.kind, key, a.asset, handed, zoom, c)
            && v[at..at + 2] == a.asset
            && v[k..k + 4] == handed;
        if !ok {
            bad += 1;
            first_bad.get_or_insert((zoom, c, key, v.clone(), a.kind as u32, a.asset, handed));
        }
        filled += u64::from(a.kind == Kind::Cell && !answer::transparent(a.asset[0]));
        match ref_kept.get(&(zoom, c, key)) {
            Some((_, r)) => {
                let alike = v[pyramid::W_KIND as usize] & pyramid::KIND_MASK
                    == r[pyramid::W_KIND as usize] & pyramid::KIND_MASK
                    && v[at..at + 2] == r[at..at + 2]
                    && v[k..k + 2] == r[k..k + 2];
                if !alike {
                    bad += 1;
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
            }
            None => only_hand += 1,
        }
    }
    let only_ref = ref_kept
        .keys()
        .filter(|k| !hand_kept.contains_key(*k))
        .count();
    *kinds.entry("not needed").or_default() += none_put;
    *kinds.entry("kept").or_default() += hand_kept.len() as u64;
    *kinds.entry("kept, filled").or_default() += filled;
    let zooms_apart = hand_kept.keys().filter(|k| {
        let z = key_zooms(k.2);
        z[0] != z[1]
    });
    *kinds.entry("kept, far and near apart").or_default() += zooms_apart.count() as u64;

    // The slots laid, as the rule leaves each: where its node alone is asked in its bucket, by
    // rays enough that none is a grazing a GPU's rounding may enter or not.
    let mut asked_in: BTreeMap<u32, u32> = BTreeMap::new();
    for &(z, c, _) in met_ref.keys() {
        if pyramid::keeps(z, c) {
            *asked_in.entry(pyramid::entry_of(z, c)).or_default() += 1;
        }
    }
    let mut trap_cnt: BTreeMap<Trap, (u32, u32, u32)> = BTreeMap::new();
    for l in traps {
        if asked_in.get(&l.first) != Some(&1)
            || met_ref.get(&(l.zoom, l.c, l.zooms)).copied().unwrap_or(0) < SURE
        {
            continue;
        }
        let e = trap_cnt.entry(l.trap).or_default();
        e.0 += 1;
        e.1 += u32::from(trap_holds(&table_ref, table0, l, tag));
        e.2 += u32::from(trap_holds(&table_hand, table0, l, tag));
    }

    let left_unasked = listed.iter().filter(|s| !asking.contains(*s)).count();
    let lane = obs.lanes[0] as usize;
    let tallied = u64::from(table_hand[lane]);
    let asked = u64::from(table_hand[obs.lanes[1] as usize]);
    eprintln!(
        "{name}: {same} alike ({axis_over} an axis over, {stepped} a step over, {visits_off} visits off), {differ} differ of {} samples, {none_put} not needed, {stray} stray; {cnt} left, {left_unasked} asking nothing; tally {tallied} visits {asked} asked",
        taken.len()
    );
    eprintln!(
        "  asks {} of {} visits: {} past the roots, {} absent, {} at other zooms, {} its own; {} off a slot",
        asks.of_world(),
        asks.visits,
        asks.past_roots,
        asks.absent,
        asks.other_zooms,
        asks.own_pass,
        asks.kept
    );
    eprintln!(
        "  kept: hand {} ref {}, {bad} bad, {only_hand} the hand's alone, {only_ref} the Rust's alone, {filled} filled; traps (laid, Rust's, hand's): {trap_cnt:?}",
        hand_kept.len(),
        ref_kept.len()
    );
    let mut fail = |what: String| failed.push(format!("{name}: {what}"));
    if differ != 0 {
        fail(format!("{differ} samples differ, first {first:?}"));
    }
    if left_unasked > 16 + listed.len() / 1000 {
        fail(format!(
            "{left_unasked} of {} left that ask nothing of the world",
            listed.len()
        ));
    }
    if opened != cnt.div_ceil(beam::SAMPLE_SIDE * beam::SAMPLE_SIDE) {
        fail(format!("{opened} workgroups opened for {cnt} left"));
    }
    if stray != 0 {
        fail(format!("{stray} samples put that no thread takes"));
    }
    if stepped > 32 || axis_over > 256 || visits_off > 16 {
        fail(format!(
            "too many a step over: {stepped}, an axis over: {axis_over}, visits off: {visits_off}"
        ));
    }
    if bad != 0 {
        fail(format!("{bad} kept wrong, first {first_bad:?}"));
    }
    // Walks at once may each ask a node one before the other would have read, and claim apart:
    // a few kept by one side alone, never many.
    if only_hand as usize > hand_kept.len() / 25 + 16 || only_ref > ref_kept.len() / 4 + 16 {
        fail(format!(
            "kept apart: {only_hand} by the hand alone of {}, {only_ref} by the Rust alone of {}",
            hand_kept.len(),
            ref_kept.len()
        ));
    }
    for (t, (n, r, h)) in &trap_cnt {
        if r != n || h != n {
            fail(format!(
                "{t:?}: of {n} laid, the Rust's {r}, the hand's {h}"
            ));
        }
    }
    let sum_left: u64 = want
        .iter()
        .filter(|(s, _)| listed.contains(*s))
        .map(|(_, m)| u64::from(m.0.visits))
        .sum();
    if tallied.abs_diff(sum_left) * 100 > sum_left {
        fail(format!(
            "tally {tallied} against {sum_left} visits of the left"
        ));
    }
    if asked > tallied {
        fail(format!("{asked} asked of {tallied} visits"));
    }
    table_hand
}

/// Whether the words `got` of sample `s` are what its ray ends at from a start a step over, as a
/// GPU rounds it.
fn a_step_over(
    w: World,
    obs: &Observed,
    beams: &[u32],
    table: &[u32],
    tag: u32,
    s: [u32; 2],
    got: &[u32],
) -> bool {
    let (o, r, d) = obs.aim(s);
    (0..27).any(|m| {
        let from = [o[0] + m % 3 - 1, o[1] + (m / 3) % 3 - 1, o[2] + m / 9 - 1];
        let mut q = Keeping {
            w,
            cuts: obs.cuts,
            table: &mut table.to_vec(),
            tag,
            asks: Asks::default(),
            writes: true,
            met: None,
        };
        let (met, _) = obs.walk_from(&mut q, beams, s, from, &r, d);
        let node = beam::SAMPLE_NODE as usize;
        met.ends != End::Miss
            && got[node + beam::NODE_ZOOM as usize] == met.hit.node.zoom
            && (0..3).all(|a| got[node + beam::NODE_CORNER as usize + a] == met.hit.node.corner[a])
            && got[beam::SAMPLE_ASSET as usize] == met.hit.asset[0]
    })
}

/// An observer's two frames, the second a step moved.
#[expect(
    clippy::too_many_arguments,
    reason = "the device, the world, two frames' observers, their lines, what to reach and what failed"
)]
fn two_frames(
    gpu: &Gpu,
    b: &WorldBufs,
    w: World,
    first: &mut Observed,
    next: &mut Observed,
    lines: &mut dyn FnMut(&mut Observed, &wgpu::Buffer) -> Vec<u32>,
    least: &[(&'static str, u32)],
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
    let mut kinds = Kinds::new();
    let mut table0 = vec![0u32; pyramid::WORDS as usize];
    table0[pyramid::FRAME as usize] = FRAME;
    table0[pyramid::TALLY_ON as usize] = 1;
    let table_buf = gpu.buffer(&table0, store_of());
    let words = lines(first, &table_buf);
    let mut after_beam = gpu.read(&table_buf);

    // What the walks meet, the traps laid on them.
    let tag = pyramid::tag(FRAME, first.passes[0]);
    let mut met = BTreeMap::new();
    {
        let mut empty = after_beam.clone();
        let mut q = Keeping {
            w,
            cuts: first.cuts,
            table: &mut empty,
            tag,
            asks: Asks::default(),
            writes: true,
            met: Some(&mut met),
        };
        for s in first.taken() {
            if first.needed(s) {
                first.walk(&mut q, &words, s);
            }
        }
    }
    let traps = lay_traps(
        &mut after_beam,
        w,
        first.cuts,
        &met,
        [
            pyramid::tag(FRAME - 1, first.passes[1]),
            pyramid::tag(FRAME, first.passes[1]),
            tag,
        ],
    );
    let (hit, world) = (&hit, &world);
    let hand = run_frame(
        gpu,
        b,
        w,
        first,
        &words,
        &after_beam,
        FRAME,
        &traps,
        "first frame",
        &mut kinds,
        failed,
        hit,
        world,
    );

    // The next frame, the eye a little moved: off what this one kept, its false answers gone.
    let mut table1 = hand;
    table1[pyramid::FRAME as usize] = FRAME + 1;
    table1[first.lanes[0] as usize] = 0;
    table1[first.lanes[1] as usize] = 0;
    strike_traps(&mut table1, &traps, tag);
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
        FRAME + 1,
        &[],
        "next frame",
        &mut kinds,
        failed,
        hit,
        world,
    );
    eprintln!("  KINDS {} {kinds:?}", first.name);
    for (what, n) in least {
        let got = kinds.get(what).copied().unwrap_or(0);
        if got < u64::from(*n) {
            failed.push(format!(
                "{}: too few of {what} to say: {got}, wanted {n}",
                first.name
            ));
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
            least: &[
                ("hit", 25438),
                ("hit, on x", 5926),
                ("hit, on y", 454),
                ("hit, placed", 24384),
                ("kept", 30),
                ("kept, far and near apart", 3),
                ("kept, filled", 22),
                ("left", 2615),
                ("left, hit", 2615),
                ("resumed", 25438),
            ],
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
            least: &[
                ("hit", 1889),
                ("hit, on x", 285),
                ("hit, on y", 825),
                ("hit, short of its cube", 616),
                ("kept", 215),
                ("kept, far and near apart", 29),
                ("kept, filled", 75),
                ("left", 3550),
                ("left, hit", 1792),
                ("left, past its reach", 1757),
                ("past its reach", 47565),
                ("resumed", 49454),
            ],
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
            least: &[
                ("hit", 49454),
                ("hit, on x", 1330),
                ("hit, on y", 720),
                ("hit, placed", 134),
                ("kept", 45),
                ("kept, far and near apart", 5),
                ("kept, filled", 28),
                ("left", 32784),
                ("left, hit", 32784),
                ("resumed", 49454),
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
            least: &[
                ("hit", 37446),
                ("hit, a way's", 3885),
                ("hit, on x", 1283),
                ("hit, on y", 8826),
                ("hit, placed", 32006),
                ("kept", 16),
                ("kept, far and near apart", 1),
                ("kept, filled", 12),
                ("left", 4170),
                ("left, hit", 4170),
                ("resumed", 37446),
            ],
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
            least: &[
                ("hit", 37446),
                ("hit, placed", 18723),
                ("kept", 1),
                ("left", 37446),
                ("left, hit", 37446),
                ("resumed", 37446),
            ],
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
            least: &[
                ("lacks", 49454),
                ("left", 24727),
                ("left, lacks", 24727),
                ("resumed", 49454),
            ],
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
            least: &[
                ("hit", 31126),
                ("hit, a way's", 2496),
                ("hit, on x", 3753),
                ("hit, on y", 2900),
                ("hit, short of its cube", 17911),
                ("kept", 1219),
                ("kept, far and near apart", 55),
                ("kept, filled", 649),
                ("left", 12302),
                ("left, hit", 12302),
                ("resumed", 31126),
            ],
        },
    ];
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
            c.least,
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
        least: &[
            ("hit", 25438),
            ("hit, on x", 120),
            ("hit, on y", 58),
            ("hit, placed", 25222),
            ("kept", 7),
            ("kept, far and near apart", 2),
            ("kept, filled", 5),
            ("left", 2176),
            ("left, hit", 2176),
            ("resumed", 25438),
        ],
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
        c.least,
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
        least: &[
            ("hit", 25438),
            ("hit, on x", 24630),
            ("hit, on y", 630),
            ("kept", 11),
            ("kept, far and near apart", 4),
            ("kept, filled", 7),
            ("left", 18311),
            ("left, hit", 18311),
            ("resumed", 25438),
        ],
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
        c.least,
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
        least: &[],
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
        c.least,
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
            least: &[
                ("from the root", 2730),
                ("hit", 2730),
                ("hit, a way's", 164),
                ("hit, on x", 234),
                ("hit, on y", 412),
                ("hit, placed", 1790),
                ("hit, short of its cube", 43),
                ("kept", 6788),
                ("kept, filled", 1600),
                ("left", 1726),
                ("left, hit", 1726),
            ],
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
            least: &[
                ("from the root", 504),
                ("hit", 504),
                ("hit, a way's", 25),
                ("hit, on x", 47),
                ("hit, on y", 79),
                ("hit, placed", 307),
                ("hit, short of its cube", 5),
                ("kept", 1469),
                ("kept, filled", 296),
                ("left", 339),
                ("left, hit", 339),
                ("not needed", 517),
            ],
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
            least: &[
                ("from the root", 3872),
                ("hit", 3872),
                ("kept", 8400),
                ("left", 2000),
                ("listed, a row's last lanes", 16),
                ("not needed", 4320),
            ],
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
            least: &[
                ("from the root", 2392),
                ("hit", 11272),
                ("hit, a way's", 357),
                ("hit, short of its cube", 2716),
                ("kept", 4400),
                ("left", 3500),
                ("left, past its reach", 50),
                ("listed, past a row", 18512),
                ("not needed", 13824),
                ("resumed", 10488),
            ],
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
            least: &[
                ("hit", 2730),
                ("hit, a way's", 72),
                ("hit, on x", 314),
                ("hit, on y", 352),
                ("hit, placed", 1892),
                ("hit, short of its cube", 1),
                ("kept", 9706),
                ("kept, filled", 1556),
                ("left", 1753),
                ("left, hit", 1753),
                ("resumed", 2730),
            ],
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
            least: &[
                ("from the root", 2730),
                ("kept", 4251),
                ("lacks", 2730),
                ("left", 2730),
                ("left, lacks", 2730),
            ],
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
            least: &[
                ("from the root", 2730),
                ("hit", 2730),
                ("hit, a way's", 137),
                ("hit, on x", 269),
                ("hit, on y", 352),
                ("hit, short of its cube", 983),
                ("kept", 8552),
                ("kept, filled", 1977),
                ("left", 1662),
                ("left, hit", 1662),
            ],
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
            least: &[
                ("from the root", 2730),
                ("hit", 2730),
                ("hit, a way's", 2),
                ("hit, on x", 120),
                ("hit, on y", 343),
                ("hit, placed", 686),
                ("kept", 9225),
                ("kept, filled", 1424),
                ("left", 1737),
                ("left, hit", 1737),
            ],
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
            least: &[
                ("from the root", 2664),
                ("hit", 2632),
                ("hit, a way's", 51),
                ("hit, placed", 1888),
                ("kept", 9183),
                ("kept, filled", 2117),
                ("left", 1538),
                ("left, hit", 1525),
                ("left, past its reach", 13),
                ("past its reach", 98),
                ("resumed", 66),
            ],
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
            least: &[
                ("from the root", 474),
                ("hit", 481),
                ("hit, a way's", 6),
                ("hit, on x", 70),
                ("hit, on y", 27),
                ("hit, placed", 317),
                ("kept", 1080),
                ("kept, filled", 380),
                ("left", 295),
                ("left, hit", 292),
                ("not needed", 517),
                ("past its reach", 22),
                ("resumed", 29),
            ],
        },
    ];
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
            c.least,
            &mut failed,
        );
    }
    // Each cell resumed where its ray leaves its line's node down the axis it runs along, at a
    // whole unit, where the walk's planes and the finest grid part exactly: `resumed` climbs.
    let c = LightCase {
        name: "the light along an axis, one band, resumed where its node is left",
        range: [LEAF_CELL, LEAF_CELL],
        least: &[
            ("hit", 7897),
            ("left", 4100),
            ("left, climbing", 250),
            ("resumed", 8192),
            ("resumed, climbing", 552),
        ],
        ..*cases
            .iter()
            .find(|c| c.name == "the light, on a bridge, along an axis")
            .unwrap()
    };
    let mut first = light_of(&c, [0, 0]);
    let mut next = light_of(&c, [0, 0]);
    two_frames(
        &gpu,
        &b,
        w,
        &mut first,
        &mut next,
        &mut |o, table| {
            let mut words = light_beams(&gpu, &b, o, table, c.listed);
            o.climbing(&mut words);
            words
        },
        c.least,
        &mut failed,
    );
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
        least: &[
            ("hit", 2730),
            ("hit, a way's", 70),
            ("hit, on x", 631),
            ("hit, on y", 272),
            ("hit, placed", 114),
            ("kept", 8653),
            ("kept, filled", 2306),
            ("left", 1819),
            ("left, hit", 1819),
            ("resumed", 2730),
        ],
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
        c.least,
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
        least: &[("ended in another root", 190), ("resumed", 2730)],
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
        c.least,
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
        least: &[
            ("ended in another root", 1100),
            ("resumed", 8192),
            ("resumed, its line from another root", 16),
        ],
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
        c.least,
        &mut failed,
    );
    assert!(failed.is_empty(), "{}", failed.join("\n"));
}
