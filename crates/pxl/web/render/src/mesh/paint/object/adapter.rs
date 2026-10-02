// pxl-render/mesh/paint/object/adapter.rs

//! Object store records painted into a chunk volume, with the eraser's carve preview.

use std::collections::HashSet;

use pxl_spec::bake::constant::{band_cell_z, in_cell, unpack_pos, BandZ, CellZ, EntryAt, CELL, CELL_STEPS};
use pxl_spec::cell::cube_types;
use pxl_spec::cell::object::{decor_ids, decor_num_id, object_value_is_voxel, placeable_cube_num_id, placeable_cubes};

use crate::mesh::paint::object::material::solid_material;
use pxl_spec::space::pvf3::ivec::IVec;

use crate::mesh::volume::{pack_material, pack_sub_cell_fraction, Cell, Volumes, Voxel, NO_LAYER, SUB_CELL_NONE};

const AIR: u32 = cube_types::common::air as u32;

#[derive(Clone, Copy)]
pub struct Entries<'a> {
    pub col_idx: IVec,
    pub band: &'a [BandZ],
    pub pos: &'a [u32],
    pub state: &'a [u32],
}

impl Entries<'_> {
    pub fn at(&self, k: usize) -> EntryAt {
        EntryAt {
            col_idx: self.col_idx,
            band_idx: self.band[k],
            pos: self.pos[k],
        }
    }
}

/// Decor renders as instanced geometry, not a voxel.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Untaught = 0,
    Solid = 1,
    Air = 2,
    Decor = 3,
}

#[derive(Clone, Copy)]
pub struct Paint {
    pub any: bool,
    pub refused: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Painted {
    More(usize),
    Done,
    Refused,
}

pub struct Painter {
    kinds: Vec<Kind>,
    materials: Vec<u32>,
    layers: Vec<u8>,
    claimed: Claims,
}

/// So a second sub-cell object on a claimed voxel is filed as overflow rather
/// than overwriting it. A set so it can be asked for room before it grows.
pub type Claims = HashSet<Cell>;

impl Default for Painter {
    fn default() -> Self {
        Painter::new()
    }
}

impl Painter {
    pub fn new() -> Painter {
        let mut p = Painter {
            kinds: Vec::new(),
            materials: Vec::new(),
            layers: Vec::new(),
            claimed: HashSet::new(),
        };
        for &cube in placeable_cubes.iter() {
            let m = solid_material(cube);
            p.set(placeable_cube_num_id(cube), Kind::Solid, pack_material(&m), m.layer_idx as u8);
        }
        p.set(AIR, Kind::Air, 0, NO_LAYER as u8);
        for &id in decor_ids.iter() {
            p.set(decor_num_id(id), Kind::Decor, 0, NO_LAYER as u8);
        }
        p
    }

    /// A state past a leaf's width is a catalog bug and is not set, rather than
    /// sizing the tables to it.
    fn set(&mut self, state: u32, kind: Kind, material: u32, layer: u8) {
        if state >= u16::MAX as u32 {
            return;
        }
        let i = state as usize;
        if i >= self.kinds.len() {
            self.kinds.resize(i + 1, Kind::Untaught);
            self.materials.resize(i + 1, 0);
            self.layers.resize(i + 1, NO_LAYER as u8);
        }
        self.kinds[i] = kind;
        self.materials[i] = material;
        self.layers[i] = layer;
    }

    /// None for untaught, air and decor.
    pub fn material_of(&self, state: u32) -> Option<(u32, u8)> {
        match self.kind(state) {
            Kind::Solid => Some((self.materials[state as usize], self.layers[state as usize])),
            _ => None,
        }
    }

    pub fn is_voxel(&self, state: u32) -> bool {
        object_value_is_voxel(state)
    }

    fn kind(&self, state: u32) -> Kind {
        self.kinds.get(state as usize).copied().unwrap_or(Kind::Untaught)
    }

    /// Once a paint is refused the rest is not painted.
    pub fn paint(&mut self, entries: Option<Entries>, volumes: &mut Volumes, volume: u32, hide: Option<EntryAt>, carve: Option<EntryAt>) -> Paint {
        let mut claimed = core::mem::take(&mut self.claimed);
        claimed.clear();
        let mut any = false;
        let mut refused = matches!(self.paint_entries(entries, volumes, volume, hide, 0, usize::MAX, &mut claimed, &mut any), Painted::Refused);
        if !refused {
            refused = !self.paint_carve(volumes, volume, carve, &mut claimed, &mut any);
        }
        self.claimed = claimed;
        Paint { any, refused }
    }

    /// Bounded per step by `voxels`, each entry setting one voxel. `paint_carve` ends the paint.
    #[allow(clippy::too_many_arguments)]
    pub fn paint_entries(&self, entries: Option<Entries>, volumes: &mut Volumes, volume: u32, hide: Option<EntryAt>, from: usize, voxels: usize, claimed: &mut Claims, any: &mut bool) -> Painted {
        let Some(e) = entries else { return Painted::Done };
        let mut set = 0;
        for k in from..e.band.len() {
            if set >= voxels {
                return Painted::More(k);
            }
            if hide == Some(e.at(k)) {
                continue;
            }
            let (band, pos, state) = (e.band[k], e.pos[k], e.state[k]);
            match self.paint_entry(volumes, volume, band, pos, state, claimed, any) {
                Some(n) => set += n + 1,
                None => return Painted::Refused,
            }
        }
        Painted::Done
    }

    pub fn paint_carve(&self, volumes: &mut Volumes, volume: u32, carve: Option<EntryAt>, claimed: &mut Claims, any: &mut bool) -> bool {
        match carve {
            Some(c) => self.paint_entry(volumes, volume, c.band_idx, c.pos, AIR, claimed, any).is_some(),
            None => true,
        }
    }

    /// An entry is at most a cell (the store splits larger ones), so it is one voxel.
    #[allow(clippy::too_many_arguments)]
    fn paint_entry(&self, volumes: &mut Volumes, volume: u32, band: BandZ, pos: u32, state: u32, claimed: &mut Claims, any: &mut bool) -> Option<usize> {
        let kind = self.kind(state);
        if kind == Kind::Decor || kind == Kind::Untaught {
            return Some(0);
        }
        let (x, y, z, scale) = unpack_pos(pos);
        let ([cx, cy, cz], into) = in_cell([x, y, z]);
        let z = band_cell_z(band) as CellZ + cz as CellZ;
        let is_air = kind == Kind::Air;
        let (material, layer_idx) = if is_air {
            (0, NO_LAYER as u8)
        } else {
            (self.materials[state as usize], self.layers[state as usize])
        };

        let sub = if scale < CELL {
            let [ox, oy, oz] = into.map(|v| pack_sub_cell_fraction(v as f64 / CELL_STEPS as f64));
            let sz = pack_sub_cell_fraction(1.0 / scale.count_in(CELL) as f64);
            ox | (oy << 8) | (oz << 16) | (sz << 24)
        } else {
            (SUB_CELL_NONE as u32) << 24
        };
        let v = Voxel { material, layer_idx, sub };

        *any = true;
        let at = Cell { cx, cy, z };
        let ok = if claimed.contains(&at) && !is_air {
            volumes.append_sub_voxel(volume, at, v)
        } else {
            if claimed.len() == claimed.capacity() && claimed.try_reserve(claimed.len().max(64)).is_err() {
                return None;
            }
            claimed.insert(at);
            volumes.set_voxel(volume, at, v)
        };
        ok.then_some(1)
    }
}

#[cfg(test)]
#[path = "adapter.test.rs"]
mod test;
