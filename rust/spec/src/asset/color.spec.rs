// pxl-spec/asset/color.spec.rs
//
// GENERATED from spec/spec/asset/color.json (pxl-tool spec/asset.rs)
//
// The color of every palette entry and every placed color: what a cube *is* to
// the eye when no texture is drawn over it — everything drawn with the
// pack off reads this, the coarse cells included, which summarize it
// the way they summarize the tiles with the pack on
// (pxl-render paint.wgsl). A texture pack decorates these; it
// does not replace them.

// Names as the spec spells them, so one record has one name on every side.

use super::cube;
use super::Swatch;
use crate::color::rgb;

/// `sentinel`'s color for a value.
pub const fn sentinel(t: u8) -> Option<Swatch> {
    match t {
        // Bedrock, where the world holds no more: a dark gray, so it reads as the world's end against the night's black and the day's haze past it alike, rather than as whatever drew last.
        cube::sentinel::unknown => Some(Swatch::new(rgb("#565656"))),
        _ => None,
    }
}

/// `ground`'s color for a value.
pub const fn ground(t: u8) -> Option<Swatch> {
    match t {
        cube::ground::land => Some(Swatch::new(rgb("#f2efe9"))),
        cube::ground::bare_ground => Some(Swatch::new(rgb("#eee5dc"))),
        cube::ground::sand => Some(Swatch::new(rgb("#f5e9c6"))),
        cube::ground::beach => Some(Swatch::new(rgb("#fff1ba"))),
        cube::ground::heath => Some(Swatch::new(rgb("#d6d99f"))),
        cube::ground::scrub => Some(Swatch::new(rgb("#c8d7ab"))),
        cube::ground::grass => Some(Swatch::new(rgb("#cdebb0"))),
        cube::ground::park => Some(Swatch::new(rgb("#c8facc"))),
        cube::ground::leisure => Some(Swatch::new(rgb("#dffce2"))),
        cube::ground::pitch => Some(Swatch::new(rgb("#88e0be"))),
        cube::ground::forest => Some(Swatch::new(rgb("#add19e"))),
        cube::ground::orchard => Some(Swatch::new(rgb("#aedfa3"))),
        cube::ground::allotments => Some(Swatch::new(rgb("#c9e1bf"))),
        cube::ground::farmland => Some(Swatch::new(rgb("#eef0d5"))),
        cube::ground::farmyard => Some(Swatch::new(rgb("#f5dcba"))),
        cube::ground::campsite => Some(Swatch::new(rgb("#def6c0"))),
        cube::ground::cemetery => Some(Swatch::new(rgb("#aacbaf"))),
        cube::ground::residential => Some(Swatch::new(rgb("#e0dfdf"))),
        cube::ground::garages => Some(Swatch::new(rgb("#dfddce"))),
        cube::ground::retail => Some(Swatch::new(rgb("#ffd6d1"))),
        cube::ground::commercial => Some(Swatch::new(rgb("#f2dad9"))),
        cube::ground::industrial => Some(Swatch::new(rgb("#ebdbe8"))),
        cube::ground::power => Some(Swatch::new(rgb("#e2cbde"))),
        cube::ground::landfill => Some(Swatch::new(rgb("#b6b592"))),
        cube::ground::quarry => Some(Swatch::new(rgb("#c5c3c3"))),
        cube::ground::societal_amenities => Some(Swatch::new(rgb("#ffffe5"))),
        cube::ground::place_of_worship => Some(Swatch::new(rgb("#d0d0d0"))),
        cube::ground::transportation_area => Some(Swatch::new(rgb("#e9e7e2"))),
        cube::ground::parking => Some(Swatch::new(rgb("#eeeeee"))),
        cube::ground::apron => Some(Swatch::new(rgb("#dadae0"))),
        cube::ground::glacier => Some(Swatch::new(rgb("#ddecec"))),
        cube::ground::water => Some(Swatch::new(rgb("#aad3df"))),
        cube::ground::dam => Some(Swatch::new(rgb("#adadad"))),
        cube::ground::breakwater => Some(Swatch::new(rgb("#aaaaaa"))),
        cube::ground::building => Some(Swatch::new(rgb("#d9d0c9"))),
        cube::ground::building_major => Some(Swatch::new(rgb("#c4b6ab"))),
        cube::ground::footway => Some(Swatch::new(rgb("#fa8072"))),
        cube::ground::cycleway => Some(Swatch::new(rgb("#0000ff"))),
        cube::ground::bridleway => Some(Swatch::new(rgb("#008000"))),
        cube::ground::track => Some(Swatch::new(rgb("#996600"))),
        cube::ground::pedestrian => Some(Swatch::new(rgb("#dddde8"))),
        cube::ground::living_street => Some(Swatch::new(rgb("#ededed"))),
        cube::ground::residential_road => Some(Swatch::new(rgb("#ffffff"))),
        cube::ground::road => Some(Swatch::new(rgb("#dddddd"))),
        cube::ground::secondary => Some(Swatch::new(rgb("#f7fabf"))),
        cube::ground::primary => Some(Swatch::new(rgb("#fcd6a4"))),
        cube::ground::trunk => Some(Swatch::new(rgb("#f9b29c"))),
        cube::ground::motorway => Some(Swatch::new(rgb("#e892a2"))),
        cube::ground::platform => Some(Swatch::new(rgb("#bbbbbb"))),
        cube::ground::tram => Some(Swatch::new(rgb("#6e6e6e"))),
        cube::ground::light_rail => Some(Swatch::new(rgb("#666666"))),
        cube::ground::subway => Some(Swatch::new(rgb("#999999"))),
        cube::ground::monorail => Some(Swatch::new(rgb("#777777"))),
        cube::ground::rail => Some(Swatch::new(rgb("#707070"))),
        cube::ground::aeroway => Some(Swatch::new(rgb("#bbbbcc"))),
        // Minecraft's map color DIRT (minecraft.wiki Map_item_format): Carto paints nothing under the ground.
        cube::ground::soil => Some(Swatch::new(rgb("#976d4d"))),
        // Minecraft's map color SAND, which sandstone is drawn as on a map.
        cube::ground::sandstone => Some(Swatch::new(rgb("#f7e9a3"))),
        // Minecraft's map color STONE.
        cube::ground::stone => Some(Swatch::new(rgb("#707070"))),
        // A sentinel a baked cell can hold.
        _ => sentinel(t),
    }
}

/// `object`'s color for a value.
pub const fn object(t: u16) -> Option<Swatch> {
    match t {
        cube::object::white => Some(Swatch::new(rgb("#f7f7f7"))),
        cube::object::grey => Some(Swatch::new(rgb("#8d8d8d"))),
        cube::object::dark_grey => Some(Swatch::new(rgb("#3f3f3f"))),
        cube::object::black => Some(Swatch::new(rgb("#191919"))),
        cube::object::red => Some(Swatch::new(rgb("#b01313"))),
        cube::object::pink => Some(Swatch::new(rgb("#ff8787"))),
        cube::object::brown => Some(Swatch::new(rgb("#572b00"))),
        cube::object::orange => Some(Swatch::new(rgb("#d44e14"))),
        cube::object::yellow => Some(Swatch::new(rgb("#ffe611"))),
        cube::object::green => Some(Swatch::new(rgb("#60dc1e"))),
        cube::object::dark_green => Some(Swatch::new(rgb("#206700"))),
        cube::object::cyan => Some(Swatch::new(rgb("#008c94"))),
        cube::object::blue => Some(Swatch::new(rgb("#004791"))),
        cube::object::violet => Some(Swatch::new(rgb("#57039f"))),
        cube::object::magenta => Some(Swatch::new(rgb("#c9036d"))),
        _ => None,
    }
}
