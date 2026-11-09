// pxl-core/domain/view/demand.test.rs
//
// A layer's demand, kept from turn to turn as an eye moves, is held to a
// demand that observes each eye afresh.

use super::*;
use crate::domain::layer::Base;
use crate::sys::serial;
use pxl_spec::bake::publication::BaseLayer;
use pxl_spec::bake::tile::{Occupancy, Tile};
use pxl_spec::space::camera::eye::Frustum;
use pxl_spec::space::coord::zoom::units;
use pxl_spec::space::layer::lattice::demand_dist;

/// A layer the view asks of as it asks of the ground, with a tile cache of
/// its own. Its stream is one no shard serves alone, so no ask is answered
/// as absent and every ask stands.
#[derive(Clone)]
struct Probe;

static PROBE_TILES: Global<TileCache<Probe>> = Global::new();
static PROBE: Global<Demand<Probe>> = Global::new();

impl Tile for Probe {
    const LAYER: BaseLayer = BaseLayer::earth;
    const LEAF: Zoom = LEAF_ZOOM;

    fn occupancy(&self) -> Occupancy {
        Occupancy::ZERO
    }
}

impl Base for Probe {
    fn tile_cache() -> &'static mut TileCache<Probe> {
        PROBE_TILES.get(TileCache::default)
    }

    fn receive(_: Quadrant, _: u32, _: Packet, _: &mut PhaseList) -> Result<Probe, Exit> {
        Err(Failure::from(Errno::ENOTRECOVERABLE).into())
    }
}

impl ViewLayer for Probe {
    fn demand() -> &'static mut Demand<Probe> {
        PROBE.get(Demand::default)
    }
}

/// The observation from `eye` with sea level under it. Where `east` is
/// given, the view sees only what lies east of it.
fn at(eye: [f64; 3], east: Option<f64>, top: Zoom) -> Observation {
    Observation {
        eye,
        frustum: east.map(|x| {
            let mut planes = [0.0; 20];
            planes[..4].copy_from_slice(&[1.0, 0.0, 0.0, -x]);
            Frustum { planes }
        }),
        demand_dist: demand_dist(GROUND, top),
        ground: 0.0,
    }
}

/// An eye drifts within its tiles, turns, crosses into new tiles, climbs and
/// comes down, and a tile arrives on the way. On every turn, the wants the
/// kept demand declares, their order and its floor equal what a fresh
/// demand observing the same eye finds.
#[test]
fn turns_ask_what_a_fresh_observation_asks() {
    let _g = serial();
    let top = LEAF_ZOOM - 6;
    let (x0, y0, side) = Quadrant::new(top, [3, -2]).rect();
    let leaf = units(LEAF_ZOOM);
    let mut eyes = Vec::new();
    for k in 0..40 {
        let k = f64::from(k);
        // The eye steps within a leaf, then across leaves, then across the
        // top's tiles.
        let step = if k < 10.0 {
            leaf / 64.0
        } else if k < 25.0 {
            leaf / 3.0
        } else {
            side / 5.0
        };
        let at_k = [x0 + side / 2.0 + k * step, y0 + side / 3.0 - k * step / 2.0];
        let height = if (30.0..35.0).contains(&k) {
            side * (k - 29.0)
        } else {
            2.0
        };
        eyes.push([at_k[0], at_k[1], height]);
        eyes.push([at_k[0], at_k[1], height]);
    }
    let mut turns = Vec::new();
    for (k, eye) in eyes.iter().enumerate() {
        let east = (k % 3 != 0).then_some(eye[0] - k as f64 * leaf);
        turns.push(at(*eye, east, top));
    }
    let dst = Quadrant::new(LEAF_ZOOM, [0, 0]);
    let reused = Probe::demand();
    let mut changed = Vec::new();
    for (k, o) in turns.iter().enumerate() {
        let dst = (k >= 20).then_some(dst);
        reused.turn(Some(*o), dst, &[], top);
        let mut fresh = Demand::<Probe>::default();
        fresh.turn(Some(*o), dst, &[], top);
        assert_eq!(
            reused.wants, fresh.wants,
            "turn {k}: the wants and their order"
        );
        assert_eq!(reused.floor, fresh.floor, "turn {k}: the floor");
        assert_eq!(reused.observation, fresh.observation, "turn {k}");
        // A tile arrives, as one would between turns.
        if k == 50 {
            let p = Quadrant::of_id(reused.wants[reused.wants.len() / 2].0);
            Probe::tile_cache().set(p, Probe);
        }
        // The demand is told what moved, as the turn tells it (turn.rs
        // `changes_step`).
        Probe::tile_cache().take_changed(&mut changed);
        if !changed.is_empty() {
            reused.on_changed();
        }
    }
}
