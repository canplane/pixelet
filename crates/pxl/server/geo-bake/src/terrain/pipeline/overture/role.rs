// pxl-geo-bake/terrain/pipeline/overture/role.rs

//! What a way is to the road beside it, as Overture's `subclass` says:
//! the road itself, its sidewalk, a crossing over it, a slip road onto it,
//! or a lane off it. A sidewalk and a road are two lines in the data and
//! one paved strip on the ground; a crossing is a line over a road the road
//! already shows. The role decides a way's width (width.rs), whether it is
//! painted at all, what it paints over (policy.rs `paint_order`), and which
//! gap beside it is paved (corridor.rs).

use crate::terrain::pipeline::overture::between::text_at;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Role {
    #[default]
    Way,
    /// A footway along a road's side.
    Sidewalk,
    /// A footway or a cycleway across a road.
    Crossing,
    /// A slip road: one lane between two roads.
    Link,
    /// A service road's own lane — a driveway, a parking aisle, an alley.
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

    /// A way's role at fraction `at` of its length: the ranged rule that
    /// holds there, else the way's own subclass.
    pub fn at(subclass: Option<&str>, rules: Option<&str>, at: f64) -> Role {
        text_at(rules, at).as_deref().or(subclass).map_or(Role::Way, Role::of)
    }
}
