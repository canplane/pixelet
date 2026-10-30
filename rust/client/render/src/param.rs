// pxl-render/param.rs

//! The settings that shape how the world is drawn and lit, each one literal
//! with why it is what it is: a subject a module, read by the code beside
//! it and handed by build.rs to every render shader (constant.wgsl's tail),
//! each named by its path (`sky::sun::LIMB` is `SKY_SUN_LIMB`), so no
//! shader holds a value of its own that its source could leave behind.

/// A setting's value as build.rs writes it into WGSL; an `F64` is the
/// host's alone and is not written.
#[derive(Clone, Copy, Debug)]
pub enum Value {
    F32(f32),
    F64(f64),
    I32(i32),
    U32(u32),
    Vec2([f32; 2]),
    Vec3([f32; 3]),
}

macro_rules! val {
    (f32, $v:expr) => {
        $crate::param::Value::F32($v)
    };
    (f64, $v:expr) => {
        $crate::param::Value::F64($v)
    };
    (i32, $v:expr) => {
        $crate::param::Value::I32($v)
    };
    (u32, $v:expr) => {
        $crate::param::Value::U32($v)
    };
    ([f32; 2], $v:expr) => {
        $crate::param::Value::Vec2($v)
    };
    ([f32; 3], $v:expr) => {
        $crate::param::Value::Vec3($v)
    };
}

/// A module's settings, and the same as build.rs reads them: each with the
/// module's own path (`module_path!`), which names it.
macro_rules! params {
    ($($(#[$m:meta])* $name:ident: $ty:tt = $v:expr;)*) => {
        $($(#[$m])* pub const $name: $ty = $v;)*
        pub const ALL: &[(&str, &str, $crate::param::Value)] =
            &[$((module_path!(), stringify!($name), val!($ty, $name)),)*];
    };
}

/// Every module's settings, as build.rs writes them: each module once, which
/// param.test.rs holds it to.
pub const ALL: &[&[(&str, &str, Value)]] = &[
    ao::ALL,
    atmos::ALL,
    cloud::ALL,
    cloud::weather::ALL,
    cloud::altocumulus::ALL,
    cloud::cirrus::ALL,
    lens::ALL,
    light::ALL,
    night::ALL,
    noise::ALL,
    puck::zone::ALL,
    puck::beacon::ALL,
    shadow::ALL,
    sky::ALL,
    sky::sun::ALL,
    sky::moon::ALL,
    sky::star::ALL,
];

/// The screen's ambient occlusion (the passes frame/shader/ao_*.wgsl).
pub mod ao {
    params! {
        RADIUS: f32 = 1.5;
        DIST_FALLOFF: f32 = 1.0;
        INTENSITY: f32 = 1.2;
        SAMPLES: i32 = 4;
        DENOISE_SAMPLES: i32 = 4;
        /// N8AO scales the falloff by a fifth when the radius is in world
        /// units.
        FALLOFF_SHARE: f32 = 0.2;
        /// N8AO's denoise radius, 12 of the screen's pixels, which ao_blur.wgsl
        /// takes to the passes' resolution (`AO_DOWNSCALE`).
        DENOISE_RADIUS: f32 = 12.0;
    }
}

/// How finely the air is marched: its tables (gpu/atmosphere.rs), the
/// transmittance, multiple scattering and irradiance once a device and the
/// sky views whenever the light's height or the camera's altitude moves; and
/// its volume, whenever the frame's or the sky's words move.
pub mod atmos {
    params! {
        /// Transmittance, as optical depth to the top.
        TRANSMITTANCE_STEPS: i32 = 40;
        /// Hillaire's multiple scattering: the second order marched with an
        /// isotropic phase.
        MULTISCATTER_DIRS: i32 = 8;
        MULTISCATTER_STEPS: i32 = 20;
        SKY_VIEW_STEPS: i32 = 30;
        /// Bruneton's irradiance: the sky's light on an up-facing face.
        IRRADIANCE_RINGS: i32 = 8;
        IRRADIANCE_BEARINGS: i32 = 16;
        IRRADIANCE_STEPS: i32 = 16;
        /// Knots to a stretch between two of the volume's slices: four, for
        /// its far slabs, a hundred km and more down into the ozone and the
        /// Earth's shadow.
        AIR_STRETCH_KNOTS: i32 = 4;
    }
}

/// The cumulus field and the thin sheets over it.
pub mod cloud {
    params! {
        /// The share of the sky a cloud covers: fair weather, a quarter.
        COVER: f32 = 0.25;
        /// A threshold near the summed noise's mean cut it where it is
        /// densest: a thin veil everywhere and a full cloud nowhere.
        FULL_SHARE: f32 = 0.25;
        /// The octave sum is near normal: its mean exactly half the octaves'
        /// amplitudes, its deviation measured over the tile at four octaves
        /// and `DETAIL_SHARE` a quarter. A threshold on its quantile is the
        /// covered share.
        SHAPE_MEAN: f32 = 0.5 * (1.0 - 1.0 / (1u32 << super::noise::OCTAVES) as f32);
        SHAPE_SIGMA: f32 = 0.0963;
        ALBEDO: f32 = 0.9;
        /// Optical depth of a full cloud: past a few its underside is
        /// self-shadowed.
        DEPTH: f32 = 6.0;
        /// The tile's shape is its fbm with a finer one over it, starting
        /// this many octaves up, salted apart, a share of the sum.
        DETAIL_FIRST: u32 = 2;
        DETAIL_SALT: u32 = 16;
        DETAIL_SHARE: f32 = 0.25;
        /// The share of the clouds' reach past which they thin out.
        REACH_FADE: f32 = 0.6;
    }

    /// Where clouds gather: value noise over cells this many lattice units
    /// wide, about 64 km, spread even through the logistic and cut so some
    /// country stays clear and some is crowded, rather than every field
    /// equally dotted.
    pub mod weather {
        params! {
            CELL: u32 = 8;
            /// The bilinear smoothstep value noise's deviation about its 0.5
            /// mean, from its formula (param.test.rs holds it to it).
            SIGMA: f32 = 0.2144;
            CLEAR: f32 = 0.35;
            CROWDED: f32 = 0.75;
        }
    }

    /// Altocumulus' small heaps at 4 to 5 km (frame/shader/cloud.wgsl
    /// `Sheet`).
    pub mod altocumulus {
        params! {
            KM: f32 = 4.5;
            /// How many times finer than the tile its pattern is.
            FINER: u32 = 3;
            /// How much softer its edges read, as a footprint's multiple.
            SOFTER: f32 = 1.0;
            OPACITY: f32 = 0.6;
            SALT: u32 = 0x0a1c;
        }
    }

    /// Cirrus' veil at 6 to 12 km (frame/shader/cloud.wgsl `Sheet`).
    pub mod cirrus {
        params! {
            KM: f32 = 9.0;
            FINER: u32 = 1;
            SOFTER: f32 = 4.0;
            OPACITY: f32 = 0.45;
            SALT: u32 = 0x0c1c;
        }
    }
}

/// What the eye's lens adds to the light that reaches it.
pub mod lens {
    params! {
        /// The halo's share of Spencer et al. 1995's glare, its terms'
        /// (atmos_light.wgsl `glare_toward`); ISO 9358 veiling glare is
        /// 1-10%, `GLARE_VEIL` its middle.
        GLARE_HALO_SHARE: f32 = 0.6205;
        GLARE_VEIL: f32 = 0.03;
        /// postprocessing's VignetteEffect defaults.
        VIGNETTE_DARKNESS: f32 = 0.45;
        VIGNETTE_OFF: f32 = 0.3;
    }
}

/// How what is drawn is lit.
pub mod light {
    params! {
        /// The share that keeps its direction; the rest, an up-facing face's,
        /// reaches every face alike. By the normal alone, a wall turned from
        /// the sun read as a toy's.
        BY_NORMAL: f32 = 0.25;
        /// The exponent only sets the band's width; `frame_rim_intensity()`
        /// scales the peak.
        RIM_POWER: f32 = 1.0;
        /// The Earth's picture doubled, picked by eye beside the original;
        /// the cities are not its to scale.
        EARTH_GAIN: f32 = 2.0;
    }
}

/// Where night falls.
pub mod night {
    params! {
        /// The sine of the sun's height over which night falls, wherever it
        /// is shown: the stars over the sky (look/environment.rs), the
        /// cities' lights over the land (atmos_air.wgsl `city_shown`), and the
        /// night the kernel tells (pxl-kernel domain/scene/environment.rs
        /// `night`); whole at the first, none at the second, the horizon.
        FALL: [f32; 2] = [-0.1, 0.0];
    }
}

/// The lattice's fractal noise (frame/shader/noise.wgsl `lattice_fbm`).
pub mod noise {
    params! {
        OCTAVES: u32 = 4;
        /// An offset per octave keeps lattice lines from coinciding into a
        /// visible grid.
        OCTAVE_OFF: [f32; 2] = [0.37, 0.61];
    }
}

/// The location puck's look in the world (gpu/pass/puck.rs).
pub mod puck {
    /// The zone of unit boxes about it (frame/shader/puck_zone.wgsl): half
    /// transparent, so it reads as an indicator rather than a surface.
    pub mod zone {
        params! {
            FILL_OPACITY: f32 = 0.4;
            BORDER_WIDTH: f32 = 0.04;
        }
    }

    /// The octahedron over it (frame/shader/puck_beacon.wgsl).
    pub mod beacon {
        params! {
            RADIUS: f32 = 0.3;
            /// #ff1493 and #00f2ff, in linear light (param.test.rs holds
            /// them to it).
            DAY: [f32; 3] = [1.0, 0.006995, 0.291771];
            NIGHT: [f32; 3] = [0.0, 0.887923, 1.0];
            /// What it gives off itself, by day and by night.
            DAY_GLOW: f32 = 0.25;
            NIGHT_GLOW: f32 = 1.0;
            /// three's standard material's, as the gem's: metal gives its light
            /// back as a highlight tinted its color, the rest scattered.
            METALNESS: f32 = 0.8;
            ROUGHNESS: f32 = 0.2;
        }
    }
}

/// The sun's shadows.
pub mod shadow {
    params! {
        /// The share of the sunlight every face takes alike
        /// (`light::BY_NORMAL`'s rest) a cast shadow takes away; all of it left
        /// the shade as dark.
        BY_UP_TAKEN: f32 = 0.5;
        /// The share of the casters' reach past which a shadow fades to
        /// nothing at it.
        FADE_START: f32 = 0.75;
        /// The share of the box's depth its planes step along the sun by, on
        /// a grid fixed in the world as its texels are across: a step casts
        /// the world again, since every texel's ray starts on the near plane.
        /// The box is as much deeper than the sphere of its reach, half a step
        /// each side, so the sphere lies within it wherever the step leaves
        /// it.
        DEPTH_STEP: f64 = 0.2;
    }
}

/// The sky's discs and stars.
pub mod sky {
    params! {
        /// At true size each disc is eight pixels on a phone, smaller than
        /// anyone remembers; four is chosen by look. Only size is
        /// exaggerated, not light or phase.
        DISC_SCALE: f32 = 4.0;
    }

    pub mod sun {
        params! {
            LIMB: f32 = 0.6;
            /// Far past white once exposed, so the air in front colors it.
            DISC_GAIN: f32 = 4.0;
        }
    }

    pub mod moon {
        params! {
            /// The eye adapts to the moon, so its seas show where the sky's
            /// exposure would drown the disc white.
            FACE_SHOWN: f32 = 0.6;
            /// Earthshine is ~1e-4 of the sunlit side, which no 8-bit screen
            /// holds; this is how it reads at night, going with the Earth's
            /// phase.
            EARTHSHINE_SHOWN: f32 = 0.03;
        }
    }

    /// The Yale Bright Star Catalog's stars as quads at infinity.
    pub mod star {
        params! {
            /// In CSS pixels: a point a pixel wide reads as a speck the eye
            /// passes over.
            GAIN: f32 = 1.5;
            SIGMA_PX: f32 = 0.85;
            SWELL_PX: f32 = 0.6;
            /// A faint wide halo shows which star is brighter once both cores
            /// are white: from a first-magnitude star on (star.wgsl
            /// `HALO_FROM`).
            HALO_MAG: f32 = 1.0;
            HALO_SIGMA: f32 = 4.0;
            HALO_GAIN: f32 = 0.06;
            /// The faintest light a star is drawn at, in linear light, a
            /// choice: encoded, it is several 8-bit steps.
            FLOOR: f32 = 0.5 / 255.0;
            /// How many of its deviations a star's quad reaches.
            CUTOFF_SIGMAS: f32 = 3.0;
            /// The most a star's light is drawn at.
            LIGHT_MAX: f32 = 4.0;
            /// The night sky's intensity at or under which no star is drawn
            /// (gpu/star.rs `StarPass`).
            DRAWN_ABOVE: f32 = 0.001;
        }
    }
}

#[cfg(test)]
#[path = "param.test.rs"]
mod test;
