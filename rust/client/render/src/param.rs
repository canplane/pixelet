// pxl-render/param.rs

//! The settings that shape how the world is drawn and lit. Each is a literal, given with the
//! reason for its value. Each subject is a module, read by the code beside it. build.rs also
//! writes every setting into each render shader, after constant.wgsl, named by its path:
//! `sky::sun::LIMB` becomes `SKY_SUN_LIMB`.

/// A setting's value, typed as build.rs writes it into WGSL. An `F64` is the CPU's alone and
/// is not written.
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

/// Declares a module's settings as constants, and lists them again in the module's `ALL` for
/// build.rs, each with the module's path (`module_path!`), from which its WGSL name is made.
macro_rules! params {
    ($($(#[$m:meta])* $name:ident: $ty:tt = $v:expr;)*) => {
        $($(#[$m])* pub const $name: $ty = $v;)*
        pub const ALL: &[(&str, &str, $crate::param::Value)] =
            &[$((module_path!(), stringify!($name), val!($ty, $name)),)*];
    };
}

/// Every module's settings, as build.rs writes them. Each module must appear exactly once, and
/// param.test.rs checks it.
pub const ALL: &[&[(&str, &str, Value)]] = &[
    effect::ALL,
    ao::ALL,
    atmos::ALL,
    cloud::ALL,
    cloud::weather::ALL,
    cloud::altocumulus::ALL,
    cloud::cirrus::ALL,
    lens::ALL,
    light::ALL,
    noise::ALL,
    location::zone::ALL,
    location::beacon::ALL,
    shadow::ALL,
    sky::ALL,
    sky::sun::ALL,
    sky::moon::ALL,
    sky::star::ALL,
];

/// The effects on a drawn surface: its lines, the occlusion of its corners and of the screen,
/// and the shadows. They share one reach from the eye (pxl-spec space/layer/lattice.rs
/// `EFFECT_REACH`), and all fade alike before it (frame/shader/effect.wgsl `effect_share`).
pub mod effect {
    params! {
        /// The share of the reach at which the effects begin to fade. They are gone at the
        /// reach itself.
        FADE_START: f32 = 0.75;
        /// The width of a crease's line, a 64th of a unit, about 3 cm at Seoul's latitude.
        /// The line has this width on cells of every size, and only perspective thins it.
        EDGE_WIDTH: f32 = 1.0 / 64.0;
    }
}

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
        /// N8AO's denoise radius, 12 screen pixels. ao_blur.wgsl scales it to the passes'
        /// resolution (`AO_DOWNSCALE`).
        DENOISE_RADIUS: f32 = 12.0;
    }
}

/// How finely the air is marched for its tables (pass/atmosphere.rs). The transmittance,
/// multiple scattering and irradiance are made once a device. Each light's sky view and its
/// share of the volume are made again when the light's height or the eye's altitude has moved
/// half its step. The module also holds the altitude a sky view's slope is measured over.
pub mod atmos {
    params! {
        /// The steps that march the transmittance table, which holds the optical depth to the
        /// top.
        TRANSMITTANCE_STEPS: i32 = 40;
        /// These two settings size Hillaire's multiple scattering, which marches the second
        /// order with an isotropic phase.
        MULTISCATTER_DIRS: i32 = 8;
        MULTISCATTER_STEPS: i32 = 20;
        SKY_VIEW_STEPS: i32 = 30;
        /// These three settings size Bruneton's irradiance, the sky's light on an up-facing
        /// face.
        IRRADIANCE_RINGS: i32 = 8;
        IRRADIANCE_BEARINGS: i32 = 16;
        IRRADIANCE_STEPS: i32 = 16;
        /// The knots on a stretch between two of the volume's slices. Four are needed for
        /// the far slabs, which run a hundred km and more down into the ozone and the Earth's
        /// shadow.
        AIR_STRETCH_KNOTS: i32 = 4;
        /// A sky view's slope by altitude is taken over a rise of the eye: a share of the
        /// eye's altitude, a sixteenth, and at least ten meters (atmos_skyview.wgsl
        /// `slopes`).
        ALT_SAMPLE: f32 = 0.0625;
        ALT_SAMPLE_LEAST_KM: f32 = 0.01;
        /// The rows of a light's view, along its own vertical, that the exposure pass marches
        /// to tell how far the light reaches into the air (atmos_exposure.wgsl `reach`).
        REACH_ROWS: i32 = 16;
    }
}

/// The cumulus field and the thin sheets over it.
pub mod cloud {
    params! {
        /// The share of the sky a cloud covers: fair weather, a quarter.
        COVER: f32 = 0.25;
        /// Most of the summed noise lies near its mean. A threshold there would cut where the
        /// values are densest, and draw a thin veil everywhere and a full cloud nowhere.
        FULL_SHARE: f32 = 0.25;
        /// The octave sum is near normal. Its mean is exactly half the octaves' amplitudes,
        /// and its deviation was measured over the tile at four octaves with `DETAIL_SHARE` a
        /// quarter. A threshold at a quantile of that normal covers that share of the sky.
        SHAPE_MEAN: f32 = 0.5 * (1.0 - 1.0 / (1u32 << super::noise::OCTAVES) as f32);
        SHAPE_SIGMA: f32 = 0.0963;
        ALBEDO: f32 = 0.9;
        /// The optical depth of a full cloud. Past a depth of a few, the cloud's underside is
        /// in its own shadow.
        DEPTH: f32 = 6.0;
        /// The tile's shape is its fbm with a finer fbm laid over it. The finer one starts
        /// this many octaves up, draws from another salt, and is this share of the sum.
        DETAIL_FIRST: u32 = 2;
        DETAIL_SALT: u32 = 16;
        DETAIL_SHARE: f32 = 0.25;
        /// The share of the clouds' draw distance past which they thin out.
        DRAW_DIST_FADE: f32 = 0.6;
    }

    /// Where clouds gather: value noise over cells this many lattice units wide, about 64 km.
    /// The noise is spread evenly through the logistic, then cut so that some country stays
    /// clear and some is crowded.
    pub mod weather {
        params! {
            CELL: u32 = 8;
            /// The deviation of the bilinear smoothstep value noise about its mean of 0.5,
            /// from its formula; param.test.rs checks it.
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
        /// The halo's share of the glare of Spencer et al. 1995, from its terms
        /// (atmos_light.wgsl `glare_toward`). ISO 9358 puts veiling glare at 1-10%, and
        /// `GLARE_VEIL` is near the middle of that range.
        GLARE_HALO_SHARE: f32 = 0.6205;
        GLARE_VEIL: f32 = 0.03;
        /// These two are the defaults of postprocessing's VignetteEffect.
        VIGNETTE_DARKNESS: f32 = 0.45;
        VIGNETTE_OFF: f32 = 0.3;
    }
}

/// How what is drawn is lit.
pub mod light {
    params! {
        /// The share of the light that keeps its direction. The rest, what an up-facing face
        /// takes, reaches every face alike.
        BY_NORMAL: f32 = 0.25;
        /// The exponent only sets the band's width; `frame_rim_intensity()`
        /// scales the peak.
        RIM_POWER: f32 = 1.0;
        /// The Earth's picture is doubled, a value picked by eye beside the original picture.
        /// The cities' lights are not scaled by it.
        EARTH_GAIN: f32 = 2.0;
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

/// The location indicator's look in the world (pass/location.rs).
pub mod location {
    /// The zone of unit boxes about it (frame/shader/location_zone.wgsl). It is half
    /// transparent, so it reads as an indicator and not as a thing in the world.
    pub mod zone {
        params! {
            FILL_OPACITY: f32 = 0.4;
            BORDER_WIDTH: f32 = 0.04;
        }
    }

    /// The octahedron over it (frame/shader/location_beacon.wgsl).
    pub mod beacon {
        params! {
            RADIUS: f32 = 0.3;
            /// These are #ff1493 and #00f2ff in linear light; param.test.rs checks them.
            DAY: [f32; 3] = [1.0, 0.006995, 0.291771];
            NIGHT: [f32; 3] = [0.0, 0.887923, 1.0];
            /// These two are what the beacon gives off itself, by day and by night.
            DAY_GLOW: f32 = 0.25;
            NIGHT_GLOW: f32 = 1.0;
            /// The values of three.js's standard material for a gem. The metal share gives the
            /// light back as a highlight tinted the gem's color, and the rest is scattered.
            METALNESS: f32 = 0.8;
            ROUGHNESS: f32 = 0.2;
        }
    }
}

/// The sun's shadows.
pub mod shadow {
    params! {
        /// The share of the sunlight that every face takes alike (the rest after
        /// `light::BY_NORMAL`) which a cast shadow takes away. Taking all of it left the shade
        /// too dark.
        BY_UP_TAKEN: f32 = 0.5;
        /// The share of the shadow box's depth by which its planes step along the sun, on a
        /// grid fixed in the world as its texels are across. A step casts the world again,
        /// as every texel's ray starts on the near plane. The box is deeper than the sphere of
        /// its reach by half a step on each side, so the sphere lies within it wherever the
        /// step leaves it.
        DEPTH_STEP: f64 = 0.2;
    }
}

/// The sky's discs and stars.
pub mod sky {
    params! {
        /// At true size each disc is eight pixels on a phone, smaller than anyone remembers,
        /// and four times that was chosen by look. Only the size is exaggerated, not the light
        /// or the phase.
        DISC_SCALE: f32 = 4.0;
    }

    pub mod sun {
        params! {
            LIMB: f32 = 0.6;
            /// The disc is far past white once exposed, so the air in front colors it.
            DISC_GAIN: f32 = 4.0;
        }
    }

    pub mod moon {
        params! {
            /// The eye adapts to the moon, so its seas show where the sky's
            /// exposure would drown the disc white.
            FACE_SHOWN: f32 = 0.6;
            /// Earthshine is about 1e-4 of the sunlit side, which no 8-bit screen can show.
            /// This is how bright it is drawn at night, and it follows the Earth's phase.
            EARTHSHINE_SHOWN: f32 = 0.03;
        }
    }

    /// The Yale Bright Star Catalog's stars as quads at infinity.
    pub mod star {
        params! {
            /// Sizes are in CSS pixels. A point one pixel wide reads as a speck the eye passes
            /// over.
            GAIN: f32 = 1.5;
            SIGMA_PX: f32 = 0.85;
            SWELL_PX: f32 = 0.6;
            /// A faint wide halo shows which of two stars is brighter once both cores are
            /// white. Stars of the first magnitude and brighter have one (star.wgsl
            /// `HALO_FROM`).
            HALO_MAG: f32 = 1.0;
            HALO_SIGMA: f32 = 4.0;
            HALO_GAIN: f32 = 0.06;
            /// The faintest light a star is drawn at, in linear light. It is a choice; once
            /// encoded, it is several 8-bit steps.
            FLOOR: f32 = 0.5 / 255.0;
            /// How many of its deviations a star's quad reaches.
            CUTOFF_SIGMAS: f32 = 3.0;
            /// The most a star's light is drawn at.
            LIGHT_MAX: f32 = 4.0;
            /// The night sky's intensity at or under which no star is drawn
            /// (pass/star.rs `StarPass`).
            DRAWN_ABOVE: f32 = 0.001;
        }
    }
}

#[cfg(test)]
#[path = "param.test.rs"]
mod test;
