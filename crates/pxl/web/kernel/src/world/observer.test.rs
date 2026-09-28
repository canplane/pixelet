// pxl-kernel/world/observer.test.rs
//
// The observer's laws: a lens is the screen its frustum was made from, as
// the view makes it; the tiles cover the screen, none wider than a tile; an
// error projects as its size over its distance on each axis; a footprint
// touches what meets it; the estimator reads residency and leaves it as it
// found it; and what was seen is cast again only where it is stale.

use pxl_spec::bake::constant::ELE_PADDED_SPAN;
use pxl_spec::bake::format::{Ground, GroundChannelKey, Terrain};
use pxl_spec::bake::pack::with_channel;
use pxl_spec::cell::cube_types;
use pxl_spec::space::mat4::{mat4, mat4_frustum_planes, mat4_look_at, mat4_mul, mat4_perspective};
use pxl_spec::space::observation::Frustum;
use pxl_spec::space::pvf3::ivec;
use pxl_spec::space::pyramid::{LevelPage, PAGE_CELLS, TOP_LEVEL};

use super::*;
use crate::global::serial;
use crate::layer::object::store::Store;
use crate::world::paging::{Candidate, Paging, Victim};

/// An observation through the frustum a view with a vertical field of
/// `fov_deg` and `aspect` makes looking from `eye` at `target`, as
/// world/frame/view.rs makes one.
fn viewed(eye: V3, target: V3, fov_deg: f64, aspect: f64) -> Observation {
    let (mut view, mut proj, mut vp) = (mat4(), mat4(), mat4());
    mat4_look_at(&mut view, eye, target, [0.0, 0.0, 1.0]);
    mat4_perspective(&mut proj, fov_deg.to_radians(), aspect, 0.5, 50_000.0);
    mat4_mul(&mut vp, &proj, &view);
    let mut planes = [0.0; 24];
    mat4_frustum_planes(&mut planes, &vp);
    Observation {
        eye,
        frustum: Some(Frustum { planes }),
        far: 40_000.0,
        top: TOP_LEVEL,
        planet: f64::INFINITY,
        ground: 0.0,
    }
}

fn dot(a: V3, b: V3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// The lens is the view's: looking where the view looks, its screen as wide
/// and as tall as the projection made it.
#[test]
fn a_lens_is_the_screen_its_frustum_was_made_from() {
    for (eye, target, fov, aspect) in [([0.0, 0.0, 10.0], [100.0, 0.0, 0.0], 50.0, 16.0 / 9.0), ([5.0, -3.0, 400.0], [-20.0, 60.0, 0.0], 70.0, 9.0 / 19.5)] {
        let lens = Lens::of(&viewed(eye, target, fov, aspect)).expect("a screen");
        let look = unit(sub(target, eye)).unwrap();
        assert!(dot(lens.forward, look) > 1.0 - 1e-9, "{:?} looks elsewhere than {look:?}", lens.forward);
        let tan_up = (fov / 2.0).to_radians().tan();
        assert!((lens.tan[1] - tan_up).abs() < 1e-9 && (lens.tan[0] - tan_up * aspect).abs() < 1e-9, "{:?}", lens.tan);
        assert!(dot(lens.right, lens.forward).abs() < 1e-9 && dot(lens.up, lens.forward).abs() < 1e-9 && lens.up[2] > 0.0);
    }
    let around = Observation {
        frustum: None,
        ..viewed([0.0; 3], [1.0, 0.0, 0.0], 50.0, 1.0)
    };
    assert_eq!(Lens::of(&around), None, "a screen for an observer that samples every direction");
}

/// The tiles' corners run from one corner of the screen to the other, no
/// two neighbours farther apart than a tile, and the grid is the same at
/// every quality: it is the estimator's, not the cell's.
#[test]
fn the_tiles_cover_the_screen_and_none_is_wider_than_a_tile() {
    let lens = Lens::of(&viewed([0.0, 0.0, 30.0], [100.0, 20.0, 0.0], 50.0, 16.0 / 9.0)).unwrap();
    let grid = Grid::of(lens);
    let [nx, ny] = grid.across();
    assert_eq!(grid.count(), nx * ny);
    let screen = |sx: f64, sy: f64| unit(std::array::from_fn(|k| lens.forward[k] + sx * lens.tan[0] * lens.right[k] + sy * lens.tan[1] * lens.up[k])).unwrap();
    for (corner, at) in [([0, 0], (-1.0, -1.0)), ([nx, 0], (1.0, -1.0)), ([0, ny], (-1.0, 1.0)), ([nx, ny], (1.0, 1.0))] {
        assert!(dot(grid.corner(corner), screen(at.0, at.1)) > 1.0 - 1e-12, "{corner:?} is not the screen's corner");
    }
    for j in 0..=ny {
        for i in 0..=nx {
            for next in [[i + 1, j], [i, j + 1]].into_iter().filter(|c| c[0] <= nx && c[1] <= ny) {
                let apart = dot(grid.corner([i, j]), grid.corner(next)).clamp(-1.0, 1.0).acos();
                assert!(apart <= param::OBSERVED_TILE + 1e-9, "[{i}, {j}] is {apart} from {next:?}");
            }
        }
    }
}

/// An error projects as its size over its distance on the axis it lies
/// across: a cell's side seen square on spans its side over the distance
/// on both axes; seen at a grazing angle, its depth projects small up the
/// screen and a height whole.
#[test]
fn an_error_projects_its_size_over_its_distance_on_each_axis() {
    let down = Lens::of(&viewed([0.0, 0.0, 100.0], [0.0, 1e-6, 0.0], 50.0, 1.0)).unwrap();
    let [across, up] = down.error([0.0, 0.0, 0.0], 4.0, 0.0);
    assert!((across - 0.04).abs() < 1e-6 && (up - 0.04).abs() < 1e-6, "a cell seen from above: {across}, {up}");
    let level = Lens::of(&viewed([0.0, 0.0, 2.0], [100.0, 0.0, 2.0], 50.0, 1.0)).unwrap();
    let [across, up] = level.error([1_000.0, 0.0, 0.0], 4.0, 0.0);
    assert!((across - 0.004).abs() < 1e-6, "a cell seen across: {across}");
    assert!(up < 0.004 * 0.01, "a cell's depth at a grazing angle: {up}");
    let [_, up] = level.error([1_000.0, 0.0, 0.0], 0.0, 4.0);
    assert!((up - 0.004).abs() < 1e-5, "a height at a grazing angle: {up}");
    assert_eq!(level.error([0.0, 0.0, 2.0], 1.0, 1.0), [f64::INFINITY; 2], "an error at the eye");
}

/// A footprint touches a box that meets its outline and its heights, and
/// none that misses either.
#[test]
fn a_footprint_touches_what_meets_its_outline_and_heights() {
    let at = |x: f64, y: f64, z: f64| ([x, y, z], 1.0);
    let f = Footprint::of([0.0, 0.0, 10.0], [at(10.0, -5.0, 0.0), at(10.0, 5.0, 0.0), at(100.0, -50.0, 2.0), at(100.0, 50.0, 2.0)]);
    assert!(f.touches((40.0, -1.0, 2.0), (0.0, 1.0)), "a box inside");
    assert!(f.touches((95.0, 45.0, 20.0), (1.0, 5.0)), "a box over a corner");
    assert!(!f.touches((40.0, 30.0, 2.0), (0.0, 1.0)), "a box beside the wedge");
    assert!(!f.touches((40.0, -1.0, 2.0), (3.0, 9.0)), "a box over its heights");
    assert!(!f.touches((0.0, -1.0, 2.0), (0.0, 1.0)), "a box before it");
}

/// Gives up no frame: the fixture holds fewer pages than there are.
struct Keep;

impl Victim for Keep {
    fn victim(&self, _: &[Candidate]) -> Option<usize> {
        None
    }
}

/// The open ocean, as the bake makes it.
fn ocean() -> Terrain {
    let span = PAGE_CELLS as usize;
    let padded = ELE_PADDED_SPAN as usize;
    let cells = vec![with_channel(0, GroundChannelKey::LAND, cube_types::land::sea); span * span];
    Terrain::leaf(vec![0; padded * padded], Ground { cells: cells.into() })
}

/// The estimator only reads: every tile of a screen over resident ground
/// sees it, and residency is as it was, nothing asked for, held or leased.
#[test]
fn the_estimator_never_changes_residency() {
    let _g = serial();
    let (paging, pages) = resident();
    let store = Store::new();
    let o = over(&pages, [0.0; 3]);
    let before = (paging.revision(), paging.frames_used(), paging.leases().count());
    let observer = EdgeRays(Access::new(&paging, &store));
    let grid = Grid::of(Lens::of(&o).unwrap());
    let mut seen = 0;
    observer.observe(&grid, o.far, &|_| true, &mut |_, f| {
        if let Some(f) = f {
            assert!(f.near > 0.0 && f.near < o.far, "{f:?} is not on the ocean");
            seen += 1;
        }
    });
    assert!(seen > 0, "nothing seen of resident ground");
    assert_eq!(before, (paging.revision(), paging.frames_used(), paging.leases().count()));
    for &p in &pages {
        assert!(paging.asked(p).is_none(), "an observation asked for {p:?}");
    }
}

/// Four pages of open ocean, resident.
fn resident() -> (Paging<Terrain>, Vec<LevelPage>) {
    let pages: Vec<LevelPage> = (0..2).flat_map(|x| (0..2).map(move |y| LevelPage::new(0, ivec::gather(40 + x, -20 + y, 0)))).collect();
    let mut paging: Paging<Terrain> = Paging::default();
    for &p in &pages {
        paging.admit(p, &Keep, |_| Ok::<_, ()>(ocean())).unwrap();
    }
    (paging, pages)
}

/// Looking down over `pages`, the eye moved by `by`.
fn over(pages: &[LevelPage], by: V3) -> Observation {
    let (x0, y0, side) = pages[0].rect();
    let eye = [x0 + side + by[0], y0 + side + by[1], 60.0 + by[2]];
    viewed(eye, [x0 + side * 1.5 + by[0], y0 + side * 1.3 + by[1], 0.0], 50.0, 16.0 / 9.0)
}

/// What was seen is reused while it is not stale, and cast again where it
/// is: at rest nothing is cast; a small move casts nothing; a turn past the
/// share of a tile casts every tile; ground whose residency changed under
/// some tiles' rays casts those, and a footprint reused is the one it was.
#[test]
fn a_tile_is_cast_again_only_where_it_is_stale() {
    let _g = serial();
    let (paging, pages) = resident();
    let store = Store::new();
    let observer = EdgeRays(Access::new(&paging, &store));
    let o = over(&pages, [0.0; 3]);
    let tiles = Grid::of(Lens::of(&o).unwrap()).count();
    let mut sight = Sight::default();
    let full = sight.observe(&o, &observer, false, &[]);
    assert!(full >= tiles, "a first sight cast {full} rays for {tiles} tiles");
    let first: Vec<Footprint> = sight.footprints().cloned().collect();
    assert_eq!(sight.observe(&o, &observer, false, &[]), 0, "cast again at rest");
    assert_eq!(sight.footprints().cloned().collect::<Vec<_>>(), first, "a footprint at rest changed");
    assert_eq!(sight.observe(&over(&pages, [0.01, 0.0, 0.0]), &observer, false, &[]), 0, "cast again for a small move");
    let turned = over(&pages, [0.0, 0.0, 0.0]);
    let turned = Observation {
        frustum: viewed(turned.eye, [turned.eye[0] + 10.0, turned.eye[1] - 40.0, 0.0], 50.0, 16.0 / 9.0).frustum,
        ..turned
    };
    assert_eq!(sight.observe(&turned, &observer, false, &[]), full, "a turn past a share of a tile left a tile uncast");
    let nearest = sight.footprints().min_by(|a, b| a.near.total_cmp(&b.near)).expect("something seen").reach;
    let under_some = [(nearest.2 - 0.25, nearest.3 - 0.25, 0.1)];
    let some = sight.observe(&turned, &observer, false, &under_some);
    assert!(some > 0 && some < full, "residency moved under some tiles cast {some} of {full}");
}
