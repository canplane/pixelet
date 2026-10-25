// pxl-geo-bake/terrain/pipeline/overture/role.rs

//! What a way is to the road beside it, per Overture's `subclass`; decides
//! width, whether it paints, paint order and which gap is paved.

use crate::terrain::pipeline::overture::between::Rules;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Role {
    #[default]
    Way,
    Sidewalk,
    Crossing,
    Link,
    /// Carto's minor service road: a driveway, a parking aisle.
    Lane,
}

impl Role {
    fn of(subclass: &str) -> Role {
        match subclass {
            "sidewalk" => Role::Sidewalk,
            "crosswalk" | "cycle_crossing" => Role::Crossing,
            "link" => Role::Link,
            "driveway" | "parking_aisle" => Role::Lane,
            _ => Role::Way,
        }
    }

    pub fn at(subclass: Option<&str>, rules: &Rules, at: f64) -> Role {
        rules
            .at(at)
            .and_then(|v| v.as_str())
            .or(subclass)
            .map_or(Role::Way, Role::of)
    }
}
