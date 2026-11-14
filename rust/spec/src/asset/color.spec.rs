// pxl-spec/asset/color.spec.rs
//
// GENERATED from spec/spec/asset/color.json (pxl-tool spec/asset.rs)
//
// The color of every ground cube and every placed color: what a cube *is* to
// the eye when no texture is drawn over it — everything drawn with the
// pack off reads this, the coarse cells included, which summarize it
// the way they summarize the tiles with the pack on
// (pxl-render paint.wgsl). A texture pack decorates these; it
// does not replace them.

use super::cube::{Ground, Object, Sentinel};
use super::Swatch;
use crate::color::rgb;

/// `sentinel`'s color for a value.
pub const fn sentinel(t: u16) -> Option<Swatch> {
    match Sentinel::of(t) {
        // Bedrock, where the world holds no more: a dark gray, so it reads as the world's end against the night's black and the day's haze past it alike, rather than as whatever drew last.
        Some(Sentinel::Unknown) => Some(const { Swatch::new(rgb("#565656")) }),
        _ => None,
    }
}

/// `ground`'s color for a value.
pub const fn ground(t: u8) -> Option<Swatch> {
    match Ground::of(t) {
        Some(Ground::Land) => Some(const { Swatch::new(rgb("#f2efe9")) }),
        Some(Ground::BareGround) => Some(const { Swatch::new(rgb("#eee5dc")) }),
        Some(Ground::Sand) => Some(const { Swatch::new(rgb("#f5e9c6")) }),
        Some(Ground::Beach) => Some(const { Swatch::new(rgb("#fff1ba")) }),
        Some(Ground::Heath) => Some(const { Swatch::new(rgb("#d6d99f")) }),
        Some(Ground::Scrub) => Some(const { Swatch::new(rgb("#c8d7ab")) }),
        Some(Ground::Grass) => Some(const { Swatch::new(rgb("#cdebb0")) }),
        Some(Ground::Park) => Some(const { Swatch::new(rgb("#c8facc")) }),
        Some(Ground::Leisure) => Some(const { Swatch::new(rgb("#dffce2")) }),
        Some(Ground::Pitch) => Some(const { Swatch::new(rgb("#88e0be")) }),
        Some(Ground::Forest) => Some(const { Swatch::new(rgb("#add19e")) }),
        Some(Ground::Orchard) => Some(const { Swatch::new(rgb("#aedfa3")) }),
        Some(Ground::Allotments) => Some(const { Swatch::new(rgb("#c9e1bf")) }),
        Some(Ground::Farmland) => Some(const { Swatch::new(rgb("#eef0d5")) }),
        Some(Ground::Farmyard) => Some(const { Swatch::new(rgb("#f5dcba")) }),
        Some(Ground::Campsite) => Some(const { Swatch::new(rgb("#def6c0")) }),
        Some(Ground::Cemetery) => Some(const { Swatch::new(rgb("#aacbaf")) }),
        Some(Ground::Residential) => Some(const { Swatch::new(rgb("#e0dfdf")) }),
        Some(Ground::Garages) => Some(const { Swatch::new(rgb("#dfddce")) }),
        Some(Ground::Retail) => Some(const { Swatch::new(rgb("#ffd6d1")) }),
        Some(Ground::Commercial) => Some(const { Swatch::new(rgb("#f2dad9")) }),
        Some(Ground::Industrial) => Some(const { Swatch::new(rgb("#ebdbe8")) }),
        Some(Ground::Power) => Some(const { Swatch::new(rgb("#e2cbde")) }),
        Some(Ground::Landfill) => Some(const { Swatch::new(rgb("#b6b592")) }),
        Some(Ground::Quarry) => Some(const { Swatch::new(rgb("#c5c3c3")) }),
        Some(Ground::SocietalAmenities) => Some(const { Swatch::new(rgb("#ffffe5")) }),
        Some(Ground::PlaceOfWorship) => Some(const { Swatch::new(rgb("#d0d0d0")) }),
        Some(Ground::TransportationArea) => Some(const { Swatch::new(rgb("#e9e7e2")) }),
        Some(Ground::Parking) => Some(const { Swatch::new(rgb("#eeeeee")) }),
        Some(Ground::Apron) => Some(const { Swatch::new(rgb("#dadae0")) }),
        Some(Ground::Glacier) => Some(const { Swatch::new(rgb("#ddecec")) }),
        Some(Ground::Water) => Some(const { Swatch::new(rgb("#aad3df")) }),
        Some(Ground::Dam) => Some(const { Swatch::new(rgb("#adadad")) }),
        Some(Ground::Breakwater) => Some(const { Swatch::new(rgb("#aaaaaa")) }),
        Some(Ground::Building) => Some(const { Swatch::new(rgb("#d9d0c9")) }),
        Some(Ground::BuildingMajor) => Some(const { Swatch::new(rgb("#c4b6ab")) }),
        Some(Ground::Footway) => Some(const { Swatch::new(rgb("#fa8072")) }),
        Some(Ground::Cycleway) => Some(const { Swatch::new(rgb("#0000ff")) }),
        Some(Ground::Bridleway) => Some(const { Swatch::new(rgb("#008000")) }),
        Some(Ground::Track) => Some(const { Swatch::new(rgb("#996600")) }),
        Some(Ground::Pedestrian) => Some(const { Swatch::new(rgb("#dddde8")) }),
        Some(Ground::LivingStreet) => Some(const { Swatch::new(rgb("#ededed")) }),
        Some(Ground::ResidentialRoad) => Some(const { Swatch::new(rgb("#ffffff")) }),
        Some(Ground::Road) => Some(const { Swatch::new(rgb("#dddddd")) }),
        Some(Ground::Secondary) => Some(const { Swatch::new(rgb("#f7fabf")) }),
        Some(Ground::Primary) => Some(const { Swatch::new(rgb("#fcd6a4")) }),
        Some(Ground::Trunk) => Some(const { Swatch::new(rgb("#f9b29c")) }),
        Some(Ground::Motorway) => Some(const { Swatch::new(rgb("#e892a2")) }),
        Some(Ground::Platform) => Some(const { Swatch::new(rgb("#bbbbbb")) }),
        Some(Ground::Tram) => Some(const { Swatch::new(rgb("#6e6e6e")) }),
        Some(Ground::LightRail) => Some(const { Swatch::new(rgb("#666666")) }),
        Some(Ground::Subway) => Some(const { Swatch::new(rgb("#999999")) }),
        Some(Ground::Monorail) => Some(const { Swatch::new(rgb("#777777")) }),
        Some(Ground::Rail) => Some(const { Swatch::new(rgb("#707070")) }),
        Some(Ground::Aeroway) => Some(const { Swatch::new(rgb("#bbbbcc")) }),
        // Minecraft's map color DIRT (minecraft.wiki Map_item_format): Carto paints nothing under the ground.
        Some(Ground::Soil) => Some(const { Swatch::new(rgb("#976d4d")) }),
        // Minecraft's map color SAND, which sandstone is drawn as on a map.
        Some(Ground::Sandstone) => Some(const { Swatch::new(rgb("#f7e9a3")) }),
        // Minecraft's map color STONE.
        Some(Ground::Stone) => Some(const { Swatch::new(rgb("#707070")) }),
        // A sentinel a baked cell can hold.
        None => sentinel(t as u16),
    }
}

/// `object`'s color for a value.
pub const fn object(t: u16) -> Option<Swatch> {
    match Object::of(t) {
        Some(Object::White) => Some(const { Swatch::new(rgb("#f7f7f7")) }),
        Some(Object::Grey) => Some(const { Swatch::new(rgb("#8d8d8d")) }),
        Some(Object::DarkGrey) => Some(const { Swatch::new(rgb("#3f3f3f")) }),
        Some(Object::Black) => Some(const { Swatch::new(rgb("#191919")) }),
        Some(Object::Red) => Some(const { Swatch::new(rgb("#b01313")) }),
        Some(Object::Pink) => Some(const { Swatch::new(rgb("#ff8787")) }),
        Some(Object::Brown) => Some(const { Swatch::new(rgb("#572b00")) }),
        Some(Object::Orange) => Some(const { Swatch::new(rgb("#d44e14")) }),
        Some(Object::Yellow) => Some(const { Swatch::new(rgb("#ffe611")) }),
        Some(Object::Green) => Some(const { Swatch::new(rgb("#60dc1e")) }),
        Some(Object::DarkGreen) => Some(const { Swatch::new(rgb("#206700")) }),
        Some(Object::Cyan) => Some(const { Swatch::new(rgb("#008c94")) }),
        Some(Object::Blue) => Some(const { Swatch::new(rgb("#004791")) }),
        Some(Object::Violet) => Some(const { Swatch::new(rgb("#57039f")) }),
        Some(Object::Magenta) => Some(const { Swatch::new(rgb("#c9036d")) }),
        _ => None,
    }
}
