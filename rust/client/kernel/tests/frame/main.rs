//! Whole frames of the kernel, run as a device runs them: the kernel linked as its native
//! guest (lib.rs `guest!`, xpute-gpu's `native_guest!`), driven through its rings as the
//! app's shell drives it (ts/client/src/port/), a frame's turn a refresh after the last on
//! the guest's clock however long the machine takes (pace.rs), its world surface drawn
//! offscreen on a wgpu device. A pass by hand binds its own groups and reads one usage
//! scope; only the renderer's frames bind what the frame binds, so only they meet what WebGPU
//! refuses of a whole frame.
//!
//! The frames fail on any error WebGPU's validation raises, which a scope on the guest's
//! thread holds turn by turn (gpu.rs); on the kernel's stop, a broken invariant's report
//! (the encode rule's ENOTRECOVERABLE in pxl-render gpu/api.rs among them) or a trap; on
//! owed work that has not come to rest within `REST_FRAMES` idle turns of a view left
//! standing; and on any wait that outlasts the harness's own clock (watch.rs).

mod fs;
mod gpu;
mod host;
mod pace;
mod spec;
mod watch;

use host::Host;
use pxl_engine::beam::ROLLS;
use pxl_engine::screen::JITTERS;
use pxl_engine::SHARD_ZOOM;
use pxl_render::api_spec::device::FRAME_LATENCY;
use pxl_spec::bake::publication::{BaseLayer, FORMAT};
use pxl_spec::space::coord::adapter::epsg4326;
use pxl_spec::space::coord::q3;
use pxl_spec::space::layer::quadrant::{quad_rows, Quadrant, TOP_ZOOM};
use spec::V;
use xpute_host::native::{pause, Guest};

/// The display the frames are paced to, 60 Hz.
const REFRESH_MS: f64 = 1000.0 / 60.0;

/// What the client allows out at once (ts/client/src/services/origin.ts `credits`).
const CREDITS: f64 = 6.0;

/// The surface's size in pixels, a phone's at a device pixel ratio of 1.
const SIZE: [f64; 2] = [390.0, 844.0];

/// The wall clock the frames start at: noon at the place, 2026-06-21T03:00Z, so the sun
/// stands high and the shadows cast.
const NOON_MS: f64 = 1_782_010_800_000.0;

/// Where the camera goes: Seoul.
const PLACE: [f64; 2] = [126.978, 37.566];

/// Idle turns a view left standing may ask for before its owed work rests: the rolled
/// lattice's casts, one a frame (`beam::ROLLS`), then the jitter's sequence over each pixel
/// (`screen::JITTERS`), each readback landing `FRAME_LATENCY` frames after the frame that
/// copied it; twice that, for the owed queue's runs a frame's budget leaves to the next.
const REST_FRAMES: u32 = 2 * (ROLLS + JITTERS + FRAME_LATENCY);

/// Frames the renderer's pipelines are given to be made in before the world is drawn.
const PIPELINE_FRAMES: u32 = 600;

/// The shard the place lies in, the one the source serves: found from the top down, each
/// zoom's tile the one whose square holds it.
fn shard_of([lon, lat]: [f64; 2]) -> Quadrant {
    let q = epsg4326::project(&[lon, lat, 0.0]);
    let [x, y] = [q[0], q[1]].map(|v| q3::to_real(i64::from(v)));
    let holds = |t: &Quadrant| {
        let (sx, sy, side) = t.rect();
        (sx..sx + side).contains(&x) && (sy..sy + side).contains(&y)
    };
    let n = 1i32 << TOP_ZOOM;
    let mut t = quad_rows(TOP_ZOOM)
        .flat_map(|r| (0..n).map(move |c| Quadrant::new(TOP_ZOOM, [c, r])))
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
    host: Host,
    guest: Guest,
    clock_ms: f64,
    frames: u32,
}

impl Run {
    /// A turn granted as the strobe grants one (ts/client/src/port/surface.ts
    /// `submit_grant`), then held to what the frames must keep: whether the kernel wants
    /// the next frame at once.
    fn frame(&mut self, interactive: bool) -> bool {
        pace::frame();
        self.clock_ms += REFRESH_MS;
        self.frames += 1;
        self.host.send(
            "TURN_GRANT",
            &[
                ("refresh_ms", V::Num(REFRESH_MS)),
                ("paced_ms", V::Num(REFRESH_MS)),
                ("interactive", V::Bool(interactive)),
                ("credits", V::Num(CREDITS)),
                ("unix_ms", V::Num(self.clock_ms)),
            ],
        );
        watch::waiting(format!("frame {}: its turn", self.frames));
        let wake = self.host.turn(REFRESH_MS);
        self.check(wake.is_some());
        wake == Some(0.0)
    }

    fn check(&mut self, ran: bool) {
        let frame = self.frames;
        watch::waiting(format!("frame {frame}: the device's work"));
        let errors = gpu::take(self.guest);
        assert!(
            errors.is_empty(),
            "frame {frame}: the device refused what the frame encoded:\n{}",
            errors.join("\n")
        );
        if let Some(report) = &self.host.abort {
            panic!("frame {frame}: the kernel stopped: {report}");
        }
        assert!(ran, "frame {frame}: the kernel trapped");
        assert!(
            self.host.refused.is_empty(),
            "frame {frame}: the kernel refused {:?}",
            self.host.refused
        );
    }

    /// A call of the shell's, checked as a frame is.
    fn call(&mut self, name: &str, values: &[(&str, V)]) -> i32 {
        watch::waiting(format!("frame {}: {name}'s answer", self.frames));
        let r = self.host.call(name, values);
        self.check(true);
        assert!(r >= 0, "{name} answered {r}");
        r
    }

    /// `n` frames, nothing sent.
    fn frames(&mut self, n: u32) {
        for _ in 0..n {
            self.frame(false);
        }
    }

    /// Idle turns, nothing sent, the clock at the display's pace: how many until the kernel
    /// asks for the next frame at once no more, its owed work done, a readback's landing
    /// among it; it asks none at once over a jitter's sequence of turns past the bound after.
    /// What moves on its own (the companion standing in view) steps on and wakes it at its own
    /// steps, later than that.
    fn rest(&mut self) -> u32 {
        let mut asked = 0;
        for n in 1..=REST_FRAMES + JITTERS {
            if self.frame(false) {
                asked = n;
            }
        }
        assert!(
            asked < REST_FRAMES,
            "frame {}: the owed work did not rest within {REST_FRAMES} idle turns, the last \
             asked {asked} turns in",
            self.frames
        );
        asked + 1
    }

    fn pointer(&mut self, phase: &str, x: f64, y: f64) {
        let shell = &self.host.shell;
        let values = [
            ("phase", V::Num(shell.member("PointerPhase", phase).into())),
            (
                "pointer_type",
                V::Num(shell.member("PointerType", "Mouse").into()),
            ),
            ("id", V::Num(1.0)),
            // The secondary button rotates (domain/control/camera/input.rs).
            ("button", V::Num(2.0)),
            ("x", V::Num(x)),
            ("y", V::Num(y)),
        ];
        self.host.send("INPUT_POINTER", &values);
        self.frame(true);
    }
}

fn main() {
    watch::start();
    frames();
}

fn frames() {
    let guest = pxl_kernel::guest();
    // Every turn is one this test rings, none the guest's own thread runs, on the display's
    // clock.
    pause();
    pace::install(guest);
    let host = Host::lay(guest);
    watch::waiting("the device's opening".to_string());
    if gpu::open(guest).is_none() {
        eprintln!("no adapter answers: the frames are not run");
        return;
    }
    let mut run = Run {
        host,
        guest,
        clock_ms: NOON_MS,
        frames: 0,
    };
    // The boot as abi/boot.ts orders it: the host's attach, what the platform is, the GPU.
    run.host.send_xpute("SYS_ATTACH");
    run.host.send(
        "SENSOR_PLATFORM",
        &[
            ("geolocation", V::Bool(false)),
            ("orient", V::Bool(false)),
            ("orient_permission", V::Bool(false)),
            ("orient_silence_is_loss", V::Bool(false)),
        ],
    );
    run.host
        .send("SETTING_DEVELOPMENT", &[("development", V::Bool(true))]);
    run.host.send_xpute("GPU_READY");
    run.frame(false);
    // The tile source as source.ts declares it: the place's shard alone, every tile of it
    // a page its version leaves out (fs.rs), so the world there is the sea's.
    let format = BaseLayer::ALL.map(|l| FORMAT.of(l)).to_vec();
    run.call(
        "FS_MOUNT",
        &[
            ("top_zoom", V::Num(TOP_ZOOM.into())),
            ("format", V::Nums(format)),
            ("shard", V::Strs(vec![shard_of(PLACE).key()])),
        ],
    );

    // The world's surface, as the scene's place opens it (port/surface.ts, port/scene.ts).
    let surface = run.call("SURFACE_OPEN", &[]);
    assert!(surface > 0, "no surface was opened");
    let surface = V::Num(f64::from(surface));
    run.host.send(
        "SURFACE_RESIZE",
        &[
            ("surface", surface.clone()),
            ("width", V::Num(SIZE[0])),
            ("height", V::Num(SIZE[1])),
            ("device_pixel_ratio", V::Num(1.0)),
        ],
    );
    let world = run.host.shell.member("SceneContent", "World");
    run.call(
        "SCENE_DRAW",
        &[("surface", surface), ("content", V::Num(world.into()))],
    );
    let mut made = 0;
    while run.host.shown.renderer_progress < 1.0 {
        made += 1;
        assert!(
            made <= PIPELINE_FRAMES,
            "the renderer's pipelines were not made in {PIPELINE_FRAMES} frames"
        );
        run.frame(false);
    }

    // The camera sent to the place, and the cast its view asks for: the rolled lattice's
    // casts and the jitter's sequence over them.
    run.call(
        "CAMERA_GOTO",
        &[("lon", V::Num(PLACE[0])), ("lat", V::Num(PLACE[1]))],
    );
    run.frames(ROLLS + JITTERS);

    // Frames held: the view stands, the turns keep coming.
    run.frames(JITTERS);

    // A turn: the secondary button dragged across the view.
    let [cx, cy] = SIZE.map(|s| s / 2.0);
    run.pointer("Down", cx, cy);
    for k in 1..=8 {
        run.pointer("Move", cx + 12.0 * f64::from(k), cy);
    }
    run.pointer("Up", cx + 96.0, cy);
    run.frames(JITTERS);

    // A zoom step.
    run.host.send("INPUT_WHEEL", &[("delta_y", V::Num(100.0))]);
    run.frame(true);
    run.frames(JITTERS);

    // The shadows on, and the light's cast they ask for.
    run.call(
        "SETTING_SET",
        &[
            ("key", V::Num(spec::setting("shadows").into())),
            ("value", V::Num(1.0)),
        ],
    );
    run.frames(ROLLS + JITTERS);

    // Idle turns until the owed work rests.
    let rested = run.rest();
    eprintln!(
        "{} frames: the pipelines made in {made}, the owed work at rest {rested} idle turns \
         after the last input",
        run.frames
    );
}
