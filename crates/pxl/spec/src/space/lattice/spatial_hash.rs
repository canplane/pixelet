// pxl-spec/space/lattice/spatial_hash.rs

//! Broadphase index for sparse, moving points in 3D, rebuilt every frame and
//! allocating nothing while it does.
//!
//! The regime this is for is the one an octree is worst at: many points in a
//! small volume, moving. A tree prunes nothing when everything lands in two
//! leaves and charges traversal for the privilege, and a dense grid pays for
//! cells it never fills — which is fatal when the world has no bounds to
//! size a grid against. Hashing quantised positions is insensitive to both
//! density and emptiness, and re-insertion on movement is a bucket write.
//!
//! **Counting sort, not a map of buckets.** Zhyhallo (ICAART 2024) revisits
//! Teschner's 2003 algorithm and finds the bottleneck is RAM I/O rather than
//! CPU — so the layout outranks the algorithm, and more cores do not help.
//! The build is therefore three passes over flat integer arrays: count per
//! bucket, prefix-sum the starts, place each id with a bump. No `Map`, no
//! per-entity object, no array literal. A per-frame rebuild therefore asks
//! the allocator for nothing: the arrays are the ones it already has.
//!
//! **A generation stamp replaces the clear.** Teschner's trick: each rebuild
//! increments a counter, and a bucket whose stamp is stale reads as empty
//! and is overwritten rather than being zeroed in a reset pass. The prefix
//! sum then walks only the buckets actually written this generation — kept
//! in `touched` — so the cost tracks the points present, never the table
//! size. This is Zhyhallo's chunked allocation reduced to its essential
//! part: never pay for the empty regions of an unbounded space.
//!
//! **Cell size is the caller's, and it is the query radius.** Teschner
//! measured cell size against primitive extent and found the optimum at a
//! ratio of exactly one, and that cell size matters more to performance than
//! table size. Here the thing being asked about is a query, so its extent is
//! the radius: one cell per radius keeps a query to 27 buckets in 3D and
//! keeps a point in exactly one. A different consumer with a different
//! radius wants a different index rather than a compromise in this one.
//!
//! **Broadphase, so the caller does the exact test.** `query` reports every
//! id whose *cell* the query box overlaps — no false negatives, and false
//! positives bounded by geometry at one cell. Positions are not stored here:
//! the caller already holds them and already needs them for whatever it does
//! with a hit, and every precedent draws the line in the same place.
//!
//! The cell coordinate of each item *is* stored, and that is not the same
//! thing. Without it a hash collision hands back points from anywhere in the
//! world, so the caller's rejection cost would scale with the hash's quality
//! and with how much world exists — which is exactly the property this
//! structure is chosen for not having. Three extra integer lanes buy an
//! error bound that is a statement about geometry instead of a hope about a
//! hash, and they remove repeats along with it: a bucket reached twice by a
//! wide box emits only the cell it was reached for.
//!
//! **Capacity is fixed and exceeding it drops rather than grows.** A
//! structure whose job is to make the live object count constant must not
//! itself be the thing that scales with the scene. Overflow is reported, not
//! absorbed.
//!
//! Portable by construction: integer lanes and index arithmetic, the same
//! over `Box<[u32]>` and under WASM over linear memory.

use super::hash::hash3;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SpatialHashStats {
    /// Points inserted in the current generation.
    pub count: u32,
    /// Insertions refused because capacity was reached.
    pub dropped: u32,
    /// Buckets written in the current generation.
    pub touched: u32,
}

pub struct SpatialHash {
    /// Multiply rather than shift: world coordinates are real-valued here, and
    /// for a power-of-two cell the reciprocal is exact, so this is the shift
    /// that Zhyhallo's integer form spells `>>`.
    inv_cell: f64,

    capacity: u32,
    bucket_count: u32,

    /// Per bucket, valid only while `stamps[b] == generation`.
    counts: Box<[u32]>,
    starts: Box<[u32]>,
    stamps: Box<[u32]>,

    /// Buckets written this generation, so the prefix sum never walks the
    /// whole table.
    touched: Box<[u32]>,
    touched_len: u32,

    /// Parallel to insertion order: the bucket each inserted id hashed to, so
    /// the placing pass does not re-derive it.
    cells: Box<[u32]>,
    /// Parallel to insertion order: the caller's id and its cell.
    ids: Box<[u32]>,
    ins_cx: Box<[i32]>,
    ins_cy: Box<[i32]>,
    ins_cz: Box<[i32]>,

    /// The sorted output: ids grouped by bucket.
    items: Box<[u32]>,
    /// Parallel to `items`: the cell each one actually occupies, so a bucket
    /// shared by unrelated cells can be filtered down to the cell asked for.
    item_cx: Box<[i32]>,
    item_cy: Box<[i32]>,
    item_cz: Box<[i32]>,

    /// Bump cursor per bucket during the placing pass, folded back into
    /// `starts` afterwards so `starts[b]` is the group's first index.
    cursors: Box<[u32]>,

    generation: u32,
    count: u32,
    dropped: u32,
    built: bool,
}

impl SpatialHash {
    /// - `capacity`: most points in one generation; further insertions are dropped
    /// - `bucket_count`: prime, and comfortably above `capacity` — Teschner
    ///   measured the curve flattening well before that, and the cost of
    ///   overshooting is memory rather than time
    /// - `cell_size`: the query radius this index will be asked about
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
            cells: take(capacity),
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

    /// Cell coordinate of a world coordinate on one axis.
    pub fn cell_of(&self, v: f64) -> i32 {
        (v * self.inv_cell).floor() as i32
    }

    /// Bucket for a cell coordinate triple.
    fn bucket(&self, cx: i32, cy: i32, cz: i32) -> u32 {
        hash3(cx, cy, cz) % self.bucket_count
    }

    /// Opens a generation. Nothing is cleared: the stamp makes every bucket
    /// from the previous generation read as empty on its next touch.
    ///
    /// The counter is `u32` and wraps at 2^32; a wrap would make stale buckets
    /// read as current, so it resets the stamps once — every 4 billion
    /// rebuilds, which at 120 Hz is a little over a year of continuous frames.
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

    /// Records one point. Returns false when capacity is spent.
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
        self.cells[n] = b as u32;
        self.ids[n] = id;
        self.ins_cx[n] = cx;
        self.ins_cy[n] = cy;
        self.ins_cz[n] = cz;
        true
    }

    /// Closes the generation: prefix-sum the starts over the touched buckets,
    /// then place every recorded id.
    ///
    /// The bump cursors are a separate array rather than a mutated copy of
    /// `starts`, because `starts` has to survive as the group offset for the
    /// queries that follow.
    pub fn build(&mut self) {
        let mut offset: u32 = 0;
        for t in 0..self.touched_len as usize {
            let b = self.touched[t] as usize;
            self.starts[b] = offset;
            self.cursors[b] = offset;
            offset += self.counts[b];
        }
        for n in 0..self.count as usize {
            let b = self.cells[n] as usize;
            let at = self.cursors[b] as usize;
            self.cursors[b] = at as u32 + 1;
            self.items[at] = self.ids[n];
            self.item_cx[at] = self.ins_cx[n];
            self.item_cy[at] = self.ins_cy[n];
            self.item_cz[at] = self.ins_cz[n];
        }
        self.built = true;
    }

    /// Every id whose cell overlaps the axis-aligned box of `radius` about a
    /// point, written into `out`, returning how many.
    ///
    /// Broadphase: no false negatives, and false positives only from the cell
    /// quantisation itself — the caller applies the exact test it was going to
    /// apply anyway. No id appears twice. Stops at `out.len()` and reports
    /// the truncation through the return value, since a caller with a
    /// fixed-size result buffer has already decided how many it can use.
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
                        // The bucket is shared with every other cell that hashes here,
                        // and with itself when a wide box reaches it twice. Comparing
                        // the cell is what turns "probably nearby" into "in the box, or
                        // one quantisation step outside it".
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
