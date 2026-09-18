// pxl-render/mesh/volume.rs

//! The render-time volume for one 64×64 chunk.
//!
//! World layers paint into whole 64³ signed-Z bands, and the scan and the
//! raycast read the same flat lanes back; a band is allocated in full even
//! if one peak or object touches it, and sparsity exists only between
//! bands. What a band is made of and why (materials packed `0xCC_RRGGBB`,
//! a layer byte apart so the direct payload is five bytes a voxel, sub-cell
//! bytes only for bands that hold sub-cell objects, the overflow list for
//! objects sharing a base voxel) is what voxel_scan.rs reads back, so the two
//! agree on the payload's shape and nothing between them converts.
//!
//! Bands are stores kept for reuse. A composite produces several, one chunk
//! at a time, and building them fresh was measured as the heap's worst churn,
//! so up to POOLED_BANDS of them are held with their lanes and handed to the
//! next band that needs one; past that a released store gives its lanes back
//! and is refilled when it is next taken.

// ---- the voxel ----
//
// What one voxel's bytes mean: its material class and colour packed into a
// word, its texture layer, and the inset of a box that fills only part of
// its cell. Pure encoding, shared by the volume that stores voxels, the
// scan that reads them into runs and the mesher that draws the runs.

/// A run whose material is a placed object: exempt from hiding its
/// neighbours' faces, and drawn with its bottom face.
use xpute_core::sys::mem::{alloc, free, Block, Guard};

pub const MATERIAL_OBJECT: u8 = 5;
/// A coarse tier's cells, whatever they summarise.
pub const MATERIAL_LAND: u8 = 1;

/// No texture-array layer applies; use the material's flat RGB.
pub const NO_LAYER: u32 = 255;

/// A full-cell run: no inset.
pub const SUB_CELL_NONE: u8 = 0;
const SUB_CELL_SCALE: f64 = 256.0;
/// The largest packed value, read as exactly 1 rather than 255/256.
///
/// An offset of 1 is meaningless, so `unpack_sub_cell_fraction` never needs
/// to express it. A *size* of 1 is not: coarse LOD fits a box to what is
/// actually inside a cell, and a box that fills one axis while shrinking
/// another is the ordinary case, not an edge one. Reading the top value as
/// full costs a comparison and keeps 0 free as SUB_CELL_NONE.
const SUB_CELL_FULL: u8 = 255;

/// A fraction of a cell packed into a byte, rounded to the nearest 1/256.
/// The fractions this packs are never negative, where `f64::round` and
/// `round_half_up` agree.
pub fn pack_sub_cell_fraction(fraction: f64) -> u32 {
    ((fraction * SUB_CELL_SCALE).round()).clamp(0.0, SUB_CELL_SCALE - 1.0) as u32
}

pub fn unpack_sub_cell_fraction(packed: u8) -> f64 {
    packed as f64 / SUB_CELL_SCALE
}

pub fn unpack_sub_cell_size(packed: u8) -> f64 {
    if packed == SUB_CELL_FULL {
        1.0
    } else {
        packed as f64 / SUB_CELL_SCALE
    }
}

/// `MaterialClass`.
pub type MaterialClass = u32;

#[allow(non_snake_case)]
pub mod MaterialClasses {
    pub const AIR: u32 = 0;
    pub const LAND: u32 = 1;
    pub const WATER: u32 = 2;
    pub const TRANSPORT: u32 = 3;
    pub const STRUCTURE: u32 = 4;
    pub const OBJECT: u32 = 5;
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct VoxelMaterial {
    pub material_class: MaterialClass,
    pub r: u32,
    pub g: u32,
    pub b: u32,
    pub layer_idx: u32,
}

pub fn pack_material(material: &VoxelMaterial) -> u32 {
    ((material.material_class & 255) << 24) | ((material.r & 255) << 16) | ((material.g & 255) << 8) | (material.b & 255)
}

pub fn unpack_material(packed: u32, layer_idx: u32) -> VoxelMaterial {
    VoxelMaterial {
        material_class: packed >> 24,
        r: (packed >> 16) & 255,
        g: (packed >> 8) & 255,
        b: packed & 255,
        layer_idx,
    }
}

// ---- the volume ----

/// A band is a chunk's span on every axis.
pub const BAND_SPAN: u32 = pxl_spec::bake::constant::CHUNK_CELL_SPAN;
const BAND_MASK: u32 = BAND_SPAN - 1;
const BAND_SPAN2: usize = (BAND_SPAN * BAND_SPAN) as usize;
pub const BAND_VOXELS: usize = BAND_SPAN2 * BAND_SPAN as usize;
/// A band's lanes: a word of material, a byte of layer and a word of
/// sub-cell a voxel.
const MATERIAL_BYTES: u32 = BAND_VOXELS as u32 * 4;
const LAYER_BYTES: u32 = BAND_VOXELS as u32;
const SUB_CELL_BYTES: u32 = BAND_VOXELS as u32 * 4;

/// Column-major band-local index: Z is contiguous for paint, scan and raycast.
pub fn band_voxel_idx(x: u32, y: u32, z: u32) -> usize {
    ((y * BAND_SPAN + x) * BAND_SPAN + z) as usize
}

/// How many band stores keep their lanes when their band is released. Past
/// this a store gives its lanes back and takes them again when it is next
/// used, so the list never grows past the most bands alive at once.
pub const POOLED_BANDS: usize = 16;

/// The overflow when several sub-cell objects share one base voxel: the
/// first takes the voxel, the rest are listed here, sorted by column once
/// the volume is sealed.
#[derive(Default)]
pub struct Extras {
    pub col: Vec<u32>,
    pub z: Vec<i32>,
    pub material: Vec<u32>,
    pub layer_idx: Vec<u8>,
    pub sub: Vec<u32>,
}

impl Extras {
    fn clear(&mut self) {
        self.col.clear();
        self.z.clear();
        self.material.clear();
        self.layer_idx.clear();
        self.sub.clear();
    }

    fn len(&self) -> usize {
        self.col.len()
    }

    fn push(&mut self, col: u32, z: i32, material: u32, layer_idx: u8, sub: u32) {
        self.col.push(col);
        self.z.push(z);
        self.material.push(material);
        self.layer_idx.push(layer_idx);
        self.sub.push(sub);
    }

    /// Drops every entry at `(col, z)`: a later layer replacing the voxel
    /// takes its overflow with it.
    fn remove_at(&mut self, col: u32, z: i32) {
        let mut w = 0;
        for i in 0..self.len() {
            if self.col[i] == col && self.z[i] == z {
                continue;
            }
            self.col[w] = self.col[i];
            self.z[w] = self.z[i];
            self.material[w] = self.material[i];
            self.layer_idx[w] = self.layer_idx[i];
            self.sub[w] = self.sub[i];
            w += 1;
        }
        self.col.truncate(w);
        self.z.truncate(w);
        self.material.truncate(w);
        self.layer_idx.truncate(w);
        self.sub.truncate(w);
    }

    /// By column, every lane together, stably: a column's own entries stay in
    /// the order they were appended, which is the order the scan reads them
    /// back in.
    fn sort_by_column(&mut self) {
        let mut order: Vec<usize> = (0..self.len()).collect();
        order.sort_by_key(|&i| self.col[i]);
        let take = |v: &mut Vec<u32>| {
            let sorted: Vec<u32> = order.iter().map(|&i| v[i]).collect();
            *v = sorted;
        };
        take(&mut self.col);
        take(&mut self.material);
        take(&mut self.sub);
        let z: Vec<i32> = order.iter().map(|&i| self.z[i]).collect();
        self.z = z;
        let layer: Vec<u8> = order.iter().map(|&i| self.layer_idx[i]).collect();
        self.layer_idx = layer;
    }
}

/// A cell of the chunk: chunk-local x and y, absolute z.
#[derive(Clone, Copy)]
pub struct Cell {
    pub cx: u32,
    pub cy: u32,
    pub z: i32,
}

/// What is painted: the packed material, the layer, and the four sub-cell
/// bytes as one word with the size in the top byte — zero for a full cell.
#[derive(Clone, Copy)]
pub struct Voxel {
    pub material: u32,
    pub layer_idx: u8,
    pub sub: u32,
}

/// One band's lanes: heap blocks, addressed in the memory. A store
/// past the cap gives its lanes back when its band is released and is
/// refilled by the next band that needs one, so the list never grows past
/// the most bands alive at once.
struct Store {
    material: usize,
    layer_idx: usize,
    /// Lazily taken for a band with sub-cell objects; four bytes a voxel.
    sub_cell: Option<Block>,
    extra: Extras,
}

/// One band of a volume: its store, and where it sits in Z.
pub struct Band {
    pub band_z: i32,
    store: usize,
}

pub struct Volume {
    /// Ascending by band_z.
    pub bands: Vec<Band>,
    pub sealed: bool,
}

/// What a band's row says to a reader, as ten words: the band's Z, its three
/// lane addresses (the sub-cell one 0 where the band has none), the extras'
/// count and their five lane addresses.
pub const BAND_WORDS: usize = 10;

#[derive(Clone, Copy, Default, Debug)]
#[repr(C)]
pub struct VolumeStat {
    /// Bands built fresh from the heap, and bands reused from the stores.
    pub allocated: u32,
    pub reused: u32,
    /// Volumes and bands refused for want of a block.
    pub refused: u32,
}

/// Every volume alive, and the stores they draw on.
pub struct Volumes {
    mem: *mut u8,
    stores: Vec<Store>,
    /// Stores with their lanes, waiting for a band.
    free_stores: Vec<usize>,
    /// Stores whose lanes were given back.
    empty_stores: Vec<usize>,
    volumes: Vec<Option<Volume>>,
    free_ids: Vec<usize>,
    /// The current band row, for readers.
    row: [u32; BAND_WORDS],
    pub stat: VolumeStat,
}

impl Volumes {
    /// Over the memory at `mem`, taking the standing stores from
    /// the heap.
    ///
    /// # Safety
    /// `mem` must be the base of the memory the lanes are addressed in.
    pub unsafe fn new(mem: *mut u8) -> Volumes {
        Volumes {
            mem,
            stores: Vec::new(),
            free_stores: Vec::new(),
            empty_stores: Vec::new(),
            volumes: Vec::new(),
            free_ids: Vec::new(),
            row: [0; BAND_WORDS],
            stat: VolumeStat::default(),
        }
    }

    /// A volume with no bands: its id.
    pub fn open(&mut self) -> usize {
        let v = Volume { bands: Vec::new(), sealed: false };
        if let Some(id) = self.free_ids.pop() {
            self.volumes[id] = Some(v);
            id
        } else {
            self.volumes.push(Some(v));
            self.volumes.len() - 1
        }
    }

    fn volume(&mut self, id: usize) -> &mut Volume {
        self.volumes[id].as_mut().expect("a volume that is not open")
    }

    fn material(&self, store: usize) -> *mut u32 {
        self.mem.wrapping_add(self.stores[store].material) as *mut u32
    }

    fn layer(&self, store: usize) -> *mut u8 {
        self.mem.wrapping_add(self.stores[store].layer_idx)
    }

    fn sub_cell(&self, store: usize) -> Option<*mut u8> {
        self.stores[store].sub_cell.map(|(at, _)| self.mem.wrapping_add(at))
    }

    /// A store for a new band: a free one, or one built from the heap —
    /// None when the heap has no room.
    fn take_store(&mut self) -> Option<usize> {
        if let Some(s) = self.free_stores.pop() {
            self.stat.reused += 1;
            return Some(s);
        }
        let Some(material) = Guard::alloc(MATERIAL_BYTES) else {
            self.stat.refused += 1;
            return None;
        };
        let Some((layer_idx, _)) = alloc(LAYER_BYTES) else {
            self.stat.refused += 1;
            return None;
        };
        let (material, _) = material.keep();
        self.stat.allocated += 1;
        if let Some(s) = self.empty_stores.pop() {
            let store = &mut self.stores[s];
            store.material = material;
            store.layer_idx = layer_idx;
            return Some(s);
        }
        self.stores.push(Store {
            material,
            layer_idx,
            sub_cell: None,
            extra: Extras::default(),
        });
        Some(self.stores.len() - 1)
    }

    /// The band at `band_z`, made if the volume has none there: the index
    /// into the volume's bands, or None when no store could be had.
    fn get_or_create_band(&mut self, id: usize, band_z: i32) -> Option<usize> {
        assert!(!self.volume(id).sealed, "paint into a sealed volume");
        let bands = &self.volume(id).bands;
        let insert_at = bands.iter().position(|b| b.band_z >= band_z).unwrap_or(bands.len());
        if insert_at < bands.len() && bands[insert_at].band_z == band_z {
            return Some(insert_at);
        }
        let store = self.take_store()?;
        // A store keeps whatever its last borrower wrote.
        // SAFETY: the store's lanes are this module's blocks of BAND_VOXELS.
        unsafe {
            core::slice::from_raw_parts_mut(self.material(store), BAND_VOXELS).fill(0);
            core::slice::from_raw_parts_mut(self.layer(store), BAND_VOXELS).fill(NO_LAYER as u8);
        }
        self.drop_sub_cell(store);
        self.stores[store].extra.clear();
        self.volume(id).bands.insert(insert_at, Band { band_z, store });
        Some(insert_at)
    }

    fn drop_sub_cell(&mut self, store: usize) {
        if let Some(block) = self.stores[store].sub_cell.take() {
            // SAFETY: the lane this store took and has not given back.
            unsafe { free(block) };
        }
    }

    /// The sub-cell lane of `store`, taken from the heap and zeroed, since
    /// zero is SUB_CELL_NONE.
    fn ensure_sub_cell(&mut self, store: usize) -> Option<*mut u8> {
        if self.stores[store].sub_cell.is_none() {
            let (at, bytes) = alloc(SUB_CELL_BYTES)?;
            self.stores[store].sub_cell = Some((at, bytes));
            // SAFETY: a block of `bytes` this module just took.
            unsafe { core::slice::from_raw_parts_mut(self.mem.wrapping_add(at), bytes as usize).fill(0) };
        }
        self.sub_cell(store)
    }

    fn locate(z: i32) -> (i32, u32) {
        let band_z = z.div_euclid(BAND_SPAN as i32);
        (band_z, (z - band_z * BAND_SPAN as i32) as u32)
    }

    /// Full replacement at one voxel: later layers overwrite earlier ones.
    /// False when a band could not be made.
    pub fn set_voxel(&mut self, id: usize, at: Cell, v: Voxel) -> bool {
        let Cell { cx, cy, z } = at;
        // A column past the band would index the next block in the heap:
        // refused rather than written.
        if cx >= BAND_SPAN || cy >= BAND_SPAN {
            return false;
        }
        let Voxel { material, layer_idx, sub } = v;
        let (band_z, local_z) = Self::locate(z);
        let Some(b) = self.get_or_create_band(id, band_z) else { return false };
        let store = self.volume(id).bands[b].store;
        let idx = band_voxel_idx(cx, cy, local_z);
        // SAFETY: lanes of BAND_VOXELS; idx is inside a band.
        unsafe {
            *self.material(store).add(idx) = material;
            *self.layer(store).add(idx) = layer_idx;
        }
        if self.stores[store].extra.len() > 0 {
            let col = (cy & BAND_MASK) * BAND_SPAN + (cx & BAND_MASK);
            self.stores[store].extra.remove_at(col, z);
        }
        if sub >> 24 != SUB_CELL_NONE as u32 {
            let Some(lane) = self.ensure_sub_cell(store) else { return false };
            // SAFETY: the lane is BAND_VOXELS * 4 bytes.
            unsafe {
                let at = lane.add(idx * 4);
                *at = sub as u8;
                *at.add(1) = (sub >> 8) as u8;
                *at.add(2) = (sub >> 16) as u8;
                *at.add(3) = (sub >> 24) as u8;
            }
        } else if let Some(lane) = self.sub_cell(store) {
            unsafe { core::slice::from_raw_parts_mut(lane.add(idx * 4), 4).fill(0) };
        }
        true
    }

    /// Another sub-cell object on a base voxel already claimed in the same
    /// layer.
    pub fn append_sub_voxel(&mut self, id: usize, at: Cell, v: Voxel) -> bool {
        let Cell { cx, cy, z } = at;
        if cx >= BAND_SPAN || cy >= BAND_SPAN {
            return false;
        }
        let Voxel { material, layer_idx, sub } = v;
        let (band_z, _) = Self::locate(z);
        let Some(b) = self.get_or_create_band(id, band_z) else { return false };
        let store = self.volume(id).bands[b].store;
        let col = (cy & BAND_MASK) * BAND_SPAN + (cx & BAND_MASK);
        self.stores[store].extra.push(col, z, material, layer_idx, sub);
        true
    }

    /// Paints one solid column across as many whole bands as it needs.
    pub fn fill_column(&mut self, id: usize, column: (u32, u32), z0: i32, z1: i32, v: Voxel) -> bool {
        let (cx, cy) = column;
        if cx >= BAND_SPAN || cy >= BAND_SPAN {
            return false;
        }
        let Voxel { material, layer_idx, .. } = v;
        if z1 < z0 {
            return true;
        }
        let span = BAND_SPAN as i32;
        let (first, _) = Self::locate(z0);
        let (last, _) = Self::locate(z1);
        for band_z in first..=last {
            let Some(b) = self.get_or_create_band(id, band_z) else { return false };
            let store = self.volume(id).bands[b].store;
            let local_z0 = if band_z == first { (z0 - band_z * span) as u32 } else { 0 };
            let local_z1 = if band_z == last { (z1 - band_z * span) as u32 } else { BAND_SPAN - 1 };
            let start = band_voxel_idx(cx, cy, local_z0);
            let end = band_voxel_idx(cx, cy, local_z1) + 1;
            // SAFETY: lanes of BAND_VOXELS; start..end is one column of a band.
            unsafe {
                core::slice::from_raw_parts_mut(self.material(store).add(start), end - start).fill(material);
                core::slice::from_raw_parts_mut(self.layer(store).add(start), end - start).fill(layer_idx);
                if let Some(lane) = self.sub_cell(store) {
                    core::slice::from_raw_parts_mut(lane.add(start * 4), (end - start) * 4).fill(0);
                }
            }
        }
        true
    }

    /// Closes the volume to painting and orders every band's extras by
    /// column, which is what lets a column find its own during the scan.
    pub fn seal(&mut self, id: usize) {
        let v = self.volume(id);
        v.sealed = true;
        let stores: Vec<usize> = v.bands.iter().map(|b| b.store).collect();
        for s in stores {
            self.stores[s].extra.sort_by_column();
        }
    }

    /// Returns the volume's bands to the stores, and the stores past the
    /// cap to the heap.
    pub fn release(&mut self, id: usize) {
        let Some(v) = self.volumes[id].take() else { return };
        for band in v.bands {
            let s = band.store;
            self.stores[s].extra.clear();
            self.drop_sub_cell(s);
            if self.free_stores.len() < POOLED_BANDS {
                self.free_stores.push(s);
            } else {
                let store = &mut self.stores[s];
                // SAFETY: the lanes this store took and has not given back.
                unsafe {
                    free((store.material, MATERIAL_BYTES));
                    free((store.layer_idx, LAYER_BYTES));
                }
                self.empty_stores.push(s);
            }
        }
        self.free_ids.push(id);
    }

    /// The memory the volumes' addresses are offsets into.
    pub fn mem(&self) -> *mut u8 {
        self.mem
    }

    /// Band `i` of volume `id` as the scan reads it, over the memory.
    ///
    /// # Safety
    /// The views are valid until the volume is painted again or released.
    pub unsafe fn band_view(&mut self, id: usize, i: usize) -> crate::mesh::voxel_scan::BandView<'static> {
        let row = *self.band_row(id, i);
        let mem = self.mem;
        let at = |w: u32| mem.add(w as usize);
        let voxels = BAND_VOXELS;
        let n = row[4] as usize;
        crate::mesh::voxel_scan::BandView {
            band_z: row[0] as i32,
            material: core::slice::from_raw_parts(at(row[1]) as *const u32, voxels),
            layer_idx: core::slice::from_raw_parts(at(row[2]) as *const u8, voxels),
            sub_cell: (row[3] != 0).then(|| core::slice::from_raw_parts(at(row[3]) as *const u8, voxels * 4)),
            extra: crate::mesh::voxel_scan::ExtraView {
                col: core::slice::from_raw_parts(at(row[5]) as *const u32, n),
                z: core::slice::from_raw_parts(at(row[6]) as *const i32, n),
                material: core::slice::from_raw_parts(at(row[7]) as *const u32, n),
                layer_idx: core::slice::from_raw_parts(at(row[8]), n),
                sub: core::slice::from_raw_parts(at(row[9]) as *const u32, n),
            },
        }
    }

    /// Every band of volume `id` into `out`, cleared first: the scan and
    /// the raycast walk a volume by its bands many times a tick, so the
    /// list is theirs to keep, not built anew each walk.
    ///
    /// # Safety
    /// As `band_view`: valid until the volume is painted again or released.
    pub unsafe fn band_views(&mut self, id: usize, out: &mut Vec<crate::mesh::voxel_scan::BandView<'static>>) {
        out.clear();
        for i in 0..self.band_count(id) {
            out.push(self.band_view(id, i));
        }
    }

    pub fn band_count(&self, id: usize) -> usize {
        self.volumes[id].as_ref().map_or(0, |v| v.bands.len())
    }

    /// The row of band `i` of volume `id`: band_z, the lanes' addresses (the
    /// sub-cell one 0 when the band has none), the extras' count and their
    /// five lanes' addresses — valid until the volume is painted again.
    pub fn band_row(&mut self, id: usize, i: usize) -> &[u32; BAND_WORDS] {
        let band = &self.volumes[id].as_ref().expect("a volume that is not open").bands[i];
        let store = &self.stores[band.store];
        let e = &store.extra;
        let rel = |p: *const u8| (p as usize).wrapping_sub(self.mem as usize) as u32;
        self.row = [
            band.band_z as u32,
            store.material as u32,
            store.layer_idx as u32,
            store.sub_cell.map_or(0, |(at, _)| at as u32),
            e.len() as u32,
            rel(e.col.as_ptr() as *const u8),
            rel(e.z.as_ptr() as *const u8),
            rel(e.material.as_ptr() as *const u8),
            rel(e.layer_idx.as_ptr()),
            rel(e.sub.as_ptr() as *const u8),
        ];
        &self.row
    }
}
