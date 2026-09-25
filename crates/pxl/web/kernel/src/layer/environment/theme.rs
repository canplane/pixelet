// pxl-kernel/layer/environment/theme.rs

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Light {
    pub color: u32,
    pub intensity: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WorldTheme {
    pub ambient: Light,
}

/// Every theme, at its number.
const THEMES: [WorldTheme; Theme::ALL.len()] = [
    WorldTheme {
        ambient: Light { color: 0xfff2e3, intensity: 0.62 },
    },
    WorldTheme {
        ambient: Light { color: 0xf9fcff, intensity: 0.68 },
    },
    WorldTheme {
        ambient: Light { color: 0xffe4cd, intensity: 0.58 },
    },
    WorldTheme {
        ambient: Light { color: 0x24304a, intensity: 0.4 },
    },
];

const _: () = assert!(Theme::Morning as usize == 0 && Theme::Day as usize == 1 && Theme::Evening as usize == 2 && Theme::Night as usize == 3);

/// `themes[key]`.
pub fn themes(key: Theme) -> WorldTheme {
    THEMES[key as usize]
}

/// Theme derived from the current hour and the real sunrise/sunset for wherever
/// the map is centered (crates/pxl/web/kernel/src/layer/environment/state.rs).
/// MORNING/DAY/EVENING split the daylight span by `daylight_split`,
/// scaled to however long daylight actually is today.
pub fn get_theme_key(hour: f64, sunrise_hour: f64, sunset_hour: f64) -> Theme {
    if hour < sunrise_hour || hour >= sunset_hour {
        return Theme::Night;
    }
    let daylight_fraction = (hour - sunrise_hour) / (sunset_hour - sunrise_hour);
    if daylight_fraction < daylight_split::MORNING_END {
        Theme::Morning
    } else if daylight_fraction < daylight_split::DAY_END {
        Theme::Day
    } else {
        Theme::Evening
    }
}

// The world's time-of-day themes, by Theme's value. The sun's light
// and the sky, by day and by night, are the atmosphere's
// (crates/pxl/web/render/src/look/atmosphere.rs), and so is the light a face
// takes from the land under it (atmos_view.wgsl's ground_bounce), so a theme
// is the ambient the fill still carries and whether it is night.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Theme {
    Morning = 0,
    Day = 1,
    Evening = 2,
    Night = 3,
}

impl Theme {
    /// Every member, in the order they are numbered.
    pub const ALL: [Theme; 4] = [Theme::Morning, Theme::Day, Theme::Evening, Theme::Night];

    /// The member a number names, or none for a number the set does not
    /// give out.
    pub fn of(value: u8) -> Option<Theme> {
        Theme::ALL.into_iter().find(|member| *member as u8 == value)
    }
}

/// Where get_theme_key splits the daylight span: MORNING before MORNING_END
/// of it, DAY before DAY_END, EVENING after (37.5% / 37.5% / 25%, scaled to
/// however long daylight actually is today).
pub mod daylight_split {
    pub const MORNING_END: f64 = 0.375;
    pub const DAY_END: f64 = 0.75;
}
