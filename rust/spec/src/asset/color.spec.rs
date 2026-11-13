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

use super::cube::{Ground, Object, Sentinel};
use super::Swatch;
use crate::color::rgb;

/// `sentinel`'s color for a value.
pub const fn sentinel(t: u16) -> Option<Swatch> {
    match Sentinel::of(t) {
        // Bedrock, where the world holds no more: a dark gray, so it reads as the world's end against the night's black and the day's haze past it alike, rather than as whatever drew last.
        Some(Sentinel::unknown) => Some(const { Swatch::new(rgb("#565656")) }),
        _ => None,
    }
}

/// `ground`'s color for a value.
pub const fn ground(t: u8) -> Option<Swatch> {
    match Ground::of(t) {
        Some(Ground::land) => Some(const { Swatch::new(rgb("#f2efe9")) }),
        Some(Ground::bare_ground) => Some(const { Swatch::new(rgb("#eee5dc")) }),
        Some(Ground::sand) => Some(const { Swatch::new(rgb("#f5e9c6")) }),
        Some(Ground::beach) => Some(const { Swatch::new(rgb("#fff1ba")) }),
        Some(Ground::heath) => Some(const { Swatch::new(rgb("#d6d99f")) }),
        Some(Ground::scrub) => Some(const { Swatch::new(rgb("#c8d7ab")) }),
        Some(Ground::grass) => Some(const { Swatch::new(rgb("#cdebb0")) }),
        Some(Ground::park) => Some(const { Swatch::new(rgb("#c8facc")) }),
        Some(Ground::leisure) => Some(const { Swatch::new(rgb("#dffce2")) }),
        Some(Ground::pitch) => Some(const { Swatch::new(rgb("#88e0be")) }),
        Some(Ground::forest) => Some(const { Swatch::new(rgb("#add19e")) }),
        Some(Ground::orchard) => Some(const { Swatch::new(rgb("#aedfa3")) }),
        Some(Ground::allotments) => Some(const { Swatch::new(rgb("#c9e1bf")) }),
        Some(Ground::farmland) => Some(const { Swatch::new(rgb("#eef0d5")) }),
        Some(Ground::farmyard) => Some(const { Swatch::new(rgb("#f5dcba")) }),
        Some(Ground::campsite) => Some(const { Swatch::new(rgb("#def6c0")) }),
        Some(Ground::cemetery) => Some(const { Swatch::new(rgb("#aacbaf")) }),
        Some(Ground::residential) => Some(const { Swatch::new(rgb("#e0dfdf")) }),
        Some(Ground::garages) => Some(const { Swatch::new(rgb("#dfddce")) }),
        Some(Ground::retail) => Some(const { Swatch::new(rgb("#ffd6d1")) }),
        Some(Ground::commercial) => Some(const { Swatch::new(rgb("#f2dad9")) }),
        Some(Ground::industrial) => Some(const { Swatch::new(rgb("#ebdbe8")) }),
        Some(Ground::power) => Some(const { Swatch::new(rgb("#e2cbde")) }),
        Some(Ground::landfill) => Some(const { Swatch::new(rgb("#b6b592")) }),
        Some(Ground::quarry) => Some(const { Swatch::new(rgb("#c5c3c3")) }),
        Some(Ground::societal_amenities) => Some(const { Swatch::new(rgb("#ffffe5")) }),
        Some(Ground::place_of_worship) => Some(const { Swatch::new(rgb("#d0d0d0")) }),
        Some(Ground::transportation_area) => Some(const { Swatch::new(rgb("#e9e7e2")) }),
        Some(Ground::parking) => Some(const { Swatch::new(rgb("#eeeeee")) }),
        Some(Ground::apron) => Some(const { Swatch::new(rgb("#dadae0")) }),
        Some(Ground::glacier) => Some(const { Swatch::new(rgb("#ddecec")) }),
        Some(Ground::water) => Some(const { Swatch::new(rgb("#aad3df")) }),
        Some(Ground::dam) => Some(const { Swatch::new(rgb("#adadad")) }),
        Some(Ground::breakwater) => Some(const { Swatch::new(rgb("#aaaaaa")) }),
        Some(Ground::building) => Some(const { Swatch::new(rgb("#d9d0c9")) }),
        Some(Ground::building_major) => Some(const { Swatch::new(rgb("#c4b6ab")) }),
        Some(Ground::footway) => Some(const { Swatch::new(rgb("#fa8072")) }),
        Some(Ground::cycleway) => Some(const { Swatch::new(rgb("#0000ff")) }),
        Some(Ground::bridleway) => Some(const { Swatch::new(rgb("#008000")) }),
        Some(Ground::track) => Some(const { Swatch::new(rgb("#996600")) }),
        Some(Ground::pedestrian) => Some(const { Swatch::new(rgb("#dddde8")) }),
        Some(Ground::living_street) => Some(const { Swatch::new(rgb("#ededed")) }),
        Some(Ground::residential_road) => Some(const { Swatch::new(rgb("#ffffff")) }),
        Some(Ground::road) => Some(const { Swatch::new(rgb("#dddddd")) }),
        Some(Ground::secondary) => Some(const { Swatch::new(rgb("#f7fabf")) }),
        Some(Ground::primary) => Some(const { Swatch::new(rgb("#fcd6a4")) }),
        Some(Ground::trunk) => Some(const { Swatch::new(rgb("#f9b29c")) }),
        Some(Ground::motorway) => Some(const { Swatch::new(rgb("#e892a2")) }),
        Some(Ground::platform) => Some(const { Swatch::new(rgb("#bbbbbb")) }),
        Some(Ground::tram) => Some(const { Swatch::new(rgb("#6e6e6e")) }),
        Some(Ground::light_rail) => Some(const { Swatch::new(rgb("#666666")) }),
        Some(Ground::subway) => Some(const { Swatch::new(rgb("#999999")) }),
        Some(Ground::monorail) => Some(const { Swatch::new(rgb("#777777")) }),
        Some(Ground::rail) => Some(const { Swatch::new(rgb("#707070")) }),
        Some(Ground::aeroway) => Some(const { Swatch::new(rgb("#bbbbcc")) }),
        // Minecraft's map color DIRT (minecraft.wiki Map_item_format): Carto paints nothing under the ground.
        Some(Ground::soil) => Some(const { Swatch::new(rgb("#976d4d")) }),
        // Minecraft's map color SAND, which sandstone is drawn as on a map.
        Some(Ground::sandstone) => Some(const { Swatch::new(rgb("#f7e9a3")) }),
        // Minecraft's map color STONE.
        Some(Ground::stone) => Some(const { Swatch::new(rgb("#707070")) }),
        // A sentinel a baked cell can hold.
        None => sentinel(t as u16),
    }
}

/// `object`'s color for a value.
pub const fn object(t: u16) -> Option<Swatch> {
    match Object::of(t) {
        Some(Object::white) => Some(const { Swatch::new(rgb("#f7f7f7")) }),
        Some(Object::grey) => Some(const { Swatch::new(rgb("#8d8d8d")) }),
        Some(Object::dark_grey) => Some(const { Swatch::new(rgb("#3f3f3f")) }),
        Some(Object::black) => Some(const { Swatch::new(rgb("#191919")) }),
        Some(Object::red) => Some(const { Swatch::new(rgb("#b01313")) }),
        Some(Object::pink) => Some(const { Swatch::new(rgb("#ff8787")) }),
        Some(Object::brown) => Some(const { Swatch::new(rgb("#572b00")) }),
        Some(Object::orange) => Some(const { Swatch::new(rgb("#d44e14")) }),
        Some(Object::yellow) => Some(const { Swatch::new(rgb("#ffe611")) }),
        Some(Object::green) => Some(const { Swatch::new(rgb("#60dc1e")) }),
        Some(Object::dark_green) => Some(const { Swatch::new(rgb("#206700")) }),
        Some(Object::cyan) => Some(const { Swatch::new(rgb("#008c94")) }),
        Some(Object::blue) => Some(const { Swatch::new(rgb("#004791")) }),
        Some(Object::violet) => Some(const { Swatch::new(rgb("#57039f")) }),
        Some(Object::magenta) => Some(const { Swatch::new(rgb("#c9036d")) }),
        _ => None,
    }
}
