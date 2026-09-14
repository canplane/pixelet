// pxl_core/module/world/poi/poi_theme.rs

//! Category -> icon/color lookup for the POI badge, from poi_theme.json.

use super::poi_theme_spec::{CATEGORY_THEME as S, DEFAULT_THEME};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PoiTheme {
    pub icon: &'static str,
    pub color: &'static str,
}

macro_rules! themes {
    ($($key:ident),* $(,)?) => {
        &[$((stringify!($key), PoiTheme { icon: S::$key::icon, color: S::$key::color })),*]
    };
}

const CATEGORY_THEME: &[(&str, PoiTheme)] = themes![
    hospital,
    clinic,
    pharmacy,
    dentist,
    restaurant,
    cafe,
    fast_food,
    bar,
    r#pub,
    bank,
    atm,
    school,
    university,
    college,
    kindergarten,
    library,
    supermarket,
    convenience,
    mall,
    department_store,
    clothes,
    subway,
    station,
    bus_station,
    fuel,
    parking,
    hotel,
    guest_house,
    park,
    museum,
    attraction,
    place_of_worship,
    police,
    fire_station,
    post_office,
    townhall,
];

pub fn resolve_poi_theme(category: &str) -> PoiTheme {
    CATEGORY_THEME
        .iter()
        .find(|(key, _)| key.trim_start_matches("r#") == category)
        .map(|(_, theme)| *theme)
        .unwrap_or(PoiTheme {
            icon: DEFAULT_THEME::icon,
            color: DEFAULT_THEME::color,
        })
}
