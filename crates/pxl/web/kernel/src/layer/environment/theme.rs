// pxl-kernel/layer/environment/theme.rs

use crate::layer::environment::theme_spec::{daylight_split, themes as T, Theme};

/// A time-of-day theme: theme.json's `themes`, by its member.
pub type WorldThemeKey = Theme;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Light {
    pub color: u32,
    pub intensity: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hemisphere {
    pub ground: u32,
    pub intensity: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WorldTheme {
    pub ambient: Light,
    pub hemisphere: Hemisphere,
}

macro_rules! theme {
    ($k:ident) => {
        WorldTheme {
            ambient: Light {
                color: T::$k::lighting::ambient::color,
                intensity: T::$k::lighting::ambient::intensity,
            },
            hemisphere: Hemisphere {
                ground: T::$k::lighting::hemisphere::ground,
                intensity: T::$k::lighting::hemisphere::intensity,
            },
        }
    };
}

/// Every theme, at its number (theme.json's `$number`).
const THEMES: [WorldTheme; Theme::ALL.len()] = [theme!(morning), theme!(day), theme!(evening), theme!(night)];

const _: () = assert!(Theme::morning as usize == 0 && Theme::day as usize == 1 && Theme::evening as usize == 2 && Theme::night as usize == 3);

/// `themes[key]`.
pub fn themes(key: WorldThemeKey) -> WorldTheme {
    THEMES[key as usize]
}

/// Theme derived from the current hour and the real sunrise/sunset for wherever
/// the map is centered (crates/pxl/web/kernel/src/layer/environment/state.rs).
/// MORNING/DAY/EVENING split the daylight span by theme.json's daylight_split,
/// scaled to however long daylight actually is today.
pub fn get_theme_key(hour: f64, sunrise_hour: f64, sunset_hour: f64) -> WorldThemeKey {
    if hour < sunrise_hour || hour >= sunset_hour {
        return WorldThemeKey::night;
    }
    let daylight_fraction = (hour - sunrise_hour) / (sunset_hour - sunrise_hour);
    if daylight_fraction < daylight_split::MORNING_END {
        WorldThemeKey::morning
    } else if daylight_fraction < daylight_split::DAY_END {
        WorldThemeKey::day
    } else {
        WorldThemeKey::evening
    }
}
