// pxl-core/domain/query/decor.rs

//! Decor as the world answers it: what the ground generates, evaluated cell
//! by cell as the frame generates it (`generated_at`) from the ground's cells
//! around the cell, and what objects place. Whatever fills a cell is an asset,
//! so a leaf cell the layers leave void is the decor whose box claims it
//! (`World::with_decor`), among those the asker sees. A decor stands in the leaf cells alone: a fold holds none of it. The catalog
//! is spec/spec/asset/decor.json. The scene draws the decor's models as they
//! arrive (domain/scene/decor/asset.rs).

use pxl_content::generation::{CLAIM_EXTENT, TALLEST};
use pxl_content::scatter::{cnt, ROOTS};
use pxl_engine::answer::{In, Kind, BOX_WHOLE, PLACED_ASSET};
use pxl_engine::ground::Extent;
use pxl_engine::object;
use pxl_global::Global;
use pxl_spec::asset::cube::Sentinel;
use pxl_spec::asset::decor::{Decor, DECORS};
use pxl_spec::space::coord::aabb::Aabb;
use pxl_spec::space::coord::q3::Q3;
use pxl_spec::space::coord::world::idx_of;
use pxl_spec::space::coord::zoom::{units, Zoom, UNIT_ZOOM};
use pxl_spec::space::hierarchy::tree::Node;
use pxl_spec::space::layer::lattice::GROUND;

use super::node::At;
use super::scatter::{generated_at, DecorWorld};
use super::world::World;
use crate::domain::layer::instance::Instance;
use crate::domain::layer::object::asset::Asset;
use crate::domain::layer::object::meta::decor_instance;

/// The farthest any decor's box in decor.json reaches from its min corner,
/// however it turns. It is computed once.
pub fn box_extent() -> f64 {
    static EXTENT: Global<f64> = Global::new();
    *EXTENT.get(|| DECORS.iter().map(|d| d.box_extent()).fold(0.0, f64::max))
}

/// The finest zoom whose nodes are as wide as the farthest a placed decor's
/// corner lies from a cell its box claims, on any axis: the corner lies in
/// that node of the cell, or in the node before it on each axis.
const CLAIM_ZOOM: Zoom = {
    let reach = if CLAIM_EXTENT[0] > CLAIM_EXTENT[2] {
        CLAIM_EXTENT[0]
    } else {
        CLAIM_EXTENT[2]
    };
    let mut z = UNIT_ZOOM;
    while (1 << (UNIT_ZOOM - z)) < reach {
        z -= 1;
    }
    z
};

impl World {
    /// Hands what the ground's scatter stands on the leaf `cell`, decor by
    /// decor, to `each` until `each` returns true. Each instance stands on
    /// its column's surface in the base (`World::column_surface`). Nothing is
    /// handed where the world does not hold the column.
    fn generated_in(&self, cell: [i32; 2], mut each: impl FnMut(&Decor, Instance) -> bool) {
        DECORS.iter().any(|decor| {
            generated_at(&mut DecorWorld::new(self), decor, cell, false)
                .is_some_and(|(i, _)| each(decor, i))
        });
    }

    /// Every decor the ground's scatter generates on the leaves whose box
    /// shares room with `space`, whether or not its column surface is there
    /// now. This includes what an erase took away.
    pub fn generated_over(&self, space: &Aabb) -> Vec<Instance> {
        let (r, unit) = (box_extent(), units(GROUND.leaf_cell_zoom()));
        let cells = |k: usize| {
            libm::floor((space.min[k] - r) / unit) as i32..libm::ceil(space.max[k] / unit) as i32
        };
        let mut out = Vec::new();
        for y in cells(1) {
            for x in cells(0) {
                self.generated_in([x, y], |_, i| {
                    if claim(&i).is_some_and(|c| c.shares(*space)) {
                        out.push(i);
                    }
                    false
                });
            }
        }
        out
    }

    /// The instance of `decor` the ground generates standing in the leaf
    /// cell that `at` lies in. None where it generates nothing there or the
    /// clip levels do not hold the cell.
    pub fn generated_instance(&self, decor: &Decor, at: &Q3) -> Option<Instance> {
        let [x, y, z] = idx_of(GROUND.leaf_cell_zoom(), at);
        let mut met = None;
        self.generated_in([x, y], |d, i| {
            met =
                Some(i).filter(|i| d.id == decor.id && i.generated_in().is_some_and(|c| c[2] == z));
            met.is_some()
        });
        met
    }

    /// `i`, what the layers answer of `at`'s node asked down to `near`
    /// (`World::layer_node`), with the decor standing in it. `relief` is the
    /// ground's bound the layers read over the node. Where `near` is coarser
    /// than a leaf cell, no decor stands. A cell at the leaves' zoom or finer
    /// that the layers leave void is the decor whose box claims it, if any
    /// (`decor_at`) and that the asker sees (`sees`), and carries `PLACED_ASSET`
    /// where it is placed. A node a
    /// decor may reach into (`may_reach`, or at those zooms a decor claiming
    /// its cell) is split whole, as a decor stands apart from the box of what
    /// the layers split on.
    pub(super) fn with_decor(
        &self,
        at: &At,
        near: Zoom,
        relief: Option<Extent>,
        i: In<[u32; 2]>,
        sees: &mut dyn FnMut(&Instance) -> bool,
    ) -> In<[u32; 2]> {
        let zoom = at.node.0;
        let leaf = GROUND.leaf_cell_zoom();
        let void = i.kind == Kind::Cell && i.asset[0] == Sentinel::Void as u32;
        if near < leaf || !(void || i.kind == Kind::Split) {
            return i;
        }
        let split = In {
            kind: Kind::Split,
            asset: BOX_WHOLE,
        };
        if zoom < leaf {
            return if self.may_reach(at.node, relief) {
                split
            } else {
                i
            };
        }
        match self.decor_over(at.node, sees) {
            Some(d) if void => In {
                kind: Kind::Cell,
                asset: [
                    d.id | if d.generated_in().is_none() {
                        PLACED_ASSET
                    } else {
                        0
                    },
                    0,
                ],
            },
            Some(_) => split,
            None => i,
        }
    }

    /// Whether a decor's box may reach into the node `(zoom, c)`, coarser
    /// than a leaf cell; X is on the asker's lap.
    ///
    /// What the ground generates stands on the columns its box covers, each
    /// filled under its base. So one that reaches into the node stands on the
    /// node's own columns: its cells lie no higher than a cell over the
    /// highest the ground and the ways reach under the node (`relief`, in
    /// leaf cells) and as tall again as the tallest decor. Its corner is a
    /// cell holding a candidate, at most its decor's widest less one before
    /// the node, so a decor whose counts there are none is out of reach
    /// (pxl-content `scatter::cnt`, read from its root down, `ROOTS`).
    ///
    /// What is placed stands anywhere, so it is found by where it is held:
    /// a decor's corner lies at most `CLAIM_EXTENT` before a cell its box
    /// claims, so in the node at `CLAIM_ZOOM` or the node's own zoom over the
    /// node, or the node before that on each axis. Air placed there counts as
    /// held, which only makes a ray split more.
    fn may_reach(&self, (zoom, c): Node<3>, relief: Option<Extent>) -> bool {
        let bottom = i64::from(c[2]) << (GROUND.leaf_cell_zoom() - zoom);
        let grows = relief
            .is_some_and(|[ground, ways]| bottom <= i64::from(ground[1].max(ways[1]) + TALLEST))
            && (0..DECORS.len()).any(|k| may_stand(k, (zoom, [c[0], c[1]])));
        let z = zoom.min(CLAIM_ZOOM);
        let a = c.map(|v| v >> (zoom - z));
        let lanes = self.object.columns();
        grows
            || (0..8).any(|k: i32| {
                let n = [a[0] - (k & 1), a[1] - (k >> 1 & 1), a[2] - (k >> 2 & 1)];
                // Y and Z lie under the world's one root alone; X wraps
                // around it.
                (-1..=0).contains(&(n[1] >> z))
                    && (-1..=0).contains(&(n[2] >> z))
                    && object::cursor(&lanes, object::ROOT, z, n).holds()
            })
    }

    /// The decor the asker sees whose box claims the leaf cell over the node
    /// `(zoom, c)`, which is a leaf cell or finer (`decor_at`).
    fn decor_over(
        &self,
        (zoom, c): Node<3>,
        sees: &mut dyn FnMut(&Instance) -> bool,
    ) -> Option<Instance> {
        self.decor_at(c.map(|v| v >> (zoom - GROUND.leaf_cell_zoom())), sees)
    }

    /// The decor whose box claims the leaf cell `cell`, placed or generated;
    /// X is on the asker's lap. The ground generates no decor whose box
    /// meets a placed one's, and no two whose boxes meet, so at most one
    /// claims it. `sees` is told of that one and says whether the asker has it.
    fn decor_at(
        &self,
        cell: [i32; 3],
        sees: &mut dyn FnMut(&Instance) -> bool,
    ) -> Option<Instance> {
        let middle = cell.map(|v| f64::from(v) + 0.5);
        let claims = |i: &Instance| cell_box(i).is_some_and(|b| b.holds(middle));
        let near = Aabb {
            min: std::array::from_fn(|k| f64::from(cell[k] - CLAIM_EXTENT[k])),
            max: cell.map(|v| f64::from(v + 1)),
        };
        let mut met = None;
        self.object.entries_in(&near, &mut |e| {
            if met.is_none() {
                met = decor_instance(e).filter(|i| claims(i) && sees(i));
            }
        });
        // A count a little coarser tells most decors apart at once, so a
        // decor's corners are asked one by one only under a count that holds
        // one.
        let over = (GROUND.leaf_cell_zoom() - 2, [cell[0] >> 2, cell[1] >> 2]);
        met.or_else(|| {
            DECORS.iter().enumerate().find_map(|(k, d)| {
                let w = d.row.widest() as i32;
                may_stand(k, over).then_some(())?;
                (0..w * w).find_map(|n| {
                    let corner = [cell[0] - n % w, cell[1] - n / w];
                    generated_at(&mut DecorWorld::new(self), d, corner, true)
                        .filter(|(i, generates)| *generates && claims(i) && sees(i))
                        .map(|(i, _)| i)
                })
            })
        })
    }
}

/// Whether the `k`-th decor's counts hold anything over the columns at
/// `zoom` where a box as wide as its widest may stand and reach the column
/// `c`. A zoom coarser than its counts' root does not tell (pxl-content
/// `scatter::cnt`, `ROOTS`).
fn may_stand(k: usize, (zoom, c): (Zoom, [i32; 2])) -> bool {
    let s = GROUND.leaf_cell_zoom() - zoom;
    let back = (DECORS[k].row.widest() as i32 - 1 + (1 << s) - 1) >> s;
    zoom < ROOTS[k]
        || (c[1] - back..=c[1]).any(|y| (c[0] - back..=c[0]).any(|x| cnt(k, zoom, [x, y]) > 0))
}

pub(super) fn claim(i: &Instance) -> Option<Aabb> {
    Some(Asset::of(i.id).decor()?.claim(i.at, &i.meta))
}

/// The box `i` claims as cells (decor.json), every cell of it the decor's:
/// `claim` raised to the cell it stands in, where a generated one's base sinks
/// into the cell under it as its model is drawn on a top kept short of its
/// cube.
fn cell_box(i: &Instance) -> Option<Aabb> {
    let c = claim(i)?;
    let rise = i.generated_in().map_or(0.0, |g| f64::from(g[2]) - c.min[2]);
    Some(c.moved([0.0, 0.0, rise]))
}
