// xpute-core/collection/heap.rs

//! A binary heap over an array the caller owns, keyed by any `PartialOrd`
//! (an `f64` needs no `total_cmp` newtype). A NaN key breaks the order.

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Element<K, T> {
    pub key: K,
    pub val: T,
}

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
    fn ahead(x: K, y: K) -> bool {
        if MIN {
            x < y
        } else {
            x > y
        }
    }

    pub fn top(&self) -> Option<&Element<K, T>> {
        self.a.first()
    }

    /// Sifts `root` down; both of its subtrees must already be heaps.
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

    pub fn build_of(a: &mut [Element<K, T>]) {
        let n = a.len();
        for i in (0..(n >> 1)).rev() {
            Self::heapify(a, i, n);
        }
    }

    pub fn build(&mut self) {
        Self::build_of(self.a);
    }

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

    /// The heap must not be empty.
    pub fn replace_root(&mut self, e: Element<K, T>) {
        self.a[0] = e;
        let n = self.a.len();
        Self::heapify(self.a, 0, n);
    }
}

/// Ascending, unstable, in place.
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
