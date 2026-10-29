// pxl-shader/either.rs

//! The observer of a pass both the eye and the shadow map take (observer.rs
//! `Observer`), chosen by the head the pass is handed: one entry a pass, so
//! a pass is one module whichever casts it (beam.rs, raster.rs).

use crate::beam::Words;
use crate::eye::Eye;
use crate::observer::{Aim, Level, Observer};
use crate::sun::{self, Map, Moved};

/// The observer of a pass both take, read off the head it is handed
/// (pxl-spec-core `sun::MOVED_SIZE`): the shadow map where the head names
/// one, else the eye, whose head is all zero. Each call is its observer's;
/// no walk asks which, so the walk is one in the module whichever casts.
/// Both walk the same world, its looks too, which only what a stop is drawn
/// by holds, and the map's word leaves out (sun.rs `Map::word`).
pub(crate) struct Either<'a> {
    eye: &'a Eye,
    map: Map,
    sun: bool,
}

impl<'a> Either<'a> {
    pub(crate) fn of(eye: &'a Eye, moved: &Moved) -> Either<'a> {
        Either {
            eye,
            map: Map::of(eye, moved),
            sun: sun::size(moved) != 0,
        }
    }

    /// Whether it is the shadow map.
    pub(crate) fn sun(&self) -> bool {
        self.sun
    }
}

/// A point of either lattice (`Either`): the eye's tangents, or the map's
/// slot; the other none.
#[derive(Clone, Copy)]
pub(crate) struct Point {
    pub(crate) tan: [f32; 2],
    pub(crate) slot: [u32; 2],
}

impl Observer for Either<'_> {
    type At = Point;

    fn tally(&self) -> u32 {
        if self.sun {
            self.map.tally()
        } else {
            self.eye.tally()
        }
    }

    fn assets(&self) -> bool {
        if self.sun {
            self.map.assets()
        } else {
            self.eye.assets()
        }
    }

    fn passes(&self) -> [u32; 3] {
        if self.sun {
            self.map.passes()
        } else {
            self.eye.passes()
        }
    }

    fn apart(&self) -> bool {
        if self.sun {
            self.map.apart()
        } else {
            self.eye.apart()
        }
    }

    fn corners(&self) -> [u32; 2] {
        if self.sun {
            self.map.corners()
        } else {
            self.eye.corners()
        }
    }

    fn corner(&self, c: [u32; 2]) -> Aim {
        if self.sun {
            self.map.corner(c)
        } else {
            self.eye.corner(c)
        }
    }

    fn handed(&self, aim: &Aim, t: f32) -> f32 {
        if self.sun {
            self.map.handed(aim, t)
        } else {
            self.eye.handed(aim, t)
        }
    }

    fn line(&self, idx: u32) -> u32 {
        if self.sun {
            self.map.line(idx)
        } else {
            self.eye.line(idx)
        }
    }

    fn origin_of<W: Words + ?Sized>(&self, w: &W, base: u32) -> [i32; 3] {
        if self.sun {
            self.map.origin_of(w, base)
        } else {
            self.eye.origin_of(w, base)
        }
    }

    fn keep_origin<W: Words + ?Sized>(&self, w: &mut W, base: u32, o: [i32; 3]) {
        if self.sun {
            self.map.keep_origin(w, base, o)
        } else {
            self.eye.keep_origin(w, base, o)
        }
    }

    fn cells(&self) -> [u32; 2] {
        if self.sun {
            self.map.cells()
        } else {
            self.eye.cells()
        }
    }

    fn window(&self, c: [u32; 2]) -> ([u32; 2], u32) {
        if self.sun {
            self.map.window(c)
        } else {
            self.eye.window(c)
        }
    }

    fn cell_at(&self, c: [u32; 2]) -> u32 {
        if self.sun {
            self.map.cell_at(c)
        } else {
            self.eye.cell_at(c)
        }
    }

    fn samples(&self) -> [u32; 2] {
        if self.sun {
            self.map.samples()
        } else {
            self.eye.samples()
        }
    }

    fn point(&self, s: [u32; 2]) -> Point {
        if self.sun {
            Point {
                tan: [0.0; 2],
                slot: self.map.point(s),
            }
        } else {
            Point {
                tan: self.eye.point(s),
                slot: [0; 2],
            }
        }
    }

    fn sample_cell(&self, s: [u32; 2]) -> [u32; 2] {
        if self.sun {
            self.map.sample_cell(s)
        } else {
            self.eye.sample_cell(s)
        }
    }

    fn sample_at(&self, s: [u32; 2]) -> u32 {
        if self.sun {
            self.map.sample_at(s)
        } else {
            self.eye.sample_at(s)
        }
    }

    fn sample_before(&self, at: Point) -> ([u32; 2], bool) {
        if self.sun {
            self.map.sample_before(at.slot)
        } else {
            self.eye.sample_before(at.tan)
        }
    }

    fn aim(&self, at: Point) -> Aim {
        if self.sun {
            self.map.aim(at.slot)
        } else {
            self.eye.aim(at.tan)
        }
    }

    fn cell_of(&self, at: Point) -> [u32; 2] {
        if self.sun {
            self.map.cell_of(at.slot)
        } else {
            self.eye.cell_of(at.tan)
        }
    }

    fn laid(&self) -> ([i32; 3], (bool, [f32; 2])) {
        if self.sun {
            self.map.laid()
        } else {
            self.eye.laid()
        }
    }

    fn needs<W: Words + ?Sized>(&self, w: &W, level: Level, at: [u32; 2]) -> bool {
        if self.sun {
            self.map.needs(w, level, at)
        } else {
            self.eye.needs(w, level, at)
        }
    }
}
