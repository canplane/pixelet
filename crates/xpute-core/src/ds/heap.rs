// @xpute/core/ds/heap.ts
// heap primitives over an external array (0-based).
//
// Element:
//   - key: number (priority key)
//   - val: payload
//
// Indexing (0-based):
//   parent(i) = (i - 1) >> 1
//   left(i)   = (i << 1) + 1
//   right(i)  = left(i) + 1
//
// Semantics:
//   - MinHeap: smaller key = higher priority (top is minimum key)
//   - MaxHeap: larger  key = higher priority (top is maximum key)
//
// Notes:
// - heapify(root, size) assumes both child subtrees are already heaps.
// - build() is O(n) bottom-up heapify.
// - No defensive checks (caller upholds preconditions; keys are totally ordered).

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Element<T> {
    pub key: f64,
    pub val: T,
}

pub struct MinHeap<'a, T> {
    // external storage (DOD-friendly): we operate on caller-owned array.
    pub a: &'a mut Vec<Element<T>>,
}

impl<'a, T: Copy> MinHeap<'a, T> {
    pub fn new(a: &'a mut Vec<Element<T>>) -> MinHeap<'a, T> {
        MinHeap { a }
    }

    pub fn size(&self) -> usize {
        self.a.len()
    }

    pub fn empty(&self) -> bool {
        self.a.is_empty()
    }

    pub fn top(&self) -> Option<&Element<T>> {
        self.a.first()
    }

    /// heapify(a, root, size)
    /// - restore MIN-heap property in a[0..size)
    /// - assumes child subtrees already satisfy heap property.
    pub fn heapify(a: &mut [Element<T>], root: usize, size: usize) {
        let mut i = root;
        let e = a[i];

        let mut child = (i << 1) + 1;
        while child < size {
            let r = child + 1;
            if r < size && a[r].key < a[child].key {
                child = r;
            }

            if a[child].key < e.key {
                a[i] = a[child];
                i = child;
            } else {
                break;
            }
            child = (i << 1) + 1;
        }

        a[i] = e;
    }

    /// build(a)
    /// - bottom-up heapify, O(n)
    pub fn build_of(a: &mut [Element<T>]) {
        let n = a.len();
        for i in (0..(n >> 1)).rev() {
            MinHeap::heapify(a, i, n);
        }
    }

    pub fn build(&mut self) {
        MinHeap::build_of(self.a);
    }

    /// push(e)
    /// - hole sift-up (MIN)
    pub fn push(&mut self, e: Element<T>) {
        let mut i = self.a.len();
        self.a.push(e);

        while i > 0 {
            let p = (i - 1) >> 1;
            if self.a[p].key <= e.key {
                break;
            }
            self.a[i] = self.a[p];
            i = p;
        }

        self.a[i] = e;
    }

    /// pop() -> Element | undefined
    /// - removes and returns root (minimum key)
    pub fn pop(&mut self) -> Option<Element<T>> {
        let n = self.a.len();
        if n == 0 {
            return None;
        }

        let out = self.a[0];

        if n == 1 {
            self.a.pop();
            return Some(out);
        }

        self.a[0] = self.a.pop().unwrap();
        MinHeap::heapify(self.a, 0, n - 1);
        Some(out)
    }

    /// replace_root(e)
    /// - overwrite root and restore heap property
    /// - requires non-empty heap
    pub fn replace_root(&mut self, e: Element<T>) {
        self.a[0] = e;
        let n = self.a.len();
        MinHeap::heapify(self.a, 0, n);
    }
}

pub struct MaxHeap<'a, T> {
    pub a: &'a mut Vec<Element<T>>,
}

impl<'a, T: Copy> MaxHeap<'a, T> {
    pub fn new(a: &'a mut Vec<Element<T>>) -> MaxHeap<'a, T> {
        MaxHeap { a }
    }

    pub fn size(&self) -> usize {
        self.a.len()
    }

    pub fn empty(&self) -> bool {
        self.a.is_empty()
    }

    pub fn top(&self) -> Option<&Element<T>> {
        self.a.first()
    }

    /// heapify(a, root, size)
    /// - restore MAX-heap property in a[0..size)
    /// - assumes child subtrees already satisfy heap property.
    pub fn heapify(a: &mut [Element<T>], root: usize, size: usize) {
        let mut i = root;
        let e = a[i];

        let mut child = (i << 1) + 1;
        while child < size {
            let r = child + 1;
            if r < size && a[r].key > a[child].key {
                child = r;
            }

            if a[child].key > e.key {
                a[i] = a[child];
                i = child;
            } else {
                break;
            }
            child = (i << 1) + 1;
        }

        a[i] = e;
    }

    pub fn build_of(a: &mut [Element<T>]) {
        let n = a.len();
        for i in (0..(n >> 1)).rev() {
            MaxHeap::heapify(a, i, n);
        }
    }

    pub fn build(&mut self) {
        MaxHeap::build_of(self.a);
    }

    /// push(e)
    /// - hole sift-up (MAX)
    pub fn push(&mut self, e: Element<T>) {
        let mut i = self.a.len();
        self.a.push(e);

        while i > 0 {
            let p = (i - 1) >> 1;
            if self.a[p].key >= e.key {
                break;
            }
            self.a[i] = self.a[p];
            i = p;
        }

        self.a[i] = e;
    }

    /// pop() -> Element | undefined
    /// - removes and returns root (maximum key)
    pub fn pop(&mut self) -> Option<Element<T>> {
        let n = self.a.len();
        if n == 0 {
            return None;
        }

        let out = self.a[0];

        if n == 1 {
            self.a.pop();
            return Some(out);
        }

        self.a[0] = self.a.pop().unwrap();
        MaxHeap::heapify(self.a, 0, n - 1);
        Some(out)
    }

    /// replace_root(e)
    /// - overwrite root and restore heap property
    /// - requires non-empty heap
    pub fn replace_root(&mut self, e: Element<T>) {
        self.a[0] = e;
        let n = self.a.len();
        MaxHeap::heapify(self.a, 0, n);
    }
}

/// heapsort(a)
/// - unstable, in-place, O(n log n)
/// - sorts ASC by key using MAX-heap (classic: pop max to the end)
///
/// This mirrors the classic C heapsort structure:
///   build (bottom-up heapify)
///   for end=n-1..1: swap(a[0], a[end]); heapify(a, 0, end)
pub fn heapsort<T: Copy>(a: &mut [Element<T>]) {
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
