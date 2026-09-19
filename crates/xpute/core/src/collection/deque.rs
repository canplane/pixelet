// xpute-core/collection/deque.rs

//! A ring over a buffer the caller owns: `buf` is the capacity, `head` is
//! where the front sits, and `len` counts live elements forward from there.
//! The three are public because they are the caller's — this allocates
//! nothing, grows never, and holds no state of its own.
//!
//! A slot is `Option<T>` because that is how a value leaves a borrowed slice
//! without unsafe. It carries no meaning: `head` and `len` alone say which
//! slots are live.

pub struct Deque<'a, T> {
    pub buf: &'a mut [Option<T>],
    pub head: u32,
    pub len: u32,
}

impl<'a, T> Deque<'a, T> {
    pub fn new(buf: &'a mut [Option<T>], head: u32, len: u32) -> Deque<'a, T> {
        Deque { buf, head, len }
    }
}

impl<T> Deque<'_, T> {
    pub fn cap(&self) -> u32 {
        self.buf.len() as u32
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn is_full(&self) -> bool {
        self.len == self.cap()
    }

    pub fn front(&self) -> Option<&T> {
        if self.len == 0 {
            return None;
        }
        self.buf[self.head as usize].as_ref()
    }

    pub fn back(&self) -> Option<&T> {
        if self.len == 0 {
            return None;
        }
        let i = (self.head + self.len - 1) % self.cap();
        self.buf[i as usize].as_ref()
    }

    pub fn push_back(&mut self, v: T) {
        let i = (self.head + self.len) % self.cap();
        self.buf[i as usize] = Some(v);
        self.len += 1;
    }

    pub fn push_front(&mut self, v: T) {
        let cap = self.cap();
        self.head = (self.head + cap - 1) % cap;
        self.buf[self.head as usize] = Some(v);
        self.len += 1;
    }

    pub fn pop_front(&mut self) -> Option<T> {
        if self.len == 0 {
            return None;
        }
        let v = self.buf[self.head as usize].take();
        self.head = (self.head + 1) % self.cap();
        self.len -= 1;
        v
    }

    pub fn pop_back(&mut self) -> Option<T> {
        if self.len == 0 {
            return None;
        }
        let i = (self.head + self.len - 1) % self.cap();
        self.len -= 1;
        self.buf[i as usize].take()
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
