// xpute-core/collection/heap.rs

//! A binary heap over an array the caller owns, keyed by whatever orders.
//!
//! `BinaryHeap` takes the storage and wants `Ord`; here the array stays the
//! caller's and the key is anything `PartialOrd`, so a distance or a score
//! in `f64` keys it as it is, without a `total_cmp` newtype.
//!
//! One implementation serves both directions: `MIN` picks which way a key
//! outranks another and folds away at instantiation, so `MinHeap` and
//! `MaxHeap` are the same source and neither pays a comparison call.
//!
//! Keys are totally ordered and the caller upholds it: a NaN key makes the
//! two ways of asking — `ahead` and its negation — disagree, and the sift
//! walks off the order.

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Element<K, T> {
    pub key: K,
    pub val: T,
}

/// The array is the caller's, and `MIN` is the direction: smaller key first
/// when true, larger when false.
pub struct Heap<'a, K, T, const MIN: bool> {
    pub a: &'a mut Vec<Element<K, T>>,
}

pub type MinHeap<'a, K, T> = Heap<'a, K, T, true>;
pub type MaxHeap<'a, K, T> = Heap<'a, K, T, false>;

impl<'a, K: PartialOrd + Copy, T: Copy, const MIN: bool> Heap<'a, K, T, MIN> {
    pub fn new(a: &'a mut Vec<Element<K, T>>) -> Heap<'a, K, T, MIN> {
        Heap { a }
    }
}

impl<K: PartialOrd + Copy, T: Copy, const MIN: bool> Heap<'_, K, T, MIN> {
    /// Whether `x` outranks `y`. The only place the direction is read.
    fn ahead(x: K, y: K) -> bool {
        if MIN {
            x < y
        } else {
            x > y
        }
    }

    /// The root: the smallest key, or the largest.
    pub fn top(&self) -> Option<&Element<K, T>> {
        self.a.first()
    }

    /// Restores the heap property in `a[0..size)`, given that both of the
    /// root's subtrees already hold it.
    pub fn heapify(a: &mut [Element<K, T>], root: usize, size: usize) {
        let mut i = root;
        let e = a[i];

        let mut child = (i << 1) + 1;
        while child < size {
            let r = child + 1;
            if r < size && Self::ahead(a[r].key, a[child].key) {
                child = r;
            }

            if !Self::ahead(a[child].key, e.key) {
                break;
            }
            a[i] = a[child];
            i = child;
            child = (i << 1) + 1;
        }

        a[i] = e;
    }

    /// Bottom-up heapify, O(n).
    pub fn build_of(a: &mut [Element<K, T>]) {
        let n = a.len();
        for i in (0..(n >> 1)).rev() {
            Self::heapify(a, i, n);
        }
    }

    pub fn build(&mut self) {
        Self::build_of(self.a);
    }

    /// Hole sift-up: the new element's place is opened by moving parents
    /// down, so one write lands it rather than a swap per level.
    pub fn push(&mut self, e: Element<K, T>) {
        let mut i = self.a.len();
        self.a.push(e);

        while i > 0 {
            let p = (i - 1) >> 1;
            if !Self::ahead(e.key, self.a[p].key) {
                break;
            }
            self.a[i] = self.a[p];
            i = p;
        }

        self.a[i] = e;
    }

    /// Removes the root and returns it.
    pub fn pop(&mut self) -> Option<Element<K, T>> {
        if self.a.is_empty() {
            return None;
        }
        let out = self.a.swap_remove(0);
        let n = self.a.len();
        if n > 0 {
            Self::heapify(self.a, 0, n);
        }
        Some(out)
    }

    /// Overwrites the root and sifts it down. The heap must not be empty.
    pub fn replace_root(&mut self, e: Element<K, T>) {
        self.a[0] = e;
        let n = self.a.len();
        Self::heapify(self.a, 0, n);
    }
}

/// Sorts ascending by key: unstable, in place, O(n log n). A max-heap over
/// the whole array, then each root swapped to the end of the unsorted part.
pub fn heapsort<K: PartialOrd + Copy, T: Copy>(a: &mut [Element<K, T>]) {
    let n = a.len();
    if n <= 1 {
        return;
    }

    MaxHeap::build_of(a);

    for end in (1..n).rev() {
        a.swap(0, end);
        MaxHeap::heapify(a, 0, end);
    }
}

#[cfg(test)]
#[path = "heap.test.rs"]
mod test;
