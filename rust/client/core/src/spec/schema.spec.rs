// pxl-core/spec/schema.spec.rs
//
// GENERATED from spec/client/core/domain/setting/schema.json5 (xpute-gen data.rs)
//
// Every setting the app has, declared once: its default, its kind, and
// where it is shown, its `section`: one of the settings sheet's, or
// `perf`, the profiler panel's own. The store reads the defaults from
// here and the sheet renders rows from here, so adding a setting to the
// sheet is one entry in SETTING plus its two strings in the locale catalog
// under `settings.<section>.<key>`. A choice may carry `costs` beside its
// `options`, one per option in the same order, for when what an option costs
// differs from its neighbors'; an empty string is no badge. A `dev` setting
// is in the core's table in every build and offered only under a
// development one; the UI's production bundle carries SETTING_PROD,
// the table without them, in SETTING's place. A setting whose `stored` is
// false is the session's alone: the store neither writes it to its kept
// slot nor reads it from there, so every session starts it at its default.
//
// A setting carries its `number` (the core's SettingKey), and a choice's
// `options` names one of the lists below, which numbers each option. The
// numbers are what crosses and what storage keeps; the names are the
// UI's alone, for its rows and the locale. A number is given out once;
// the generator refuses one given twice. An `index` is a place in a list
// kept elsewhere — the person and the pet are by theirs in character.json5 — and
// crosses as itself.

/// Which controller makes the camera's pose.
///
///   orbit        — third person: the camera turns about the pivot, its
///                  radius ahead of the eye, and a drag moves the pivot.
///   first_person — the eye is the pivot and turns in place; a drag
///                  walks it as the orbit's pivot walks, the eye at its
///                  height over where the walk stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum CameraMode {
    Orbit = 0,
    FirstPerson = 1,
}

impl CameraMode {
    /// Every member, in its source's own order.
    pub const ALL: [CameraMode; 2] = [CameraMode::Orbit, CameraMode::FirstPerson];

    /// The member `val` is, or none for a number no member has.
    pub const fn of(val: u32) -> Option<CameraMode> {
        match val {
            0 => Some(CameraMode::Orbit),
            1 => Some(CameraMode::FirstPerson),
            _ => None,
        }
    }

    /// The member's name, as its source spells it.
    pub const fn name(self) -> &'static str {
        match self {
            CameraMode::Orbit => "orbit",
            CameraMode::FirstPerson => "first_person",
        }
    }
}

/// What terrain does to the camera.
///
///   through — nothing. The eye goes wherever the controls put it, ground
///             included, where its rays start inside what they meet and
///             the scene reads inside out.
///   stop    — a control goes as far as it asked or as far as it can.
///             Nothing moves on its own; a zoom into a hill stops short.
///   climb   — dist is granted and the *view* pitches up instead, as far
///             as straight down, at the cost of an angle nobody asked for.
///             Where its way hits something all the same, a zoom in goes
///             through to the pivot's side and any other control stops.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum OrbitObstacleMode {
    Through = 0,
    Stop = 1,
    Climb = 2,
}

impl OrbitObstacleMode {
    /// Every member, in its source's own order.
    pub const ALL: [OrbitObstacleMode; 3] = [
        OrbitObstacleMode::Through,
        OrbitObstacleMode::Stop,
        OrbitObstacleMode::Climb,
    ];

    /// The member `val` is, or none for a number no member has.
    pub const fn of(val: u32) -> Option<OrbitObstacleMode> {
        match val {
            0 => Some(OrbitObstacleMode::Through),
            1 => Some(OrbitObstacleMode::Stop),
            2 => Some(OrbitObstacleMode::Climb),
            _ => None,
        }
    }

    /// The member's name, as its source spells it.
    pub const fn name(self) -> &'static str {
        match self {
            OrbitObstacleMode::Through => "through",
            OrbitObstacleMode::Stop => "stop",
            OrbitObstacleMode::Climb => "climb",
        }
    }
}

/// Where the ambient occlusion comes from. Either is the sky's visibility
/// and darkens only the light from the sky and its bounce, never the direct
/// sun or moon, which the shadows are its visibility of; neither tints
/// (rust/client/render/src/frame/shader/atmos_light.wgsl `surface_light_in`).
///
///   off    — none.
///   corner — the screen pass's own: the floor along a cube and the inside
///            of a corner, where a neighboring pixel's face rises in front
///            of the pixel's own plane within a cell or two of it
///            (rust/client/render/src/frame/shader/screen_ray.wgsl).
///            A few taps of the rays a pixel, behind a uniform.
///   screen — N8AO
///            (rust/client/render/src/pass/ao.rs): the world's depth drawn
///            once more, the occlusion sampled from it at half resolution,
///            denoised, and read by every lit surface. Five passes and 11
///            bytes a pixel. The corners' too, the darker of the two where
///            both fall (screen_shade.wgsl), as N8AO reads a cube's corner
///            softer at half resolution.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum AoMode {
    Off = 0,
    Corner = 1,
    Screen = 2,
}

impl AoMode {
    /// Every member, in its source's own order.
    pub const ALL: [AoMode; 3] = [AoMode::Off, AoMode::Corner, AoMode::Screen];

    /// The member `val` is, or none for a number no member has.
    pub const fn of(val: u32) -> Option<AoMode> {
        match val {
            0 => Some(AoMode::Off),
            1 => Some(AoMode::Corner),
            2 => Some(AoMode::Screen),
            _ => None,
        }
    }

    /// The member's name, as its source spells it.
    pub const fn name(self) -> &'static str {
        match self {
            AoMode::Off => "off",
            AoMode::Corner => "corner",
            AoMode::Screen => "screen",
        }
    }
}

/// How many device pixels the world is drawn at to a CSS pixel, and the rate
/// frames go at.
///
///   auto  — a frame each whole number of vsync intervals, from RATE_MAX_HZ
///           to RATE_MIN_HZ;
///           the display's pixels, stepped down to a share of them, while
///           moving frames' GPU time runs over their budget — the frame's
///           target interval — and back up once the step above would fit, a
///           budget control as congestion control is
///           (rust/client/core/src/domain/scene/resolution.rs).
///   fixed — the display's, at its rate to RATE_MAX_HZ, whatever the frames
///           cost.
///   low   — half the display's each way, auto's floor (RES_MIN), at the
///           display's rate to RATE_MAX_HZ.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum ResMode {
    Auto = 0,
    Fixed = 1,
    Low = 2,
}

impl ResMode {
    /// Every member, in its source's own order.
    pub const ALL: [ResMode; 3] = [ResMode::Auto, ResMode::Fixed, ResMode::Low];

    /// The member `val` is, or none for a number no member has.
    pub const fn of(val: u32) -> Option<ResMode> {
        match val {
            0 => Some(ResMode::Auto),
            1 => Some(ResMode::Fixed),
            2 => Some(ResMode::Low),
            _ => None,
        }
    }

    /// The member's name, as its source spells it.
    pub const fn name(self) -> &'static str {
        match self {
            ResMode::Auto => "auto",
            ResMode::Fixed => "fixed",
            ResMode::Low => "low",
        }
    }
}

/// The hour the sky is drawn at, at the place the sky is seen from: the
/// wall clock's own, or one held in the place's solar time, so the sun
/// stands where it does there at that hour today whatever the clock reads
/// (pxl-core domain/scene/environment.rs `shown`). Only the sky's time;
/// the clouds, the frame's step and the storage keep the wall clock.
///
///   real      — the wall clock.
///   dawn      — 7:00, a low sun.
///   morning   — 9:00.
///   noon      — 12:00, the sun highest.
///   afternoon — 15:00.
///   dusk      — 17:00, a low sun.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum SkyTime {
    Real = 0,
    Dawn = 1,
    Morning = 2,
    Noon = 3,
    Afternoon = 4,
    Dusk = 5,
}

impl SkyTime {
    /// Every member, in its source's own order.
    pub const ALL: [SkyTime; 6] = [
        SkyTime::Real,
        SkyTime::Dawn,
        SkyTime::Morning,
        SkyTime::Noon,
        SkyTime::Afternoon,
        SkyTime::Dusk,
    ];

    /// The member `val` is, or none for a number no member has.
    pub const fn of(val: u32) -> Option<SkyTime> {
        match val {
            0 => Some(SkyTime::Real),
            1 => Some(SkyTime::Dawn),
            2 => Some(SkyTime::Morning),
            3 => Some(SkyTime::Noon),
            4 => Some(SkyTime::Afternoon),
            5 => Some(SkyTime::Dusk),
            _ => None,
        }
    }

    /// The member's name, as its source spells it.
    pub const fn name(self) -> &'static str {
        match self {
            SkyTime::Real => "real",
            SkyTime::Dawn => "dawn",
            SkyTime::Morning => "morning",
            SkyTime::Noon => "noon",
            SkyTime::Afternoon => "afternoon",
            SkyTime::Dusk => "dusk",
        }
    }
}

/// Every setting, by its number in schema.json5's SETTING.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum SettingKey {
    /// Ground color from the art pack (a tap of the layer array per
    /// fragment) instead of the flat per-instance average. The paint shader's
    /// `frame.low_spec` is this, inverted.
    ///
    /// Low because the tap is all of it: fill rate, which scales with
    /// the screen and not with the world, and nothing else — no geometry, no
    /// pass of its own, and no work off the GPU past an atlas the client
    /// already holds.
    Texture = 0,
    /// Procedurally scattered decor only; what the player placed stays.
    ///
    /// Med because it is models rather than a shading term: the GPU generates the
    /// ground's decor where the eye's clip levels are (decor_pose_cs) and the frame draws
    /// each kind as a model, an impostor or nothing by the angle it
    /// spans, so what it costs follows what is in view. The badge is an ordering,
    /// and this is the one to reach for before any shading term.
    Decor = 1,
    /// The cumulus' models and the altocumulus and cirrus sheets
    /// (rust/client/core/src/domain/scene/environment/cloud.rs, rust/client/render/src/pass/cloud.rs).
    /// Med as decor is: models in view, and a pass over the screen for the sheets.
    Cloud = 2,
    /// The places' markers over the world, from the published places tiles.
    Place = 3,
    /// Rotate-drag and pinch stop writing polar; azimuth is unaffected.
    /// Off by default: the tilt is the one control a view from above is expected to
    /// have, and locking it reads as the app being stuck.
    TiltLock = 4,
    /// On a switch to first person the person stands at the pivot, the eye at its
    /// height over the ground. On a switch to third person the pivot is laid on the
    /// ground and the eye steps back by the radius.
    CameraMode = 5,
    /// The pivot stands on what supports it and a pan takes the lowest way over what is in it (control/support.rs). Off, a pan keeps the pivot's height and passes through anything; a tap still stands it on the surface it struck.
    SurfaceFollow = 6,
    /// Only `climb` goes into a tunnel (camera/orbit.rs `dive`), so a shipped build holds it, and the others are a development build's to compare, each session from the default.
    OrbitObstacle = 7,
    /// Darkens cube edges that border open air (rust/client/render/src/frame/shader/paint.wgsl).
    /// A few fragment ops behind a uniform; the shader is the same either way.
    EdgeShade = 8,
    /// A uniform on an already-compiled shader term; not a pass.
    RimLight = 9,
    Ao = 10,
    /// The shadow map (rust/client/render/src/pass/shadow.rs): the world cast
    /// once more from the sun, or the moon where the sun is down, as the eye's
    /// rays are, none where neither is up; beams and samples on
    /// the node cache, the boxes they stop at drawn and a texel's sample
    /// taken, the rest walked (pxl-render queue.wgsl), recast only where what it
    /// reads moves; decor and the characters drawn into it depth only, and a 3x3
    /// compare per fragment; 16 MB of map.
    Shadow = 11,
    /// The pixels the world is drawn at: two to a CSS pixel is four times the
    /// work done a drawn pixel at a time (span.json5 `GPU_SPAN_PER_PX`), and nothing
    /// more; the camera grid's rays and the light's map cost the same at any.
    /// The markers and the finish stay at the display's.
    Res = 12,
    /// Where the camera and the pointer are, read live (perf/read.rs, NOW's view section).
    View = 13,
    /// The performance panel, recording while it is on (perf/window.rs `perf_follow`).
    Perf = 14,
    /// Marks for the pivot and the cell it floors to, and rings in the level, the view's upright and the north–south planes (domain/scene/gizmo/frame.rs); and over the ground the eye's windows' bricks by how they stand, their tiles and shards (domain/scene/gizmo/coverage.rs).
    WorldGizmo = 15,
    /// The camera grid over the screen, the cells the rays are spaced by, and the cells the rays stop on tinted by the share of a grid cell their faces show the eye (rust/client/render/src/frame/shader/camera_grid.wgsl, screen_shade.wgsl `cell_share`), in the looks debug_look.json5 gives them.
    CameraGizmo = 16,
    Person = 17,
    Pet = 18,
    /// Each ray's pixel colored by the steps taken on its way from the eye, as the frame casts it, on debug_look.json5's log ramp: the steps of the beam ray its cell starts off (beam::LINE_STEPS) and its own after, or, where it took a voxel or the sky without a walk, those of the sample before it on the camera grid's lattice, counted the same way (beam::SAMPLE_STEPS_SHIFT; rust/client/render/src/frame/shader/cast.wgsl `taken_word`, queue.wgsl `finish`).
    RayStep = 19,
    /// Counts what the eye's cast and the sun's shadow map do: the nodes each pass's walks asked, the pixels or slots that took the visible sample or walked, and the boxes drawn. The GPU counts them and writes them into the profiler's ray lanes of the frame that counted them (rust/client/engine/src/count.rs, rust/client/render/src/ray/count.rs). The counting's atomics run inside the cast's spans, so time the cast with this off.
    RayCnt = 20,
    SkyTime = 21,
}

impl SettingKey {
    /// Every member, in its source's own order.
    pub const ALL: [SettingKey; 22] = [
        SettingKey::Texture,
        SettingKey::Decor,
        SettingKey::Cloud,
        SettingKey::Place,
        SettingKey::TiltLock,
        SettingKey::CameraMode,
        SettingKey::SurfaceFollow,
        SettingKey::OrbitObstacle,
        SettingKey::EdgeShade,
        SettingKey::RimLight,
        SettingKey::Ao,
        SettingKey::Shadow,
        SettingKey::Res,
        SettingKey::View,
        SettingKey::Perf,
        SettingKey::WorldGizmo,
        SettingKey::CameraGizmo,
        SettingKey::Person,
        SettingKey::Pet,
        SettingKey::RayStep,
        SettingKey::RayCnt,
        SettingKey::SkyTime,
    ];

    /// The member `val` is, or none for a number no member has.
    pub const fn of(val: u32) -> Option<SettingKey> {
        match val {
            0 => Some(SettingKey::Texture),
            1 => Some(SettingKey::Decor),
            2 => Some(SettingKey::Cloud),
            3 => Some(SettingKey::Place),
            4 => Some(SettingKey::TiltLock),
            5 => Some(SettingKey::CameraMode),
            6 => Some(SettingKey::SurfaceFollow),
            7 => Some(SettingKey::OrbitObstacle),
            8 => Some(SettingKey::EdgeShade),
            9 => Some(SettingKey::RimLight),
            10 => Some(SettingKey::Ao),
            11 => Some(SettingKey::Shadow),
            12 => Some(SettingKey::Res),
            13 => Some(SettingKey::View),
            14 => Some(SettingKey::Perf),
            15 => Some(SettingKey::WorldGizmo),
            16 => Some(SettingKey::CameraGizmo),
            17 => Some(SettingKey::Person),
            18 => Some(SettingKey::Pet),
            19 => Some(SettingKey::RayStep),
            20 => Some(SettingKey::RayCnt),
            21 => Some(SettingKey::SkyTime),
            _ => None,
        }
    }

    /// The member's name, as its source spells it.
    pub const fn name(self) -> &'static str {
        match self {
            SettingKey::Texture => "texture",
            SettingKey::Decor => "decor",
            SettingKey::Cloud => "cloud",
            SettingKey::Place => "place",
            SettingKey::TiltLock => "tilt_lock",
            SettingKey::CameraMode => "camera_mode",
            SettingKey::SurfaceFollow => "surface_follow",
            SettingKey::OrbitObstacle => "orbit_obstacle",
            SettingKey::EdgeShade => "edge_shade",
            SettingKey::RimLight => "rim_light",
            SettingKey::Ao => "ao",
            SettingKey::Shadow => "shadow",
            SettingKey::Res => "res",
            SettingKey::View => "view",
            SettingKey::Perf => "perf",
            SettingKey::WorldGizmo => "world_gizmo",
            SettingKey::CameraGizmo => "camera_gizmo",
            SettingKey::Person => "person",
            SettingKey::Pet => "pet",
            SettingKey::RayStep => "ray_step",
            SettingKey::RayCnt => "ray_cnt",
            SettingKey::SkyTime => "sky_time",
        }
    }
}

impl SettingKey {
    /// One past the highest number: the length of a list of every
    /// setting's value at its number, where a number not given out stays 0.
    pub const SLOT_CNT: usize = 22;

    /// Written to the settings' storage file and read from it; one schema.json5
    /// marks `stored` false is held for the session alone.
    pub fn is_stored(self) -> bool {
        !matches!(
            self,
            SettingKey::Place
                | SettingKey::CameraMode
                | SettingKey::OrbitObstacle
                | SettingKey::SkyTime
        )
    }

    /// Offered by a development build only.
    pub fn dev(self) -> bool {
        matches!(
            self,
            SettingKey::SurfaceFollow
                | SettingKey::OrbitObstacle
                | SettingKey::RayCnt
                | SettingKey::SkyTime
        )
    }

    /// A choice's option a development build only offers.
    pub fn dev_option(self, _val: u32) -> bool {
        false
    }
}

/// Every setting's value: what the store holds, and what the app reads.
#[derive(Clone, Debug, PartialEq)]
pub struct Setting {
    pub texture: bool,
    pub decor: bool,
    pub cloud: bool,
    pub place: bool,
    pub tilt_lock: bool,
    pub camera_mode: CameraMode,
    pub surface_follow: bool,
    pub orbit_obstacle: OrbitObstacleMode,
    pub edge_shade: bool,
    pub rim_light: bool,
    pub ao: AoMode,
    pub shadow: bool,
    pub res: ResMode,
    pub view: bool,
    pub perf: bool,
    pub world_gizmo: bool,
    pub camera_gizmo: bool,
    pub person: u8,
    pub pet: u8,
    pub ray_step: bool,
    pub ray_cnt: bool,
    pub sky_time: SkyTime,
}

impl Setting {
    /// Every setting at schema.json5's default.
    pub const DEFAULT: Setting = Setting {
        texture: true,
        decor: true,
        cloud: true,
        place: false,
        tilt_lock: false,
        camera_mode: CameraMode::Orbit,
        surface_follow: true,
        orbit_obstacle: OrbitObstacleMode::Climb,
        edge_shade: true,
        rim_light: true,
        ao: AoMode::Corner,
        shadow: false,
        res: ResMode::Auto,
        view: false,
        perf: false,
        world_gizmo: false,
        camera_gizmo: false,
        person: 1,
        pet: 5,
        ray_step: false,
        ray_cnt: false,
        sky_time: SkyTime::Real,
    };

    /// A setting's value as it crosses and is kept: a toggle's 0 or 1, a
    /// choice's option by its number, an index as itself.
    pub fn get(&self, key: SettingKey) -> u32 {
        match key {
            SettingKey::Texture => self.texture as u32,
            SettingKey::Decor => self.decor as u32,
            SettingKey::Cloud => self.cloud as u32,
            SettingKey::Place => self.place as u32,
            SettingKey::TiltLock => self.tilt_lock as u32,
            SettingKey::CameraMode => self.camera_mode as u32,
            SettingKey::SurfaceFollow => self.surface_follow as u32,
            SettingKey::OrbitObstacle => self.orbit_obstacle as u32,
            SettingKey::EdgeShade => self.edge_shade as u32,
            SettingKey::RimLight => self.rim_light as u32,
            SettingKey::Ao => self.ao as u32,
            SettingKey::Shadow => self.shadow as u32,
            SettingKey::Res => self.res as u32,
            SettingKey::View => self.view as u32,
            SettingKey::Perf => self.perf as u32,
            SettingKey::WorldGizmo => self.world_gizmo as u32,
            SettingKey::CameraGizmo => self.camera_gizmo as u32,
            SettingKey::Person => self.person as u32,
            SettingKey::Pet => self.pet as u32,
            SettingKey::RayStep => self.ray_step as u32,
            SettingKey::RayCnt => self.ray_cnt as u32,
            SettingKey::SkyTime => self.sky_time as u32,
        }
    }

    /// Sets a setting from its value as it crosses; false, and nothing set,
    /// for a value the setting cannot hold.
    pub fn set(&mut self, key: SettingKey, val: u32) -> bool {
        match key {
            SettingKey::Texture if val <= 1 => self.texture = val == 1,
            SettingKey::Decor if val <= 1 => self.decor = val == 1,
            SettingKey::Cloud if val <= 1 => self.cloud = val == 1,
            SettingKey::Place if val <= 1 => self.place = val == 1,
            SettingKey::TiltLock if val <= 1 => self.tilt_lock = val == 1,
            SettingKey::CameraMode => match CameraMode::of(val) {
                Some(o) => self.camera_mode = o,
                None => return false,
            },
            SettingKey::SurfaceFollow if val <= 1 => self.surface_follow = val == 1,
            SettingKey::OrbitObstacle => match OrbitObstacleMode::of(val) {
                Some(o) => self.orbit_obstacle = o,
                None => return false,
            },
            SettingKey::EdgeShade if val <= 1 => self.edge_shade = val == 1,
            SettingKey::RimLight if val <= 1 => self.rim_light = val == 1,
            SettingKey::Ao => match AoMode::of(val) {
                Some(o) => self.ao = o,
                None => return false,
            },
            SettingKey::Shadow if val <= 1 => self.shadow = val == 1,
            SettingKey::Res => match ResMode::of(val) {
                Some(o) => self.res = o,
                None => return false,
            },
            SettingKey::View if val <= 1 => self.view = val == 1,
            SettingKey::Perf if val <= 1 => self.perf = val == 1,
            SettingKey::WorldGizmo if val <= 1 => self.world_gizmo = val == 1,
            SettingKey::CameraGizmo if val <= 1 => self.camera_gizmo = val == 1,
            SettingKey::Person => match u8::try_from(val) {
                Ok(n) => self.person = n,
                Err(_) => return false,
            },
            SettingKey::Pet => match u8::try_from(val) {
                Ok(n) => self.pet = n,
                Err(_) => return false,
            },
            SettingKey::RayStep if val <= 1 => self.ray_step = val == 1,
            SettingKey::RayCnt if val <= 1 => self.ray_cnt = val == 1,
            SettingKey::SkyTime => match SkyTime::of(val) {
                Some(o) => self.sky_time = o,
                None => return false,
            },
            _ => return false,
        }
        true
    }
}
