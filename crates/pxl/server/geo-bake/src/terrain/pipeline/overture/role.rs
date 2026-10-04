// pxl-geo-bake/terrain/pipeline/overture/role.rs

//! What a way is to the road beside it, per Overture's `subclass`; decides
//! width, whether it paints, paint order and which gap is paved.

use crate::terrain::pipeline::overture::between::text_at;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Role {
    #[default]
    Way,
    Sidewalk,
    Crossing,
    Link,
    /// A service road's own lane: a driveway, a parking aisle, an alley.
    Lane,
}

impl Role {
    fn of(subclass: &str) -> Role {
        match subclass {
            "sidewalk" => Role::Sidewalk,
            "crosswalk" | "cycle_crossing" => Role::Crossing,
            "link" => Role::Link,
            "driveway" | "parking_aisle" | "alley" => Role::Lane,
            _ => Role::Way,
        }
    }

    pub fn at(subclass: Option<&str>, rules: Option<&str>, at: f64) -> Role {
        text_at(rules, at).as_deref().or(subclass).map_or(Role::Way, Role::of)
    }
}
