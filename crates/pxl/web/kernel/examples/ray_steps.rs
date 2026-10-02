// pxl-kernel/examples/ray_steps.rs

//! How many steps a ray cast into the records takes, per pixel, from the
//! views the drawing experiment fixes (issue/BACKLOG.md, "How the samples are
//! drawn"): straight down, at 45° and along the ground at street level.
//!
//! A cast's cost is pixels times steps. The pixels are the screen's; the
//! steps are the query's own (world/access.rs `first_hit`): the regions read
//! and the cells walked. Both are counted here on the CPU over real terrain,
//! which says before any shader is written how many steps a pixel would pay,
//! and how far a grazing view's spread runs past a view from above. What a
//! phone's GPU makes of those steps — divergence, the texture cache — is the
//! experiment's to measure, not this.
//!
//! Pages come from a running tile server, as the client's do: rays are cast,
//! the pages they lacked are fetched, and they are cast again until nothing
//! they lack can be had. A page past the server's top level is not served,
//! and a ray that needs one answers coarser, as the client's would.
//!
//! Each view's steps are also written as a heatmap in screen coordinates
//! (`crates/target/ray_steps/<view>.ppm`, black none, through blue and green to
//! white at the most any ray took): where on the screen the long rays lie —
//! a line at the horizon, a whole slope, every ridge — decides more of what
//! a GPU pays than their mean, since rays run in groups that wait on their
//! longest.
//!
//!   cargo run --release -p pxl-kernel --example ray_steps -- [lon lat] [server]

use std::cell::Cell;
use std::collections::HashSet;
use std::process::Command;

use pxl_kernel::layer::object::store::Store;
use pxl_kernel::world::access::{Access, Answer, Detail, Ray, FINEST};
use pxl_kernel::world::paging::{Candidate, Paging, Victim};
use pxl_spec::bake::format::{decode_terrain, Terrain};
use pxl_spec::bake::quadkey::page_name;
use pxl_spec::space::adapter::epsg4326;
use pxl_spec::space::lod::Tau;
use pxl_spec::space::pvf3::pvf3::{self, Pvf3};
use pxl_spec::space::pyramid::{LevelPage, GROUND};

/// The phone the experiment is set on: an iPhone 13 mini's screen in
/// points, drawn at the two ratios it names.
const SCREEN_PT: [f64; 2] = [375.0, 812.0];
const RATIOS: [f64; 2] = [2.0, 3.0];
/// The camera's vertical field of view (pxl-spec space/lod.rs
/// `REFERENCE_FOV_DEG`).
const FOV_DEG: f64 = 50.0;
/// One ray every this many points across and down: the steps' spread, not
/// the frame's sum, is what is measured, and it is the same at any stride
/// fine against the ground's cells.
const STRIDE_PT: f64 = 3.0;
/// How far a ray is asked, in units: past the reach any view draws to.
const REACH: f64 = 40_000.0;
/// Rounds of cast and fetch before what is still lacked is taken as what
/// cannot be had.
const ROUNDS: usize = 16;

/// A view: how high the eye stands over the ground under it, in units, and
/// how far it looks down, in degrees.
struct View {
    name: &'static str,
    height: f64,
    pitch_deg: f64,
}

const VIEWS: [View; 3] = [
    View {
        name: "from above",
        height: 1_500.0,
        pitch_deg: -89.9,
    },
    View {
        name: "at 45°",
        height: 400.0,
        pitch_deg: -45.0,
    },
    View {
        name: "street level",
        height: 1.0,
        pitch_deg: -5.0,
    },
];

/// Gives up no frame: what does not fit is refused, as residency would.
struct Keep;

impl Victim for Keep {
    fn victim(&self, _: &[Candidate]) -> Option<usize> {
        None
    }
}

/// The pages a run has asked the server for and what came of it.
struct Pages {
    server: String,
    paging: Paging<Terrain>,
    tried: HashSet<LevelPage>,
}

impl Pages {
    /// `p`, fetched and held, once; false where it cannot be had.
    fn fetch(&mut self, p: LevelPage) -> bool {
        if !self.tried.insert(p) {
            return false;
        }
        let url = format!("{}/tile/terrain/{}", self.server, page_name(p));
        let Ok(out) = Command::new("sh").arg("-c").arg(format!("curl -sf '{url}' | zstd -dq")).output() else {
            return false;
        };
        if !out.status.success() || out.stdout.is_empty() {
            return false;
        }
        let Ok(page) = decode_terrain(&out.stdout) else { return false };
        self.paging.admit(p, &Keep, |_| Ok::<_, ()>(page)).is_ok()
    }
}

fn point(x: f64, y: f64) -> Pvf3 {
    let mut at = Pvf3::default();
    pxl_spec::space::pvf3::projection::project(&mut at, x, y, 0.0);
    at
}

/// The rays of a view through `eye`, one every `STRIDE_PT` points.
fn rays(eye: [f64; 3], pitch_deg: f64) -> Vec<Ray> {
    let p = pitch_deg.to_radians();
    let forward = [0.0, p.cos(), p.sin()];
    let right = [1.0, 0.0, 0.0];
    let up = [0.0, -p.sin(), p.cos()];
    let half = (FOV_DEG.to_radians() / 2.0).tan();
    let aspect = SCREEN_PT[0] / SCREEN_PT[1];
    let (w, h) = ((SCREEN_PT[0] / STRIDE_PT) as usize, (SCREEN_PT[1] / STRIDE_PT) as usize);
    let mut out = Vec::with_capacity(w * h);
    for j in 0..h {
        for i in 0..w {
            let x = (2.0 * (i as f64 + 0.5) / w as f64 - 1.0) * half * aspect;
            let y = (1.0 - 2.0 * (j as f64 + 0.5) / h as f64) * half;
            let d: [f64; 3] = std::array::from_fn(|k| forward[k] + x * right[k] + y * up[k]);
            let n = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
            out.push(Ray {
                origin: eye,
                direction: d.map(|v| v / n),
                reach: REACH,
            });
        }
    }
    out
}

/// What one view's rays came to.
struct Tally {
    steps: Vec<u64>,
    exact: usize,
    coarser: usize,
    nothing: usize,
    hit: usize,
}

fn cast(pages: &Pages, store: &Store, rays: &[Ray]) -> (Tally, Vec<LevelPage>) {
    let mut t = Tally {
        steps: Vec::with_capacity(rays.len()),
        exact: 0,
        coarser: 0,
        nothing: 0,
        hit: 0,
    };
    let mut lacked = Vec::new();
    for ray in rays {
        let visits = Cell::new(0);
        let answer = Access::new(&pages.paging, store).counting(&visits).first_hit(ray, Detail::Distance(Tau::FINEST));
        t.steps.push(visits.get());
        match answer {
            Answer::Exact(h) => {
                t.exact += 1;
                t.hit += usize::from(h.is_some());
            }
            Answer::Coarser(h, lack) => {
                t.coarser += 1;
                t.hit += usize::from(h.is_some());
                lacked.push(lack.page);
            }
            Answer::Nothing(lack) => {
                t.nothing += 1;
                lacked.push(lack.page);
            }
        }
    }
    (t, lacked)
}

/// `steps`, in the rays' order — row by row from the screen's top — as an
/// image of the screen at one pixel a ray, the brightest the longest.
fn heatmap(name: &str, steps: &[u64]) {
    let (w, h) = ((SCREEN_PT[0] / STRIDE_PT) as usize, (SCREEN_PT[1] / STRIDE_PT) as usize);
    let most = steps.iter().copied().max().unwrap_or(1).max(1) as f64;
    let mut out = format!("P6\n{w} {h}\n255\n").into_bytes();
    for &n in steps {
        // Black to blue to green to white, as the share of the longest grows.
        let v = n as f64 / most;
        let rgb = [(v * 3.0 - 2.0).clamp(0.0, 1.0), (v * 3.0 - 1.0).clamp(0.0, 1.0), (v * 3.0).clamp(0.0, 1.0)];
        out.extend(rgb.map(|c| (c * 255.0) as u8));
    }
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../target/ray_steps");
    std::fs::create_dir_all(&dir).expect("the heatmaps' directory");
    let file = dir.join(format!("{}.ppm", name.replace([' ', '°'], "_")));
    std::fs::write(&file, out).expect("a heatmap");
    println!("  heatmap: {}", file.display());
}

fn percentile(sorted: &[u64], q: f64) -> u64 {
    sorted[((sorted.len() - 1) as f64 * q).round() as usize]
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    // Namsan in Seoul: a hill over a city, where a ray along the ground
    // grazes slopes and roofs both.
    let (lon, lat) = match args.as_slice() {
        [lon, lat, ..] => (lon.parse().expect("lon"), lat.parse().expect("lat")),
        _ => (126.9882, 37.5512),
    };
    let server = args.get(2).cloned().unwrap_or_else(|| "http://localhost:3001".into());
    let mut at = Pvf3::default();
    epsg4326::project(&mut at, &[lon, lat, 0.0]);
    let (x, y) = (pvf3::to_real(pvf3::X(&at)), pvf3::to_real(pvf3::Y(&at)));

    let store = Store::new();
    let mut pages = Pages {
        server,
        paging: Paging::default(),
        tried: HashSet::new(),
    };
    // The eye's own column first, so it can stand on the ground.
    let leaf = GROUND.cell_of(FINEST, &point(x, y)).page();
    let mut p = leaf;
    loop {
        pages.fetch(p);
        if p.level == pxl_spec::space::pyramid::TOP_LEVEL {
            break;
        }
        p = p.parent();
    }
    let ground = Access::new(&pages.paging, &store).at(&point(x, y), FINEST).value().map_or(0.0, |g| g.top);
    println!("at {lon}, {lat}: ground {ground:.1} u, pages from {}", pages.server);

    for view in VIEWS {
        let eye = [x, y, ground + view.height];
        let rays = rays(eye, view.pitch_deg);
        let mut tally = None;
        for _ in 0..ROUNDS {
            let (t, lacked) = cast(&pages, &store, &rays);
            tally = Some(t);
            let fetched = lacked.into_iter().collect::<HashSet<_>>().into_iter().filter(|&p| pages.fetch(p)).count();
            if fetched == 0 {
                break;
            }
        }
        let mut t = tally.expect("one round at least");
        heatmap(view.name, &t.steps);
        t.steps.sort_unstable();
        let n = t.steps.len();
        let mean = t.steps.iter().sum::<u64>() as f64 / n as f64;
        println!();
        println!("{} — eye {} u up, looking {}°: {n} rays", view.name, view.height, view.pitch_deg);
        println!(
            "  steps a ray: mean {mean:.1}, p50 {}, p90 {}, p99 {}, max {}",
            percentile(&t.steps, 0.5),
            percentile(&t.steps, 0.9),
            percentile(&t.steps, 0.99),
            t.steps[n - 1]
        );
        println!(
            "  answered: exact {:.0}%, coarser {:.0}%, nothing {:.0}%; met something {:.0}%",
            100.0 * t.exact as f64 / n as f64,
            100.0 * t.coarser as f64 / n as f64,
            100.0 * t.nothing as f64 / n as f64,
            100.0 * t.hit as f64 / n as f64
        );
        for ratio in RATIOS {
            let pixels = SCREEN_PT[0] * SCREEN_PT[1] * ratio * ratio;
            println!("  at ratio {ratio}: {:.1} M pixels, {:.0} M steps a frame", pixels / 1e6, pixels * mean / 1e6);
        }
    }
    println!();
    println!("{} pages held of {} asked", pages.paging.held().count(), pages.tried.len());
}
