// pxl_core/module/world/environment/theme.rs

use super::theme_spec::{daylight_split, themes as T};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum WorldThemeKey {
    Morning,
    #[default]
    Day,
    Evening,
    Night,
}

impl WorldThemeKey {
    /// The name the UI snapshot carries, and theme.json's key.
    pub fn as_str(self) -> &'static str {
        match self {
            WorldThemeKey::Morning => "morning",
            WorldThemeKey::Day => "day",
            WorldThemeKey::Evening => "evening",
            WorldThemeKey::Night => "night",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SkyColors {
    pub top: u32,
    pub middle: u32,
    pub horizon: u32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Light {
    pub color: u32,
    pub intensity: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hemisphere {
    pub sky: u32,
    pub ground: u32,
    pub intensity: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WorldTheme {
    pub fog: u32,
    pub sky: SkyColors,
    pub ambient: Light,
    pub hemisphere: Hemisphere,
    pub directional: Light,
}

macro_rules! theme {
    ($k:ident) => {
        WorldTheme {
            fog: T::$k::fog,
            sky: SkyColors {
                top: T::$k::sky::colors::top,
                middle: T::$k::sky::colors::middle,
                horizon: T::$k::sky::colors::horizon,
            },
            ambient: Light {
                color: T::$k::lighting::ambient::color,
                intensity: T::$k::lighting::ambient::intensity,
            },
            hemisphere: Hemisphere {
                sky: T::$k::lighting::hemisphere::sky,
                ground: T::$k::lighting::hemisphere::ground,
                intensity: T::$k::lighting::hemisphere::intensity,
            },
            directional: Light {
                color: T::$k::lighting::directional::color,
                intensity: T::$k::lighting::directional::intensity,
            },
        }
    };
}

/// `themes[key]`.
pub fn themes(key: WorldThemeKey) -> WorldTheme {
    match key {
        WorldThemeKey::Morning => theme!(morning),
        WorldThemeKey::Day => theme!(day),
        WorldThemeKey::Evening => theme!(evening),
        WorldThemeKey::Night => theme!(night),
    }
}

/// Theme derived from the current hour and the real sunrise/sunset for wherever
/// the map is centered (crates/pxl/core/src/module/map/environment.rs).
/// MORNING/DAY/EVENING split the daylight span by theme.json's daylight_split,
/// scaled to however long daylight actually is today.
pub fn get_theme_key(hour: f64, sunrise_hour: f64, sunset_hour: f64) -> WorldThemeKey {
    if hour < sunrise_hour || hour >= sunset_hour {
        return WorldThemeKey::Night;
    }
    let daylight_fraction = (hour - sunrise_hour) / (sunset_hour - sunrise_hour);
    if daylight_fraction < daylight_split::MORNING_END {
        WorldThemeKey::Morning
    } else if daylight_fraction < daylight_split::DAY_END {
        WorldThemeKey::Day
    } else {
        WorldThemeKey::Evening
    }
}
