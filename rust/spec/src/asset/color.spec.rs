// pxl-spec/asset/color.spec.rs
//
// GENERATED from spec/spec/asset/color.json5 (pxl-tool spec/asset.rs)
//
// The color of every ground cube and every placed color: what a cube *is* to
// the eye when no texture is drawn over it — everything drawn with the
// pack off reads this, the coarse cells included, which summarize it
// the way they summarize the tiles with the pack on
// (pxl-render paint.wgsl). A texture pack decorates these; it
// does not replace them.

use super::Swatch;
use super::cube::{Ground, Object, Sentinel};

/// `sentinel`'s color for a value.
pub const fn sentinel(t: u16) -> Option<Swatch> {
    match Sentinel::of(t) {
        // Bedrock, where the world holds no more: a dark gray, so it reads as the world's end against the night's black and the day's haze past it alike, rather than as whatever drew last.
        Some(Sentinel::Unknown) => Some(Swatch::new(5658198)),
        _ => None,
    }
}

/// `ground`'s color for a value.
pub const fn ground(t: u8) -> Option<Swatch> {
    match Ground::of(t) {
        Some(Ground::Land) => Some(Swatch::new(15921129)),
        Some(Ground::BareGround) => Some(Swatch::new(15656412)),
        Some(Ground::Sand) => Some(Swatch::new(16116166)),
        Some(Ground::Beach) => Some(Swatch::new(16773562)),
        Some(Ground::Heath) => Some(Swatch::new(14080415)),
        Some(Ground::Scrub) => Some(Swatch::new(13162411)),
        Some(Ground::Grass) => Some(Swatch::new(13495216)),
        Some(Ground::Park) => Some(Swatch::new(13171404)),
        Some(Ground::Leisure) => Some(Swatch::new(14679266)),
        Some(Ground::Pitch) => Some(Swatch::new(8970430)),
        Some(Ground::Forest) => Some(Swatch::new(11391390)),
        Some(Ground::Orchard) => Some(Swatch::new(11460515)),
        Some(Ground::Allotments) => Some(Swatch::new(13230527)),
        Some(Ground::Farmland) => Some(Swatch::new(15659221)),
        Some(Ground::Farmyard) => Some(Swatch::new(16112826)),
        Some(Ground::Campsite) => Some(Swatch::new(14612160)),
        Some(Ground::Cemetery) => Some(Swatch::new(11193263)),
        Some(Ground::Residential) => Some(Swatch::new(14737375)),
        Some(Ground::Garages) => Some(Swatch::new(14671310)),
        Some(Ground::Retail) => Some(Swatch::new(16766673)),
        Some(Ground::Commercial) => Some(Swatch::new(15915737)),
        Some(Ground::Industrial) => Some(Swatch::new(15457256)),
        Some(Ground::Power) => Some(Swatch::new(14863326)),
        Some(Ground::Landfill) => Some(Swatch::new(11974034)),
        Some(Ground::Quarry) => Some(Swatch::new(12960707)),
        Some(Ground::SocietalAmenities) => Some(Swatch::new(16777189)),
        Some(Ground::PlaceOfWorship) => Some(Swatch::new(13684944)),
        Some(Ground::TransportationArea) => Some(Swatch::new(15329250)),
        Some(Ground::Parking) => Some(Swatch::new(15658734)),
        Some(Ground::Apron) => Some(Swatch::new(14342880)),
        Some(Ground::Glacier) => Some(Swatch::new(14544108)),
        Some(Ground::Water) => Some(Swatch::new(11195359)),
        Some(Ground::Dam) => Some(Swatch::new(11382189)),
        Some(Ground::Breakwater) => Some(Swatch::new(11184810)),
        Some(Ground::Building) => Some(Swatch::new(14274761)),
        Some(Ground::BuildingMajor) => Some(Swatch::new(12891819)),
        Some(Ground::Footway) => Some(Swatch::new(16416882)),
        Some(Ground::Cycleway) => Some(Swatch::new(255)),
        Some(Ground::Bridleway) => Some(Swatch::new(32768)),
        Some(Ground::Track) => Some(Swatch::new(10053120)),
        Some(Ground::Pedestrian) => Some(Swatch::new(14540264)),
        Some(Ground::LivingStreet) => Some(Swatch::new(15592941)),
        Some(Ground::ResidentialRoad) => Some(Swatch::new(16777215)),
        Some(Ground::Road) => Some(Swatch::new(14540253)),
        Some(Ground::Secondary) => Some(Swatch::new(16251583)),
        Some(Ground::Primary) => Some(Swatch::new(16570020)),
        Some(Ground::Trunk) => Some(Swatch::new(16364188)),
        Some(Ground::Motorway) => Some(Swatch::new(15241890)),
        Some(Ground::Platform) => Some(Swatch::new(12303291)),
        Some(Ground::Tram) => Some(Swatch::new(7237230)),
        Some(Ground::LightRail) => Some(Swatch::new(6710886)),
        Some(Ground::Subway) => Some(Swatch::new(10066329)),
        Some(Ground::Monorail) => Some(Swatch::new(7829367)),
        Some(Ground::Rail) => Some(Swatch::new(7368816)),
        Some(Ground::Aeroway) => Some(Swatch::new(12303308)),
        // Minecraft's map color DIRT (minecraft.wiki Map_item_format): Carto paints nothing under the ground.
        Some(Ground::Soil) => Some(Swatch::new(9923917)),
        // Minecraft's map color SAND, which sandstone is drawn as on a map.
        Some(Ground::Sandstone) => Some(Swatch::new(16247203)),
        // Minecraft's map color STONE.
        Some(Ground::Stone) => Some(Swatch::new(7368816)),
        // A sentinel a baked cell can hold.
        None => sentinel(t as u16),
    }
}

/// `object`'s color for a value.
pub const fn object(t: u16) -> Option<Swatch> {
    match Object::of(t) {
        Some(Object::White) => Some(Swatch::new(16250871)),
        Some(Object::Grey) => Some(Swatch::new(9276813)),
        Some(Object::DarkGrey) => Some(Swatch::new(4144959)),
        Some(Object::Black) => Some(Swatch::new(1644825)),
        Some(Object::Red) => Some(Swatch::new(11539219)),
        Some(Object::Pink) => Some(Swatch::new(16746375)),
        Some(Object::Brown) => Some(Swatch::new(5712640)),
        Some(Object::Orange) => Some(Swatch::new(13913620)),
        Some(Object::Yellow) => Some(Swatch::new(16770577)),
        Some(Object::Green) => Some(Swatch::new(6347806)),
        Some(Object::DarkGreen) => Some(Swatch::new(2123520)),
        Some(Object::Cyan) => Some(Swatch::new(35988)),
        Some(Object::Blue) => Some(Swatch::new(18321)),
        Some(Object::Violet) => Some(Swatch::new(5702559)),
        Some(Object::Magenta) => Some(Swatch::new(13173613)),
        _ => None,
    }
}
