//! Runs whole frames of the core the way a device runs them. The core is linked as its
//! native core (lib.rs `core!`, xpute-gpu's `native_core!`) and driven through its rings
//! the way the app's UI drives it (ts/client/src/service/core/). Each frame's turn comes one
//! refresh after the last on the core's clock, however long the machine takes (pace.rs),
//! and the world's surface is drawn offscreen on a wgpu device. A test that encodes a pass
//! by hand binds its own groups and sees one usage scope. Only the renderer's real frames
//! bind everything a frame binds, so only they meet what WebGPU refuses of a whole frame.
//!
//! The run fails on any error WebGPU's validation raises, which a scope where the stream
//! runs collects turn by turn (gpu.rs). It also fails when the core stops, on a broken
//! invariant's report (among them the encode rule's ENOTRECOVERABLE in pxl-render
//! gpu/api.rs) or on a trap; when a view left standing does not come to rest within
//! `REST_FRAMES` idle turns; and when any wait outlasts the harness's own clock (watch.rs).

mod fs;
mod gpu;
mod pace;
mod runtime;
mod spec;
mod watch;

use pxl_engine::ray::holes_words;
use pxl_render::api_spec::device::FRAME_LATENCY;
use pxl_render::frame::shader::word::screen::JITTERS;
use pxl_render::gpu::surface::Surface;
use pxl_spec::bake::publication::{BaseLayer, FORMAT};
use pxl_spec::space::coord::adapter::epsg4326;
use pxl_spec::space::coord::q3;
use pxl_spec::space::coord::zoom::{MIN_ZOOM, SHARD_ZOOM};
use pxl_spec::space::layer::quadrant::{quad_rows, Quadrant};
use runtime::Runtime;
use spec::V;
use xpute_runtime::native::pause;

/// The refresh interval of the display the frames are paced to, 60 Hz.
const REFRESH_MS: f64 = 1000.0 / 60.0;

/// The surface's size in pixels, a phone's at a device pixel ratio of 1.
const SIZE: [f64; 2] = [390.0, 844.0];

/// The surface's size for the last frames, an 8K display's at a device pixel ratio of 1.
const LARGE: [u32; 2] = [7680, 4320];

/// The wall clock the frames start at: noon at the place, 2026-06-21T03:00Z, so the sun
/// stands high and casts shadows.
const NOON_MS: f64 = 1_782_010_800_000.0;

/// Where the camera goes: Seoul.
const PLACE: [f64; 2] = [126.978, 37.566];

/// How many idle turns a standing view may ask for before it rests: one jitter sequence over
/// each pixel (`screen::JITTERS`), plus the `FRAME_LATENCY` frames a readback takes to land
/// after the frame that copied it.
const REST_FRAMES: u32 = JITTERS + FRAME_LATENCY;

/// How long the renderer's pipelines, compiled apart from the frames, are given to be made before
/// the world is drawn.
const PIPELINE_TIME: std::time::Duration = std::time::Duration::from_secs(300);

/// How many frames a zoom's ease is given to end, about twice the fifty or so a step takes.
const EASE_FRAMES: u32 = 120;

/// The shard the place lies in, which is the one shard the test's source serves. It is found
/// from the top zoom down, taking at each zoom the tile whose square holds the place.
fn shard_of([lon, lat]: [f64; 2]) -> Quadrant {
    let q = epsg4326::project(&[lon, lat, 0.0]);
    let [x, y] = [q[0], q[1]].map(|v| q3::to_real(i64::from(v)));
    let holds = |t: &Quadrant| {
        let (sx, sy, side) = t.rect();
        (sx..sx + side).contains(&x) && (sy..sy + side).contains(&y)
    };
    let n = 1i32 << MIN_ZOOM;
    let mut t = quad_rows(MIN_ZOOM)
        .flat_map(|r| (0..n).map(move |c| Quadrant::new(MIN_ZOOM, [c, r])))
        .find(holds)
        .expect("a top tile holds the place");
    while t.zoom < SHARD_ZOOM {
        t = t
            .children()
            .into_iter()
            .find(holds)
            .expect("a child holds the place");
    }
    t
}

struct Run {
    runtime: Runtime,
    clock_ms: f64,
    frame_cnt: u32,
}

impl Run {
    /// Grants a turn the way the runtime does (@xpute/runtime's SYS_FRAME, then its
    /// SYS_TURN) and checks what every frame must keep. Returns whether
    /// the core wants the next frame at once.
    fn frame(&mut self, interactive: bool) -> bool {
        pace::frame();
        self.clock_ms += REFRESH_MS;
        self.frame_cnt += 1;
        self.runtime.send(
            "SYS_FRAME",
            &[
                ("refresh_ms", V::Num(REFRESH_MS)),
                ("paced_ms", V::Num(REFRESH_MS)),
                ("interactive", V::Bool(interactive)),
            ],
        );
        self.runtime
            .send("SYS_TURN", &[("unix_ms", V::Num(self.clock_ms))]);
        watch::waiting(format!("frame {}: its turn", self.frame_cnt));
        let wake = self.runtime.turn(REFRESH_MS);
        self.check(wake.is_some());
        wake == Some(0.0)
    }

    fn check(&mut self, ran: bool) {
        let frame = self.frame_cnt;
        watch::waiting(format!("frame {frame}: the device's work"));
        let errs = gpu::take();
        assert!(
            errs.is_empty(),
            "frame {frame}: the device refused what the frame encoded:\n{}",
            errs.join("\n")
        );
        if let Some(report) = &self.runtime.abort {
            panic!("frame {frame}: the core stopped: {report}");
        }
        assert!(ran, "frame {frame}: the core trapped");
        assert!(
            self.runtime.refused.is_empty(),
            "frame {frame}: the core refused {:?}",
            self.runtime.refused
        );
    }

    /// Makes one of the UI's calls and checks it as a frame is checked.
    fn call(&mut self, name: &str, vals: &[(&str, V)]) -> i32 {
        watch::waiting(format!("frame {}: {name}'s answer", self.frame_cnt));
        let r = self.runtime.call(name, vals);
        self.check(true);
        assert!(r >= 0, "{name} answered {r}");
        r
    }

    /// Runs `n` frames with no input.
    fn frames(&mut self, n: u32) {
        for _ in 0..n {
            self.frame(false);
        }
    }

    /// Runs frames until the zoom's ease ends. The zoom was the camera's last input, so the
    /// view stands still from there.
    fn settle(&mut self) {
        let mut eased = 0;
        while self.runtime.shown.zooming {
            eased += 1;
            assert!(
                eased <= EASE_FRAMES,
                "frame {}: the zoom did not end in {EASE_FRAMES} frames",
                self.frame_cnt
            );
            self.frame(false);
        }
    }

    /// Runs idle turns with no input, the clock at the display's pace, and returns how many
    /// passed before the core stopped asking for the next frame at once. A readback's landing
    /// is among what the core waits on. The core must then ask for no immediate frame through
    /// a further jitter sequence of turns. What moves on its own, such as the characters
    /// standing in view, wakes the core at its own steps, which come later than that.
    fn rest(&mut self) -> u32 {
        let mut asked = 0;
        for n in 1..=REST_FRAMES + JITTERS {
            if self.frame(false) {
                asked = n;
            }
        }
        assert!(
            asked < REST_FRAMES,
            "frame {}: the view did not rest within {REST_FRAMES} idle turns, the last asked \
             {asked} turns in",
            self.frame_cnt
        );
        asked + 1
    }

    fn pointer(&mut self, phase: &str, x: f64, y: f64) {
        let ui = &self.runtime.ui;
        let vals = [
            ("phase", V::Num(ui.member("PointerPhase", phase).into())),
            (
                "pointer_type",
                V::Num(ui.member("PointerType", "Mouse").into()),
            ),
            ("id", V::Num(1.0)),
            // The secondary button rotates (domain/control/input.rs).
            ("button", V::Num(2.0)),
            ("x", V::Num(x)),
            ("y", V::Num(y)),
        ];
        self.runtime.send("INPUT_POINTER", &vals);
        self.frame(true);
    }
}

fn main() {
    watch::start();
    frames();
}

fn frames() {
    let core = pxl_core::core();
    // The test rings every turn itself, on the display's clock, and the core's own thread
    // runs none.
    pause();
    pace::install(core);
    let runtime = Runtime::lay(core);
    watch::waiting("the device's opening".to_string());
    if gpu::open().is_none() {
        eprintln!("no adapter answers: the frames are not run");
        return;
    }
    let mut run = Run {
        runtime,
        clock_ms: NOON_MS,
        frame_cnt: 0,
    };
    // The boot in the order runtime/boot.ts gives it: the runtime attaches, the platform is
    // described, and the GPU is ready.
    run.runtime.send("SYS_ATTACH", &[]);
    run.runtime.send(
        "SENSOR_PLATFORM",
        &[
            ("geolocation", V::Bool(false)),
            ("orient", V::Bool(false)),
            ("orient_permission", V::Bool(false)),
            ("orient_silence_is_loss", V::Bool(false)),
        ],
    );
    run.runtime.send("SETTING_DEV", &[("dev", V::Bool(true))]);
    run.runtime.send("GPU_READY", &[]);
    run.frame(false);
    // The tile source, declared as service/core/source.ts declares it, with the place's shard
    // alone. Every tile of the shard is a page its version leaves out (fs.rs), so the world
    // there is sea.
    let format = BaseLayer::ALL.map(|l| FORMAT.of(l)).to_vec();
    run.call(
        "SOURCE_DECLARE",
        &[
            ("min_zoom", V::Num(MIN_ZOOM.into())),
            ("format", V::Nums(format)),
            ("shard", V::Strs(vec![shard_of(PLACE).key()])),
        ],
    );

    // The world's surface, opened the way the scene's place opens it (service/core/surface.ts,
    // service/core/scene.ts).
    let surface = run.call("GPU_SURFACE_OPEN", &[]);
    assert!(surface > 0, "no surface was opened");
    let surface = V::Num(f64::from(surface));
    let resize = |run: &mut Run, [w, h]: [f64; 2]| {
        run.runtime.send(
            "GPU_SURFACE_RESIZE",
            &[
                ("surface", surface.clone()),
                ("width", V::Num(w)),
                ("height", V::Num(h)),
                ("device_pixel_ratio", V::Num(1.0)),
            ],
        );
    };
    resize(&mut run, SIZE);
    let world = run.runtime.ui.member("SceneContent", "World");
    run.call(
        "SCENE_DRAW",
        &[
            ("surface", surface.clone()),
            ("content", V::Num(world.into())),
        ],
    );
    let mut made = 0;
    let began = std::time::Instant::now();
    while run.runtime.shown.renderer_progress < 1.0 {
        made += 1;
        assert!(
            began.elapsed() < PIPELINE_TIME,
            "the renderer's pipelines were not made in {PIPELINE_TIME:?}"
        );
        run.frame(false);
        std::thread::sleep(std::time::Duration::from_millis(2));
    }

    // The camera goes to the place, and its view asks for one jitter sequence of casts.
    run.call(
        "CAMERA_GOTO",
        &[("lon", V::Num(PLACE[0])), ("lat", V::Num(PLACE[1]))],
    );
    run.frames(JITTERS);

    // The view stands still while the turns keep coming.
    run.frames(JITTERS);

    // A rotation: the secondary button dragged across the view.
    let [cx, cy] = SIZE.map(|s| s / 2.0);
    run.pointer("Down", cx, cy);
    for k in 1..=8 {
        run.pointer("Move", cx + 12.0 * f64::from(k), cy);
    }
    run.pointer("Up", cx + 96.0, cy);
    run.frames(JITTERS);

    // A zoom step.
    run.runtime
        .send("INPUT_WHEEL", &[("delta_y", V::Num(100.0))]);
    run.frame(true);
    run.frames(JITTERS);

    // The shadows go on, which asks for the light's cast.
    run.call(
        "SETTING_SET",
        &[
            ("key", V::Num(spec::setting("shadows").into())),
            ("val", V::Num(1.0)),
        ],
    );
    run.frames(JITTERS);

    // The camera finishes its ease, then idle turns run until the view rests.
    run.settle();
    let rested = run.rest();
    eprintln!(
        "{} frames: the pipelines made in {made}, the view at rest {rested} idle turns after \
         the camera stood",
        run.frame_cnt
    );

    // The surface grows to an 8K display's, the view is cast again over it, and it rests.
    resize(&mut run, LARGE.map(f64::from));
    run.frames(JITTERS);
    let rested = run.rest();
    // The largest size the resolution rule draws the surface at, at full ratio (pxl-render
    // gpu/surface.rs `render_px`), and the bytes the eye's holes take there (pxl-engine
    // `ray::holes_words`).
    let whole = Surface {
        px_w: LARGE[0],
        px_h: LARGE[1],
        dpr: 1.0,
        ratio: 1.0,
        ..Surface::default()
    };
    let (w, h) = whole.render_px();
    let bytes = 4 * u64::from(holes_words([w, h]));
    eprintln!(
        "{LARGE:?}: drawn whole at {w}x{h}, the eye's holes {bytes} bytes, {:.3} bytes a drawn \
         pixel; the view at rest {rested} idle turns after",
        bytes as f64 / f64::from(w * h),
    );
}
