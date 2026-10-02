// pxl-spec/space/lattice/spatial_hash.rs

//! Broadphase index for sparse, moving 3D points, rebuilt every frame without
//! allocating: a counting sort over flat arrays (count, prefix-sum, place),
//! with a generation stamp instead of a clear so the cost tracks the points
//! present, not the table size. The cell size is the query radius, so a query
//! reads 27 buckets. Each item's cell is stored so a hash collision never
//! returns a point from elsewhere. Past capacity, insertions are dropped.

use super::hash::hash3;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SpatialHashStats {
    pub count: u32,
    pub dropped: u32,
    pub touched: u32,
}

pub struct SpatialHash {
    /// Exact for a power-of-two cell.
    inv_cell: f64,

    capacity: u32,
    bucket_count: u32,

    /// Valid only while `stamps[b] == generation`.
    counts: Box<[u32]>,
    starts: Box<[u32]>,
    stamps: Box<[u32]>,

    touched: Box<[u32]>,
    touched_len: u32,

    /// Per insertion: its bucket, id and cell.
    buckets: Box<[u32]>,
    ids: Box<[u32]>,
    ins_cx: Box<[i32]>,
    ins_cy: Box<[i32]>,
    ins_cz: Box<[i32]>,

    /// Ids grouped by bucket, with each one's cell.
    items: Box<[u32]>,
    item_cx: Box<[i32]>,
    item_cy: Box<[i32]>,
    item_cz: Box<[i32]>,

    /// Separate from `starts`, which queries still need after the placing pass.
    cursors: Box<[u32]>,

    generation: u32,
    count: u32,
    dropped: u32,
    built: bool,
}

impl SpatialHash {
    /// `bucket_count` should be prime and comfortably above `capacity`;
    /// `cell_size` is the query radius.
    pub fn new(capacity: u32, bucket_count: u32, cell_size: f64) -> SpatialHash {
        fn take<T: Copy + Default>(n: u32) -> Box<[T]> {
            vec![T::default(); n as usize].into_boxed_slice()
        }
        SpatialHash {
            inv_cell: 1.0 / cell_size,
            capacity,
            bucket_count,
            counts: take(bucket_count),
            starts: take(bucket_count),
            stamps: take(bucket_count),
            cursors: take(bucket_count),
            touched: take(bucket_count),
            touched_len: 0,
            buckets: take(capacity),
            ids: take(capacity),
            ins_cx: take(capacity),
            ins_cy: take(capacity),
            ins_cz: take(capacity),
            items: take(capacity),
            item_cx: take(capacity),
            item_cy: take(capacity),
            item_cz: take(capacity),
            generation: 0,
            count: 0,
            dropped: 0,
            built: false,
        }
    }

    pub fn cell_of(&self, v: f64) -> i32 {
        (v * self.inv_cell).floor() as i32
    }

    fn bucket(&self, cx: i32, cy: i32, cz: i32) -> u32 {
        hash3(cx, cy, cz) % self.bucket_count
    }

    /// Opens a generation; stale stamps read as empty. On the u32 wrap
    /// (about a year at 120 Hz) the stamps are reset once.
    pub fn begin(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        if self.generation == 0 {
            self.stamps.fill(0);
            self.generation = 1;
        }
        self.touched_len = 0;
        self.count = 0;
        self.dropped = 0;
        self.built = false;
    }

    /// Returns false when capacity is spent.
    pub fn insert(&mut self, id: u32, x: f64, y: f64, z: f64) -> bool {
        if self.count >= self.capacity {
            self.dropped += 1;
            return false;
        }
        let (cx, cy, cz) = (self.cell_of(x), self.cell_of(y), self.cell_of(z));
        let b = self.bucket(cx, cy, cz) as usize;

        if self.stamps[b] != self.generation {
            self.stamps[b] = self.generation;
            self.counts[b] = 0;
            self.touched[self.touched_len as usize] = b as u32;
            self.touched_len += 1;
        }
        self.counts[b] += 1;

        let n = self.count as usize;
        self.count += 1;
        self.buckets[n] = b as u32;
        self.ids[n] = id;
        self.ins_cx[n] = cx;
        self.ins_cy[n] = cy;
        self.ins_cz[n] = cz;
        true
    }

    pub fn build(&mut self) {
        let mut offset: u32 = 0;
        for t in 0..self.touched_len as usize {
            let b = self.touched[t] as usize;
            self.starts[b] = offset;
            self.cursors[b] = offset;
            offset += self.counts[b];
        }
        for n in 0..self.count as usize {
            let b = self.buckets[n] as usize;
            let at = self.cursors[b] as usize;
            self.cursors[b] = at as u32 + 1;
            self.items[at] = self.ids[n];
            self.item_cx[at] = self.ins_cx[n];
            self.item_cy[at] = self.ins_cy[n];
            self.item_cz[at] = self.ins_cz[n];
        }
        self.built = true;
    }

    /// Every id whose cell overlaps the box of `radius` about a point, each
    /// once, into `out`; returns how many, stopping at `out.len()`. The caller
    /// applies the exact test.
    pub fn query(&self, x: f64, y: f64, z: f64, radius: f64, out: &mut [u32]) -> u32 {
        if !self.built {
            return 0;
        }

        let (x0, x1) = (self.cell_of(x - radius), self.cell_of(x + radius));
        let (y0, y1) = (self.cell_of(y - radius), self.cell_of(y + radius));
        let (z0, z1) = (self.cell_of(z - radius), self.cell_of(z + radius));

        let mut n: u32 = 0;
        for cz in z0..=z1 {
            for cy in y0..=y1 {
                for cx in x0..=x1 {
                    let b = self.bucket(cx, cy, cz) as usize;
                    if self.stamps[b] != self.generation {
                        continue;
                    }
                    let start = self.starts[b] as usize;
                    let end = start + self.counts[b] as usize;
                    for i in start..end {
                        // The bucket is shared with every cell that hashes here, and with itself when a wide box reaches it twice.
                        if self.item_cx[i] != cx || self.item_cy[i] != cy || self.item_cz[i] != cz {
                            continue;
                        }
                        if n as usize >= out.len() {
                            return n;
                        }
                        out[n as usize] = self.items[i];
                        n += 1;
                    }
                }
            }
        }
        n
    }

    pub fn stats(&self) -> SpatialHashStats {
        SpatialHashStats {
            count: self.count,
            dropped: self.dropped,
            touched: self.touched_len,
        }
    }
}

#[cfg(test)]
#[path = "spatial_hash.test.rs"]
mod test;
