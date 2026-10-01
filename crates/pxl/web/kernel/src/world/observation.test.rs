// pxl-kernel/world/observation.test.rs

use std::collections::HashSet;

use pxl_spec::space::observation::{ByBounds, Paged, Storage as _};
use pxl_spec::space::v3::V3;

use super::*;
use crate::world::fixture;
use crate::world::observer::{Lens, Sight};

/// What the estimator from the tiles' footprints requires of `world` seen
/// by `o` at `tau`, as the pages of the pyramid's storage.
fn footprint_pages(o: &Observation, world: &fixture::World, tau: Tau) -> HashSet<LevelPage> {
    let mut sight = Sight::default();
    sight.observe(o, world, true, &[]);
    let mut out = HashSet::new();
    ByFootprints { tau, sight: &sight }.requirements(o, world, &mut |r| Paged.pages(&r, &mut |p| _ = out.insert(p)));
    out
}

const GROUND: f64 = 10.5;

