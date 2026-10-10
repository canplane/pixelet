// xpute_core/ds/deque.rs
// deque primitives over an external ring buffer (0-based) — DOD view.
//
// Storage contract (caller-owned):
//   - buf.len() is the fixed capacity (cap)
//   - head: pop-front index
//   - len : live element count
//
// No defensive checks. Caller upholds preconditions.

pub struct Deque<'a, T> {
    // external storage (DOD-friendly): caller owns the ring buffer.
    pub buf: &'a mut [Option<T>],
    pub head: u32,
    pub len: u32,
}

impl<'a, T> Deque<'a, T> {
    pub fn new(buf: &'a mut [Option<T>], head: u32, len: u32) -> Deque<'a, T> {
        Deque { buf, head, len }
    }

    pub fn cap(&self) -> u32 {
        self.buf.len() as u32
    }

    pub fn size(&self) -> u32 {
        self.len
    }

    pub fn empty(&self) -> bool {
        self.len == 0
    }

    pub fn full(&self) -> bool {
        self.len == self.buf.len() as u32
    }

    pub fn front(&self) -> Option<&T> {
        if self.len != 0 {
            self.buf[self.head as usize].as_ref()
        } else {
            None
        }
    }

    pub fn back(&self) -> Option<&T> {
        if self.len == 0 {
            return None;
        }
        let cap = self.cap();
        let i = (self.head + self.len - 1) % cap;
        self.buf[i as usize].as_ref()
    }

    pub fn push_back(&mut self, v: T) {
        let cap = self.cap();
        let i = (self.head + self.len) % cap;
        self.buf[i as usize] = Some(v);
        self.len += 1;
    }

    pub fn push_front(&mut self, v: T) {
        let cap = self.cap();
        self.head = (self.head + cap - 1) % cap;
        self.buf[self.head as usize] = Some(v);
        self.len += 1;
    }

    pub fn pop_front(&mut self) -> T {
        let v = self.buf[self.head as usize].take().expect("deque: pop_front on an empty slot");
        self.head = (self.head + 1) % self.cap();
        self.len -= 1;
        v
    }

    pub fn pop_back(&mut self) -> T {
        let cap = self.cap();
        let i = (self.head + self.len - 1) % cap;
        let v = self.buf[i as usize].take().expect("deque: pop_back on an empty slot");
        self.len -= 1;
        v
    }

    pub fn clear(&mut self) {
        let cap = self.cap();
        for i in 0..self.len {
            self.buf[((self.head + i) % cap) as usize] = None;
        }
        self.head = 0;
        self.len = 0;
    }
}

#[cfg(test)]
#[path = "deque.test.rs"]
mod test;
